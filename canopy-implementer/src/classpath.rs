//! An index of the classes a project can load: its own, the JDK's, and those of the jars its build resolves.
//!
//! The import audit in [`crate::java`] knows the repository's classes and a short table of JDK ones. This
//! extends the same check to library classes, so a wrong or missing import can be refused, before the build, with
//! the exact line to write. Design and the measurements behind it: `docs/design/classpath-class-index.md`.
//!
//! No data is not absence: only an index whose classpath was resolved ([`ClassIndex::resolved`]) may say a class
//! is not there.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::java;

/// Where a class comes from, in the order candidates are preferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Origin {
    /// The project's own sources.
    Project,
    /// A jar on the compile classpath.
    Compile,
    Jdk,
    /// A jar only the tests have.
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub qualified: String,
    pub origin: Origin,
}

/// JDK packages preferred when a simple name exists in several (`java.util.List` over `java.awt.List`).
const COMMON_JDK_PACKAGES: &[&str] = &[
    "java.util",
    "java.util.function",
    "java.util.stream",
    "java.time",
    "java.io",
    "java.math",
    "java.net",
    "java.nio.file",
    "java.util.concurrent",
];

#[derive(Debug, Default)]
pub struct ClassIndex {
    by_simple: HashMap<String, Vec<Candidate>>,
    qualified: HashSet<String>,
    resolved: bool,
    /// How many times the project's own sources import each qualified name.
    imported: HashMap<String, u32>,
}

impl ClassIndex {
    /// An index whose classpath was resolved but that has nothing in it yet.
    pub fn resolved_empty() -> Self {
        Self {
            resolved: true,
            ..Self::default()
        }
    }

    /// An index that may be partial: it never claims a class is absent.
    pub fn unresolved() -> Self {
        Self::default()
    }

    pub fn resolved(&self) -> bool {
        self.resolved
    }

    /// Adds the top-level classes among jar or module entries (`a/b/C.class`). Nested classes, `module-info` and
    /// `package-info`, multi-release copies and anything that is not a class are left out. A class already
    /// present keeps its first origin.
    pub fn add_entries<I, S>(&mut self, entries: I, origin: Origin)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for entry in entries {
            let entry = entry.as_ref();
            let Some(path) = entry.strip_suffix(".class") else {
                continue;
            };
            if path.contains('$') || path.starts_with("META-INF/") {
                continue;
            }
            let qualified = path.replace('/', ".");
            let simple = qualified.rsplit('.').next().unwrap_or_default();
            if simple == "module-info" || simple == "package-info" || !self.qualified.insert(qualified.clone()) {
                continue;
            }
            self.by_simple
                .entry(simple.to_string())
                .or_default()
                .push(Candidate { qualified, origin });
        }
    }

    pub fn candidates(&self, simple: &str) -> &[Candidate] {
        self.by_simple.get(simple).map_or(&[], Vec::as_slice)
    }

    pub fn contains(&self, qualified: &str) -> bool {
        self.qualified.contains(qualified)
    }

    /// Records that the project's own sources import `qualified`, which ranks it first among candidates.
    pub fn note_import(&mut self, qualified: &str) {
        *self.imported.entry(qualified.to_string()).or_insert(0) += 1;
    }

    fn origin_of(&self, qualified: &str) -> Option<Origin> {
        let simple = qualified.rsplit('.').next()?;
        self.candidates(simple)
            .iter()
            .find(|c| c.qualified == qualified)
            .map(|c| c.origin)
    }

    /// The sort key of a candidate: more imported by the project first, then project over library over JDK, then
    /// the common JDK packages.
    fn key(&self, c: &Candidate) -> (std::cmp::Reverse<u32>, Origin, usize) {
        let package = c.qualified.rsplit_once('.').map_or("", |(p, _)| p);
        (
            std::cmp::Reverse(self.imported.get(&c.qualified).copied().unwrap_or(0)),
            c.origin,
            COMMON_JDK_PACKAGES
                .iter()
                .position(|p| *p == package)
                .unwrap_or(usize::MAX),
        )
    }

    /// The candidates ranked. When the first beats the second on the key, only it is returned: the ranking
    /// decided. When they tie, all are returned and none is chosen.
    fn decide(&self, mut candidates: Vec<Candidate>) -> Vec<String> {
        candidates.sort_by_key(|c| (self.key(c), c.qualified.clone()));
        match candidates.as_slice() {
            [] => vec![],
            [only] => vec![only.qualified.clone()],
            [first, second, ..] if self.key(first) != self.key(second) => vec![first.qualified.clone()],
            _ => candidates.into_iter().map(|c| c.qualified).collect(),
        }
    }

    /// The index for the repository at `root`: the project's imports, the JDK, and the jars of its Maven
    /// classpath. Resolved only when Maven gave a classpath; without it the index is partial and never claims
    /// absence.
    pub fn for_repository(root: &Path) -> Self {
        let mut index = Self::unresolved();
        if let Ok(classpath) = resolve_classpath(root) {
            let mut compile_ok = true;
            for jar in &classpath.compile {
                match jar_entries(jar) {
                    Some(entries) => index.add_entries(entries, Origin::Compile),
                    None => compile_ok = false,
                }
            }
            for jar in classpath.test.iter().filter(|j| !classpath.compile.contains(j)) {
                match jar_entries(jar) {
                    Some(entries) => index.add_entries(entries, Origin::Test),
                    None => compile_ok = false,
                }
            }
            if let Some(listing) = jdk_listing() {
                index.add_entries(jdk_entries(&listing), Origin::Jdk);
                index.resolved = compile_ok;
            }
        }
        index.note_project_imports(root);
        index
    }

    fn note_project_imports(&mut self, root: &Path) {
        fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                match entry.file_type() {
                    Ok(t) if t.is_dir() => walk(&path, found),
                    Ok(t) if t.is_file() && path.extension().is_some_and(|e| e == "java") => found.push(path),
                    _ => {}
                }
            }
        }
        let mut files = Vec::new();
        walk(&root.join("src"), &mut files);
        for file in files {
            if let Ok(source) = std::fs::read_to_string(&file) {
                for import in java::imports_in(&source) {
                    if !import.starts_with("static ") && !import.ends_with(".*") {
                        self.note_import(&import);
                    }
                }
            }
        }
    }
}

/// The public API classes of a `jimage list` output: not the internals (`sun`, `jdk`, `com.sun`).
pub fn jdk_entries(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter_map(|l| l.strip_prefix("    "))
        .map(str::trim)
        .filter(|e| e.ends_with(".class") && !e.contains('$'))
        .filter(|e| {
            !(e.starts_with("sun/") || e.starts_with("jdk/") || e.starts_with("com/sun/") || e.starts_with("META-INF/"))
        })
        .map(str::to_string)
        .collect()
}

pub fn parse_classpath(text: &str) -> Vec<PathBuf> {
    text.trim()
        .split(':')
        .filter(|p| !p.trim().is_empty())
        .map(|p| PathBuf::from(p.trim()))
        .collect()
}

#[derive(Debug, Default)]
pub struct Classpath {
    /// What production code compiles against.
    pub compile: Vec<PathBuf>,
    /// What tests compile against: everything.
    pub test: Vec<PathBuf>,
}

/// The classpaths Maven resolves for the project at `root` (`dependency:build-classpath`, once per scope; the
/// first call downloads the plugin).
pub fn resolve_classpath(root: &Path) -> Result<Classpath, String> {
    let ask = |scope: &str| -> Result<Vec<PathBuf>, String> {
        let out = std::env::temp_dir().join(format!("canopy-classpath-{}-{scope}.txt", std::process::id()));
        let result = Command::new("mvn")
            .args(["-q", "dependency:build-classpath"])
            .arg(format!("-Dmdep.outputFile={}", out.display()))
            .arg(format!("-Dmdep.includeScope={scope}"))
            .current_dir(root)
            .output()
            .map_err(|e| format!("mvn: {e}"))?;
        let text = std::fs::read_to_string(&out);
        let _ = std::fs::remove_file(&out);
        match (result.status.success(), text) {
            (true, Ok(text)) => Ok(parse_classpath(&text)),
            _ => Err(format!("mvn dependency:build-classpath ({scope}) failed")),
        }
    };
    Ok(Classpath {
        compile: ask("compile")?,
        test: ask("test")?,
    })
}

/// The entry names of a jar, from `unzip -Z1`; `None` when the jar cannot be listed.
fn jar_entries(jar: &Path) -> Option<Vec<String>> {
    let out = Command::new("unzip").arg("-Z1").arg(jar).output().ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    })
}

/// The JDK's own listing, from `jimage` of the `java` on the path.
fn jdk_listing() -> Option<String> {
    let java = std::process::Command::new("sh")
        .args(["-c", "command -v java"])
        .output()
        .ok()?;
    let java = PathBuf::from(String::from_utf8_lossy(&java.stdout).trim());
    let home = std::fs::canonicalize(java).ok()?.parent()?.parent()?.to_path_buf();
    let out = Command::new(home.join("bin/jimage"))
        .arg("list")
        .arg(home.join("lib/modules"))
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A finding about a Java file's imports, with what the model is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A class used without an import. One candidate means the exact line is known (or the ranking decided).
    Missing { class: String, candidates: Vec<String> },
    /// An import that does not exist, for a class that exists elsewhere.
    WrongImport { imported: String, suggestions: Vec<String> },
    /// An import of a class that is not on the classpath at all.
    Absent { imported: String },
    /// Production code importing a class only the tests have.
    TestScope { imported: String },
}

impl Finding {
    /// The import line that fixes it, when there is exactly one.
    pub fn exact_fix(&self) -> Option<String> {
        match self {
            Finding::Missing { candidates, .. }
            | Finding::WrongImport {
                suggestions: candidates,
                ..
            } => matches!(candidates.as_slice(), [_]).then(|| format!("import {};", candidates[0])),
            _ => None,
        }
    }

    pub fn message(&self) -> String {
        let list = |names: &[String]| names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
        match self {
            Finding::Missing { class, candidates } => match self.exact_fix() {
                Some(line) => format!("`{class}` is used without an import; add `{line}`"),
                None => format!(
                    "`{class}` is used without an import; it could be {}. Decide which one you mean and import it",
                    list(candidates)
                ),
            },
            Finding::WrongImport { imported, suggestions } => match self.exact_fix() {
                Some(line) => format!("`import {imported};` does not exist; the class is `{}`: use `{line}`", suggestions[0]),
                None => format!("`import {imported};` does not exist; the class could be {}", list(suggestions)),
            },
            Finding::Absent { imported } => format!(
                "`import {imported};`: that class is not available on this project's classpath; use a class the project has, or do without it"
            ),
            Finding::TestScope { imported } => format!(
                "`import {imported};` is a test-scope class; production code cannot use it"
            ),
        }
    }
}

/// The names a file declares itself (`class X`, `interface X`, `enum X`, `record X`).
fn declared_names(source: &str) -> HashSet<String> {
    let words = java::code_words(source);
    words
        .windows(2)
        .filter(|w| matches!(w[0].as_str(), "class" | "interface" | "enum" | "record"))
        .map(|w| w[1].clone())
        .collect()
}

/// Findings about the imports of one Java file, against the classes the project has (`project`: simple name to
/// qualified names, including those of this submission) and the class index. The project's own classes are left
/// to [`java::missing_imports`]: a name the project has is never looked up in a library.
pub fn import_findings(
    path: &str,
    source: &str,
    project: &HashMap<String, Vec<String>>,
    index: &ClassIndex,
) -> Vec<Finding> {
    let production = path.starts_with("src/main/");
    let own_package = java::package_in(source).or_else(|| {
        path.strip_prefix("src/main/java/")
            .or_else(|| path.strip_prefix("src/test/java/"))
            .and_then(|p| p.rsplit_once('/'))
            .map(|(dir, _)| dir.replace('/', "."))
    });
    let own_class = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned());
    let imports = java::imports_in(source);
    let explicit: Vec<&String> = imports
        .iter()
        .filter(|i| !i.starts_with("static ") && !i.ends_with(".*"))
        .collect();
    let wildcards: Vec<&str> = imports.iter().filter_map(|i| i.strip_suffix(".*")).collect();
    let declared = declared_names(source);
    let mut found: Vec<Finding> = Vec::new();

    // 1. Imports that do not resolve.
    for import in &explicit {
        let simple = import.rsplit('.').next().unwrap_or(import);
        let in_project = project.get(simple).is_some_and(|q| q.contains(import));
        let parent_is_class = import
            .rsplit_once('.')
            .is_some_and(|(parent, _)| index.contains(parent) || project.values().flatten().any(|q| q == parent));
        if in_project || parent_is_class {
            continue;
        }
        if index.contains(import) {
            if production && index.origin_of(import) == Some(Origin::Test) {
                found.push(Finding::TestScope {
                    imported: (*import).clone(),
                });
            }
            continue;
        }
        let mut elsewhere: Vec<Candidate> = index
            .candidates(simple)
            .iter()
            .filter(|c| !(production && c.origin == Origin::Test))
            .cloned()
            .collect();
        elsewhere.extend(project.get(simple).into_iter().flatten().map(|q| Candidate {
            qualified: q.clone(),
            origin: Origin::Project,
        }));
        if !elsewhere.is_empty() {
            found.push(Finding::WrongImport {
                imported: (*import).clone(),
                suggestions: index.decide(elsewhere),
            });
        } else if index.resolved() && !shares_project_root(import, own_package.as_deref()) {
            found.push(Finding::Absent {
                imported: (*import).clone(),
            });
        }
    }

    // 2. Classes used without an import.
    let mut seen: HashSet<String> = HashSet::new();
    for word in java::code_words(source) {
        if word.len() < 2 || !seen.insert(word.clone()) {
            continue;
        }
        if own_class.as_deref() == Some(word.as_str()) || declared.contains(&word) || project.contains_key(&word) {
            continue;
        }
        if explicit.iter().any(|i| i.rsplit('.').next() == Some(word.as_str())) {
            continue;
        }
        let usable: Vec<Candidate> = index
            .candidates(&word)
            .iter()
            .filter(|c| !(production && c.origin == Origin::Test))
            .cloned()
            .collect();
        if usable.is_empty() {
            continue;
        }
        let covered = usable.iter().any(|c| {
            let package = c.qualified.rsplit_once('.').map_or("", |(p, _)| p);
            package == "java.lang" || own_package.as_deref() == Some(package) || wildcards.contains(&package)
        });
        if covered {
            continue;
        }
        found.push(Finding::Missing {
            class: word,
            candidates: index.decide(usable),
        });
    }
    found
}

/// Whether an import sits in the project's own package tree (the first two segments match), where the classes
/// are the project's and may simply not be written yet.
fn shares_project_root(import: &str, own_package: Option<&str>) -> bool {
    let Some(own) = own_package else { return false };
    let head = |s: &str| s.split('.').take(2).collect::<Vec<_>>().join(".");
    head(import) == head(own)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn index() -> ClassIndex {
        let mut index = ClassIndex::resolved_empty();
        index.add_entries(
            [
                "org/springframework/boot/test/web/server/LocalServerPort.class",
                "org/springframework/stereotype/Service.class",
                "org/springframework/web/bind/annotation/RestController.class",
                "org/apache/catalina/Service.class",
                "org/springframework/beans/factory/annotation/Autowired.class",
                "org/springframework/context/annotation/Lazy.class",
                "com/example/lib/Request.class",
            ],
            Origin::Compile,
        );
        index.add_entries(
            [
                "org/junit/jupiter/api/Test.class",
                "org/junit/jupiter/api/Assertions.class",
            ],
            Origin::Test,
        );
        index.add_entries(
            [
                "java/util/List.class",
                "java/util/UUID.class",
                "java/awt/List.class",
                "java/lang/String.class",
                "java/util/Map.class",
            ],
            Origin::Jdk,
        );
        index
    }

    fn findings(path: &str, source: &str) -> Vec<Finding> {
        import_findings(path, source, &HashMap::new(), &index())
    }

    const PROD: &str = "src/main/java/com/example/widgets/Thing.java";
    const TEST: &str = "src/test/java/com/example/widgets/ThingTest.java";

    #[test]
    fn entries_keep_top_level_classes_only_and_skip_nested_module_and_package_files() {
        let mut index = ClassIndex::resolved_empty();
        index.add_entries(
            [
                "a/b/Outer.class",
                "a/b/Outer$Inner.class",
                "a/b/module-info.class",
                "a/b/package-info.class",
                "META-INF/MANIFEST.MF",
                "a/b/resource.txt",
            ],
            Origin::Compile,
        );
        assert_eq!(index.candidates("Outer").len(), 1);
        assert_eq!(index.candidates("Outer")[0].qualified, "a.b.Outer");
        assert!(index.candidates("Inner").is_empty() && index.candidates("module-info").is_empty());
        assert!(index.contains("a.b.Outer") && !index.contains("a.b.Inner"));
    }

    #[test]
    fn the_jdk_listing_gives_the_public_api_classes_of_each_module_and_not_the_internals() {
        let listing = "jimage: /x/lib/modules\n\nModule: java.base\n    META-INF/services/foo\n    java/util/List.class\n    java/util/Map$Entry.class\n    sun/security/Internal.class\n    jdk/internal/misc/Unsafe.class\n\nModule: java.desktop\n    java/awt/List.class\n    javax/swing/JFrame.class\n";
        let mut names = jdk_entries(listing);
        names.sort();
        assert_eq!(
            names,
            [
                "java/awt/List.class",
                "java/util/List.class",
                "javax/swing/JFrame.class"
            ]
        );
    }

    #[test]
    fn a_classpath_file_is_split_into_its_entries() {
        assert_eq!(
            parse_classpath("/a/x.jar:/b/y.jar\n"),
            [
                std::path::PathBuf::from("/a/x.jar"),
                std::path::PathBuf::from("/b/y.jar")
            ]
        );
        assert!(parse_classpath("  \n").is_empty());
    }

    #[test]
    fn a_library_class_used_without_an_import_is_named_with_the_exact_line_when_there_is_one_candidate() {
        let source = "package com.example.widgets;\n\nclass ThingTest {\n    @LocalServerPort\n    int port;\n}\n";
        let found = findings(TEST, source);
        assert_eq!(
            found,
            [Finding::Missing {
                class: "LocalServerPort".into(),
                candidates: vec!["org.springframework.boot.test.web.server.LocalServerPort".into()]
            }]
        );
        assert_eq!(
            found[0].exact_fix().as_deref(),
            Some("import org.springframework.boot.test.web.server.LocalServerPort;")
        );
    }

    #[test]
    fn an_import_from_the_wrong_package_is_named_with_the_package_the_class_is_in() {
        let source = "package com.example.widgets;\nimport org.springframework.boot.web.server.LocalServerPort;\nclass ThingTest {\n    @LocalServerPort int port;\n}\n";
        let found = findings(TEST, source);
        assert_eq!(
            found,
            [Finding::WrongImport {
                imported: "org.springframework.boot.web.server.LocalServerPort".into(),
                suggestions: vec!["org.springframework.boot.test.web.server.LocalServerPort".into()]
            }]
        );
        assert_eq!(
            found[0].exact_fix().as_deref(),
            Some("import org.springframework.boot.test.web.server.LocalServerPort;")
        );
    }

    #[test]
    fn an_import_of_a_class_that_is_not_on_the_classpath_is_absent_only_when_the_classpath_was_resolved() {
        let source = "package com.example.widgets;\nimport jakarta.validation.constraints.NotBlank;\nclass Thing { @NotBlank String name; }\n";
        let found = findings(PROD, source);
        assert_eq!(
            found,
            [Finding::Absent {
                imported: "jakarta.validation.constraints.NotBlank".into()
            }]
        );
        assert!(
            found[0].exact_fix().is_none(),
            "no import fixes a class that is not there"
        );
        let mut unresolved = ClassIndex::unresolved();
        unresolved.add_entries(["java/util/List.class"], Origin::Jdk);
        assert!(
            import_findings(PROD, source, &HashMap::new(), &unresolved).is_empty(),
            "no data is not absence"
        );
    }

    #[test]
    fn a_name_in_two_packages_is_listed_with_candidates_and_never_picked_silently() {
        let source = "package com.example.widgets;\n@Service\nclass Thing {}\n";
        let found = findings(PROD, source);
        let Finding::Missing { candidates, .. } = &found[0] else {
            panic!("{found:?}")
        };
        assert_eq!(candidates.len(), 2, "{candidates:?}");
        assert!(found[0].exact_fix().is_none(), "ambiguous: no single fix");
    }

    #[test]
    fn a_class_the_project_already_imports_elsewhere_is_ranked_first_among_candidates() {
        let mut index = index();
        index.note_import("org.springframework.stereotype.Service");
        index.note_import("org.springframework.stereotype.Service");
        let source = "package com.example.widgets;\n@Service\nclass Thing {}\n";
        let found = import_findings(PROD, source, &HashMap::new(), &index);
        let Finding::Missing { candidates, .. } = &found[0] else {
            panic!("{found:?}")
        };
        assert_eq!(candidates[0], "org.springframework.stereotype.Service");
        assert_eq!(
            found[0].exact_fix().as_deref(),
            Some("import org.springframework.stereotype.Service;"),
            "the ranking decides"
        );
    }

    #[test]
    fn jdk_names_rank_by_the_common_packages() {
        let source = "package com.example.widgets;\nclass Thing { List<String> l; }\n";
        let found = findings(PROD, source);
        assert_eq!(
            found[0].exact_fix().as_deref(),
            Some("import java.util.List;"),
            "{found:?}"
        );
    }

    #[test]
    fn nothing_is_reported_for_a_name_that_is_imported_covered_declared_or_in_java_lang() {
        let imported = "package a;\nimport org.springframework.stereotype.Service;\n@Service class Thing {}\n";
        assert!(findings(PROD, imported).is_empty());
        let wildcard = "package a;\nimport org.springframework.stereotype.*;\n@Service class Thing {}\n";
        assert!(findings(PROD, wildcard).is_empty());
        let declared = "package a;\nclass Thing { record Request(String q) {} Request r; }\n";
        assert!(findings(PROD, declared).is_empty(), "Request is declared in the file");
        let lang = "package a;\nclass Thing { String s; }\n";
        assert!(findings(PROD, lang).is_empty());
        let qualified = "package a;\nclass Thing { java.util.List<String> l; }\n";
        assert!(findings(PROD, qualified).is_empty(), "a qualified use needs no import");
        let own = "package a;\nclass Service {}\n";
        assert!(
            findings("src/main/java/a/Service.java", own).is_empty(),
            "the file's own class"
        );
    }

    #[test]
    fn a_project_class_wins_over_a_library_class_of_the_same_name_and_is_left_to_the_existing_audit() {
        let mut project = HashMap::new();
        project.insert("Request".to_string(), vec!["com.example.widgets.Request".to_string()]);
        let source = "package com.example.widgets;\nclass Thing { Request r; }\n";
        assert!(import_findings(PROD, source, &project, &index()).is_empty());
    }

    #[test]
    fn production_code_that_imports_a_class_only_the_tests_have_is_a_test_scope_finding() {
        let source = "package a;\nimport org.junit.jupiter.api.Test;\nclass Thing { @Test void t() {} }\n";
        assert_eq!(
            findings(PROD, source),
            [Finding::TestScope {
                imported: "org.junit.jupiter.api.Test".into()
            }]
        );
        assert!(findings(TEST, source).is_empty(), "a test may import it");
    }

    #[test]
    fn a_finding_reads_as_what_the_model_is_told() {
        let missing = Finding::Missing {
            class: "LocalServerPort".into(),
            candidates: vec!["a.b.LocalServerPort".into()],
        };
        assert_eq!(
            missing.message(),
            "`LocalServerPort` is used without an import; add `import a.b.LocalServerPort;`"
        );
        let several = Finding::Missing {
            class: "Service".into(),
            candidates: vec!["a.Service".into(), "b.Service".into()],
        };
        assert!(
            several.message().contains("a.Service")
                && several.message().contains("b.Service")
                && several.message().contains("which")
        );
        let wrong = Finding::WrongImport {
            imported: "x.LocalServerPort".into(),
            suggestions: vec!["a.b.LocalServerPort".into()],
        };
        assert!(wrong.message().contains("x.LocalServerPort") && wrong.message().contains("a.b.LocalServerPort"));
        let absent = Finding::Absent {
            imported: "jakarta.validation.constraints.NotBlank".into(),
        };
        assert!(absent.message().contains("not available on this project's classpath"));
        let scope = Finding::TestScope {
            imported: "org.junit.jupiter.api.Test".into(),
        };
        assert!(scope.message().contains("test") && scope.message().contains("org.junit.jupiter.api.Test"));
    }
}
