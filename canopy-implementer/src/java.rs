//! What the harness knows about Java source: where a class lives, how to import it, which JDK classes a
//! model tends to forget. This is the one place that knows Java's rules, so a language registry can take
//! its place later without touching the callers.

use std::collections::HashMap;
use std::path::Path;

/// Classes the JDK ships that a model forgets to import. Only consulted when the repository has no class
/// of that name.
const JDK_CLASSES: &[(&str, &str)] = &[
    ("UUID", "java.util.UUID"),
    ("Optional", "java.util.Optional"),
    ("List", "java.util.List"),
    ("ArrayList", "java.util.ArrayList"),
    ("Map", "java.util.Map"),
    ("HashMap", "java.util.HashMap"),
    ("Set", "java.util.Set"),
    ("HashSet", "java.util.HashSet"),
    ("Objects", "java.util.Objects"),
    ("Collectors", "java.util.stream.Collectors"),
    ("ConcurrentHashMap", "java.util.concurrent.ConcurrentHashMap"),
    ("Instant", "java.time.Instant"),
    ("LocalDate", "java.time.LocalDate"),
    ("BigDecimal", "java.math.BigDecimal"),
];

/// The fully qualified name of a well-known JDK class, if `class` is one.
pub fn jdk_class(class: &str) -> Option<&'static str> {
    JDK_CLASSES
        .iter()
        .find(|(name, _)| *name == class)
        .map(|(_, full)| *full)
}

/// `src/main/java/com/example/greeting/Greeter.java` gives `com.example.greeting`.
pub fn package_of(path: &str) -> Option<String> {
    let inside = path.strip_prefix("src/main/java/")?;
    let (directory, _) = inside.rsplit_once('/')?;
    Some(directory.replace('/', "."))
}

/// The package a source or test file sits in, from its path.
fn source_package_of(path: &str) -> Option<String> {
    let inside = path
        .strip_prefix("src/main/java/")
        .or_else(|| path.strip_prefix("src/test/java/"))?;
    let (directory, _) = inside.rsplit_once('/')?;
    Some(directory.replace('/', "."))
}

/// `src/main/java/com/example/greeting/Greeter.java` gives `com.example.greeting.Greeter`.
pub fn qualified_name(path: &str) -> Option<String> {
    let package = package_of(path)?;
    let class = Path::new(path).file_stem()?.to_string_lossy().into_owned();
    Some(format!("{package}.{class}"))
}

/// Every production class under `src/main/java`: simple name to the qualified names that carry it. A name two
/// packages share has two entries, and is treated as ambiguous by `missing_imports`. Symlinks are skipped.
pub fn class_index(root: &Path) -> HashMap<String, Vec<String>> {
    fn walk(dir: &Path, relative: &str, index: &mut HashMap<String, Vec<String>>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let Ok(file_type) = entry.file_type() else { continue };
            if file_type.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = format!("{relative}/{name}");
            if file_type.is_dir() {
                walk(&entry.path(), &path, index);
            } else if let Some(class) = name.strip_suffix(".java") {
                if let Some(qualified) = qualified_name(&path) {
                    index.entry(class.to_string()).or_default().push(qualified);
                }
            }
        }
    }
    let mut index = HashMap::new();
    walk(&root.join("src/main/java"), "src/main/java", &mut index);
    index
}

/// The `package x.y;` of a source file.
pub fn package_in(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("package ")?;
        Some(rest.trim_end_matches(';').trim().to_string())
    })
}

/// The imports of a source file, without `import ` and `;`. A static import keeps its `static ` prefix.
pub fn imports_in(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("import ")?;
            Some(rest.trim_end_matches(';').trim().to_string())
        })
        .collect()
}

/// The identifiers a source file uses as code: not in comments, string or character literals, or the package
/// and import lines, and not after a `.` (a qualified use needs no import).
fn code_words(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut code = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            code.push(' ');
        } else if c == '"' && chars.get(i + 1) == Some(&'"') && chars.get(i + 2) == Some(&'"') {
            i += 3;
            while i + 2 < chars.len() && !(chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"') {
                i += 1;
            }
            i += 3;
            code.push(' ');
        } else if c == '"' || c == '\'' {
            i += 1;
            while i < chars.len() && chars[i] != c && chars[i] != '\n' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            code.push(' ');
        } else {
            code.push(c);
            i += 1;
        }
    }
    let mut words = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("import ") || trimmed.starts_with("package ") {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut j = 0;
        let mut previous_significant: Option<char> = None;
        while j < chars.len() {
            let c = chars[j];
            if c.is_alphabetic() || c == '_' || c == '$' {
                let start = j;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '$') {
                    j += 1;
                }
                if previous_significant != Some('.') {
                    words.push(chars[start..j].iter().collect());
                }
                previous_significant = Some('a');
            } else {
                if !c.is_whitespace() {
                    previous_significant = Some(c);
                }
                j += 1;
            }
        }
    }
    words
}

/// The qualified names a file uses but does not import: a class of the repository (found in `index`) or a
/// well-known JDK class, named in code, that is neither in the file's own package, nor imported, nor covered by
/// a wildcard import. A name the repository carries in two packages is ambiguous and never reported; a class
/// of the repository beats a JDK class of the same name; the file's own class is never reported.
pub fn missing_imports(path: &str, source: &str, index: &HashMap<String, Vec<String>>) -> Vec<String> {
    let own_class = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned());
    let own_package = package_in(source).or_else(|| source_package_of(path));
    let imports = imports_in(source);
    let mut missing: Vec<String> = Vec::new();
    for word in code_words(source) {
        if own_class.as_deref() == Some(word.as_str()) {
            continue;
        }
        let qualified = match index.get(&word).map(Vec::as_slice) {
            Some([only]) => only.clone(),
            Some(_) => continue,
            None => match jdk_class(&word) {
                Some(full) => full.to_string(),
                None => continue,
            },
        };
        let package = qualified
            .rsplit_once('.')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default();
        let covered = own_package.as_deref() == Some(package.as_str())
            || imports.contains(&qualified)
            || imports.contains(&format!("{package}.*"));
        if !covered && !missing.contains(&qualified) {
            missing.push(qualified);
        }
    }
    missing.sort();
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_comes_from_the_path_under_src_main_java() {
        assert_eq!(
            package_of("src/main/java/com/example/widgets/domain/Widget.java").as_deref(),
            Some("com.example.widgets.domain")
        );
        assert_eq!(package_of("src/test/java/com/example/WidgetTest.java"), None);
        assert_eq!(package_of("src/main/java/Loose.java"), None);
    }

    #[test]
    fn the_qualified_name_is_the_package_and_the_file_stem() {
        assert_eq!(
            qualified_name("src/main/java/com/example/widgets/domain/Widget.java").as_deref(),
            Some("com.example.widgets.domain.Widget")
        );
    }

    fn index(pairs: &[(&str, &str)]) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (class, qualified) in pairs {
            map.entry(class.to_string()).or_default().push(qualified.to_string());
        }
        map
    }

    const SERVICE: &str = "src/main/java/com/example/service/WidgetService.java";

    #[test]
    fn the_package_and_imports_are_read_from_the_source() {
        let src =
            "package com.example.service;\n\nimport java.util.Optional;\nimport com.example.domain.*;\n\nclass A {}\n";
        assert_eq!(package_in(src).as_deref(), Some("com.example.service"));
        assert_eq!(
            imports_in(src),
            vec!["java.util.Optional".to_string(), "com.example.domain.*".to_string()]
        );
    }

    #[test]
    fn a_known_class_used_without_an_import_is_missing() {
        let src =
            "package com.example.service;\nclass WidgetService { Optional<Widget> find(UUID id) { return null; } }\n";
        let idx = index(&[("Widget", "com.example.domain.Widget")]);
        assert_eq!(
            missing_imports(SERVICE, src, &idx),
            vec![
                "com.example.domain.Widget".to_string(),
                "java.util.Optional".to_string(),
                "java.util.UUID".to_string()
            ]
        );
    }

    #[test]
    fn an_imported_class_a_wildcard_covered_class_and_a_same_package_class_are_not_missing() {
        let src = "package com.example.service;\nimport java.util.Optional;\nimport com.example.domain.*;\n\
                   class WidgetService { Optional<Widget> a; Helper h; }\n";
        let idx = index(&[
            ("Widget", "com.example.domain.Widget"),
            ("Helper", "com.example.service.Helper"),
        ]);
        assert_eq!(missing_imports(SERVICE, src, &idx), Vec::<String>::new());
    }

    #[test]
    fn words_in_comments_strings_and_the_import_and_package_lines_do_not_count() {
        let src = "package com.example.service;\n// uses UUID and Optional\n/* List */\n\
                   class WidgetService { String s = \"Optional UUID\"; char c = 'x'; /* Map */ }\n";
        assert_eq!(missing_imports(SERVICE, src, &index(&[])), Vec::<String>::new());
    }

    #[test]
    fn the_files_own_class_and_a_name_that_two_packages_share_are_not_flagged() {
        let src = "package com.example.service;\nclass WidgetService { WidgetService self; Clash c; }\n";
        let idx = index(&[
            ("WidgetService", "com.example.other.WidgetService"),
            ("Clash", "a.Clash"),
            ("Clash", "b.Clash"),
        ]);
        assert_eq!(missing_imports(SERVICE, src, &idx), Vec::<String>::new());
    }

    #[test]
    fn a_name_inside_a_longer_identifier_does_not_count_and_a_repository_class_beats_the_jdk() {
        let src = "package com.example.service;\nclass WidgetService { MyOptionalThing x; List y; }\n";
        let idx = index(&[("List", "com.example.domain.List")]);
        assert_eq!(
            missing_imports(SERVICE, src, &idx),
            vec!["com.example.domain.List".to_string()]
        );
    }

    #[test]
    fn the_class_index_comes_from_src_main_java() {
        let dir = tempfile::tempdir().unwrap();
        for rel in [
            "src/main/java/com/example/a/Alpha.java",
            "src/main/java/com/example/b/Alpha.java",
            "src/main/java/com/example/b/Beta.java",
            "src/test/java/com/example/a/GammaTest.java",
        ] {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "class X {}").unwrap();
        }
        let idx = class_index(dir.path());
        assert_eq!(idx["Alpha"].len(), 2);
        assert_eq!(idx["Beta"], vec!["com.example.b.Beta".to_string()]);
        assert!(!idx.contains_key("GammaTest"));
    }

    #[test]
    fn a_file_without_a_package_line_takes_its_package_from_its_path_main_or_test() {
        let idx = index(&[("Greeter", "com.example.Greeter")]);
        assert_eq!(
            missing_imports(
                "src/test/java/com/example/GreeterTest.java",
                "class GreeterTest { Greeter g; }",
                &idx
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            missing_imports(
                "src/test/java/com/other/GreeterTest.java",
                "class GreeterTest { Greeter g; }",
                &idx
            ),
            vec!["com.example.Greeter".to_string()]
        );
    }
}
