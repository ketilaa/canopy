//! What the harness checks before it will write the model's submitted files.
//!
//! Everything here is mechanical and answers a question the harness can settle itself: is every path
//! safe, was every existing file read before being changed, does production code come with a test
//! that mentions it. A rejection is a message for the model; nothing is written.

use std::collections::BTreeSet;
use std::path::Path;

use crate::files::{DeclaredImport, GeneratedFiles};
use crate::implementer::{Scope, StepKind};
use crate::java;
use crate::sandbox::RepoPath;

#[derive(Debug, Clone, PartialEq)]
pub enum Rejection {
    /// A path is unsafe, repeated, or names something that is not a plain file.
    Unsafe(String),
    /// Existing files the model submits changes to without having read them.
    Unread(Vec<String>),
    /// Nothing was submitted.
    Empty,
    /// Files outside the step's scope.
    OutOfScope { paths: Vec<String>, scope: Scope },
    /// Files the step must create that were not submitted.
    MissingCreate(Vec<String>),
    /// Files the step must create that already exist.
    CreateExists(Vec<String>),
    /// Files the planner said must change, which the submission leaves identical or omits.
    ModifyUnchanged(Vec<String>),
    /// Every submitted file is identical to what is already there.
    NoChange,
    /// Production code changed and no test changed with it; holds the changed production paths.
    NoTest(Vec<String>),
    /// Production code changed and no changed test mentions it.
    TestsUnrelated(Vec<String>),
    /// Files that use a class from another package without importing it: each file and the qualified names to add.
    MissingImports(Vec<(String, Vec<String>)>),
    /// The submission carries no import checklist although one is required.
    ImportsNotDeclared,
    /// Imports the checklist lists that the file does not contain: each file and the qualified name.
    DeclaredImportAbsent(Vec<(String, String)>),
}

impl Rejection {
    /// What the model is told. Always says that nothing was written and what to do next.
    pub fn message(&self) -> String {
        match self {
            Rejection::Unsafe(detail) => format!("Not submitted: {detail}. Nothing was written."),
            Rejection::Unread(paths) => format!(
                "Not submitted. These files already exist and you have not read them: {}. \
                 Call read_file on each one, then call submit_files again.",
                paths.join(", ")
            ),
            Rejection::OutOfScope { paths, scope } => {
                let list = |items: &[String]| {
                    if items.is_empty() {
                        "nothing".to_string()
                    } else {
                        items.join(", ")
                    }
                };
                format!(
                    "Not submitted: {} not among the files for this step. You may only change {} and create {}. \
                     Submit only those files, each at exactly its path.",
                    if paths.len() == 1 {
                        format!("{} is", paths[0])
                    } else {
                        format!("{} are", paths.join(", "))
                    },
                    list(&scope.modify),
                    list(&scope.create)
                )
            }
            Rejection::ModifyUnchanged(paths) => format!(
                "Not submitted: {} must change in this step, but your submission leaves {} identical to what is \
                 already there or does not include {}. Change {} as the task describes and submit {} with your \
                 other files.",
                paths.join(", "),
                if paths.len() == 1 { "it" } else { "them" },
                if paths.len() == 1 { "it" } else { "them" },
                if paths.len() == 1 { "it" } else { "them" },
                if paths.len() == 1 { "it" } else { "them" },
            ),
            Rejection::MissingCreate(paths) => format!(
                "Not submitted: you did not create {}. This step requires each of them, at exactly that path. \
                 Submit them together with your other files.",
                paths.join(", ")
            ),
            Rejection::CreateExists(paths) => format!(
                "Not submitted: {} already exists, but this step is meant to create it. Change the files you \
                 were given instead of creating it again.",
                paths.join(", ")
            ),
            Rejection::NoChange => "Not submitted: every file you submitted is identical to what is already there, so \
                 nothing would change. The task asks for a change: make it, with a test for it, and call \
                 submit_files with the files that differ."
                .to_string(),
            Rejection::Empty => "Not submitted: you submitted no files. If you have read what you need, write your \
                 change now: call submit_files with the complete content of every file you change, and a test for \
                 the behaviour."
                .to_string(),
            Rejection::NoTest(paths) => {
                let classes: Vec<String> = paths
                    .iter()
                    .filter_map(|p| Path::new(p).file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect();
                format!(
                    "Not submitted. You changed {classes} but submitted no changed test. Submit your change to {paths} \
                     and a test that exercises {classes} together, in one submit_files call. ALWAYS include a test for \
                     every behaviour you change: read the existing test file (find it with glob), add a test for the \
                     behaviour, and submit all the files.",
                    classes = classes.join(", "),
                    paths = paths.join(", ")
                )
            }
            Rejection::ImportsNotDeclared => {
                "Not submitted. submit_files needs the `declared_imports` list: for every file, each \
                 class it uses from another package and its qualified name, written before the files. Send it \
                 again with the `declared_imports` list first."
                    .to_string()
            }
            Rejection::DeclaredImportAbsent(entries) => {
                let lines: Vec<String> = entries
                    .iter()
                    .map(|(path, name)| format!("{path} does not contain import {name};"))
                    .collect();
                format!(
                    "Not submitted. Your imports list and your files disagree: {}. Put the import in the file, \
                     then call submit_files again with all your files.",
                    lines.join("; ")
                )
            }
            Rejection::MissingImports(files) => {
                let lines: Vec<String> = files
                    .iter()
                    .map(|(path, names)| {
                        let imports: Vec<String> = names.iter().map(|n| format!("import {n};")).collect();
                        format!("{path} needs {}", imports.join(" "))
                    })
                    .collect();
                format!(
                    "Not submitted. These files use classes they do not import: {}. Add the lines and call \
                     submit_files again with all your files.",
                    lines.join("; ")
                )
            }
            Rejection::TestsUnrelated(classes) => format!(
                "Not submitted. No changed test mentions {}. Add a test that exercises it, then call \
                 submit_files again with all your files.",
                classes.join(", ")
            ),
        }
    }
}

/// Checks a submission against the files the planner allowed this step to touch.
pub fn check_scope(files: &GeneratedFiles, scope: &Scope, root: &Path) -> Result<(), Rejection> {
    let submitted: Vec<String> = files
        .files
        .iter()
        .map(|f| {
            RepoPath::parse(&f.path)
                .map(|p| p.as_str())
                .unwrap_or_else(|_| f.path.clone())
        })
        .collect();
    let allowed: BTreeSet<&str> = scope.modify.iter().chain(&scope.create).map(String::as_str).collect();
    let outside: Vec<String> = submitted
        .iter()
        .filter(|p| !allowed.contains(p.as_str()))
        .cloned()
        .collect();
    if !outside.is_empty() {
        return Err(Rejection::OutOfScope {
            paths: outside,
            scope: scope.clone(),
        });
    }
    let missing: Vec<String> = scope
        .create
        .iter()
        .filter(|p| !submitted.contains(p))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(Rejection::MissingCreate(missing));
    }
    let existing: Vec<String> = scope
        .create
        .iter()
        .filter(|p| std::fs::symlink_metadata(root.join(p)).is_ok())
        .cloned()
        .collect();
    if !existing.is_empty() {
        return Err(Rejection::CreateExists(existing));
    }
    // The planner lists a file under `modify` because it has to change; one the model leaves alone, or
    // gives back as it was, means the step is not done.
    let unchanged: Vec<String> = scope
        .modify
        .iter()
        .filter(|path| {
            let submitted_content = files
                .files
                .iter()
                .zip(&submitted)
                .find(|(_, normalised)| normalised == path)
                .map(|(file, _)| file.content.as_str());
            match submitted_content {
                None => true,
                Some(content) => std::fs::read_to_string(root.join(path)).is_ok_and(|on_disk| on_disk == content),
            }
        })
        .cloned()
        .collect();
    if !unchanged.is_empty() {
        return Err(Rejection::ModifyUnchanged(unchanged));
    }
    Ok(())
}

/// Checks a submission against the repository at `root`. `has_read` says whether a normalised
/// relative path was successfully read in this attempt.
pub fn check_submission(
    files: &GeneratedFiles,
    root: &Path,
    has_read: &dyn Fn(&str) -> bool,
    kind: StepKind,
) -> Result<(), Rejection> {
    if files.files.is_empty() {
        return Err(Rejection::Empty);
    }
    // (normalised path, content, already exists)
    let mut entries: Vec<(String, &str, bool)> = Vec::new();
    let mut seen = BTreeSet::new();
    for file in &files.files {
        let path = RepoPath::parse(&file.path).map_err(|e| Rejection::Unsafe(format!("`{}`: {e}", file.path)))?;
        let relative = path.as_str();
        if !seen.insert(relative.clone()) {
            return Err(Rejection::Unsafe(format!("`{relative}` appears more than once")));
        }
        let exists = existing_kind(root, &relative)?;
        entries.push((relative, &file.content, exists));
    }

    let unread: Vec<String> = entries
        .iter()
        .filter(|(relative, _, exists)| *exists && !has_read(relative))
        .map(|(relative, _, _)| relative.clone())
        .collect();
    if !unread.is_empty() {
        return Err(Rejection::Unread(unread));
    }
    let changed: Vec<&(String, &str, bool)> = entries
        .iter()
        .filter(|(relative, content, exists)| {
            !*exists || std::fs::read_to_string(root.join(relative)).map_or(true, |on_disk| on_disk != *content)
        })
        .collect();
    if changed.is_empty() {
        return Err(Rejection::NoChange);
    }
    let missing = missing_imports_in(&changed, root);
    if !missing.is_empty() {
        return Err(Rejection::MissingImports(missing));
    }
    if kind == StepKind::Refactor {
        return Ok(());
    }
    let production_paths: Vec<String> = changed
        .iter()
        .filter(|(relative, _, _)| is_production(relative))
        .map(|(relative, _, _)| relative.clone())
        .collect();
    let production: Vec<(String, &str)> = changed
        .iter()
        .filter(|(relative, _, _)| is_production(relative))
        .filter_map(|(relative, content, _)| {
            Path::new(relative)
                .file_stem()
                .map(|stem| (stem.to_string_lossy().into_owned(), *content))
        })
        .collect();
    if production.is_empty() {
        return Ok(());
    }
    let tests: Vec<&str> = changed
        .iter()
        .filter(|(relative, _, _)| relative.starts_with("src/test/"))
        .map(|(_, content, _)| *content)
        .collect();
    if tests.is_empty() {
        return Err(Rejection::NoTest(production_paths));
    }
    let unrelated = uncovered(&production, &tests);
    if unrelated.is_empty() {
        Ok(())
    } else {
        Err(Rejection::TestsUnrelated(unrelated))
    }
}

/// The changed production classes no changed test reaches. A class is reached when a changed test mentions it,
/// or when a changed production class that is itself reached mentions it (a request record the test only
/// exercises through the controller that takes it). Only classes changed in this submission count.
fn uncovered(production: &[(String, &str)], tests: &[&str]) -> Vec<String> {
    let mut covered: Vec<bool> = production
        .iter()
        .map(|(class, _)| tests.iter().any(|test| mentions_word(test, class)))
        .collect();
    loop {
        let mut grew = false;
        for i in 0..production.len() {
            if covered[i] {
                continue;
            }
            let reached = production
                .iter()
                .zip(&covered)
                .any(|((_, content), is_covered)| *is_covered && mentions_word(content, &production[i].0));
            if reached {
                covered[i] = true;
                grew = true;
            }
        }
        if !grew {
            return production
                .iter()
                .zip(&covered)
                .filter(|(_, is_covered)| !**is_covered)
                .map(|((class, _), _)| class.clone())
                .collect();
        }
    }
}

/// Checks the model's import checklist against its files: when one is required it must be there, and every
/// import it lists must be in the file it names (exactly, or through a wildcard of the same package). An entry for
/// a file that was not submitted is ignored. A deterministic audit: nothing is changed.
pub fn check_declared_imports(
    files: &GeneratedFiles,
    declared: Option<&[DeclaredImport]>,
    required: bool,
) -> Result<(), Rejection> {
    let Some(declared) = declared else {
        return if required {
            Err(Rejection::ImportsNotDeclared)
        } else {
            Ok(())
        };
    };
    let mut absent: Vec<(String, String)> = Vec::new();
    for entry in declared {
        let Some(file) = files
            .files
            .iter()
            .find(|f| f.path.trim_start_matches("./") == entry.file.trim_start_matches("./"))
        else {
            continue;
        };
        let imports = java::imports_in(&file.content);
        let package = entry.import.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
        let present = imports.contains(&entry.import) || imports.contains(&format!("{package}.*"));
        let entry_pair = (file.path.clone(), entry.import.clone());
        if !present && !absent.contains(&entry_pair) {
            absent.push(entry_pair);
        }
    }
    if absent.is_empty() {
        Ok(())
    } else {
        Err(Rejection::DeclaredImportAbsent(absent))
    }
}

/// The changed Java files that use a class they do not import, from the repository's classes plus the ones this
/// submission adds. A deterministic audit: it reads the files and never changes them.
fn missing_imports_in(changed: &[&(String, &str, bool)], root: &Path) -> Vec<(String, Vec<String>)> {
    let mut index = java::class_index(root);
    for (relative, _, _) in changed {
        if let (true, Some(qualified), Some(class)) = (
            is_production(relative) && relative.ends_with(".java"),
            java::qualified_name(relative),
            Path::new(relative)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned()),
        ) {
            let entry = index.entry(class).or_default();
            if !entry.contains(&qualified) {
                entry.push(qualified);
            }
        }
    }
    changed
        .iter()
        .filter(|(relative, _, _)| relative.ends_with(".java"))
        .filter_map(|(relative, content, _)| {
            let missing = java::missing_imports(relative, content, &index);
            (!missing.is_empty()).then(|| (relative.clone(), missing))
        })
        .collect()
}

/// Whether `relative` exists, refusing a symlink, a directory, or any symlinked directory on the way.
fn existing_kind(root: &Path, relative: &str) -> Result<bool, Rejection> {
    let mut current = root.to_path_buf();
    let components: Vec<&str> = relative.split('/').collect();
    for (index, component) in components.iter().enumerate() {
        current.push(component);
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            return Ok(false);
        };
        if metadata.file_type().is_symlink() {
            return Err(Rejection::Unsafe(format!(
                "`{relative}` is or passes through a symbolic link"
            )));
        }
        let last = index == components.len() - 1;
        if last && metadata.is_dir() {
            return Err(Rejection::Unsafe(format!("`{relative}` is a directory")));
        }
    }
    Ok(true)
}

/// Java production code: the files whose behaviour a test must cover.
fn is_production(relative: &str) -> bool {
    relative.starts_with("src/main/") && relative.ends_with(".java")
}

/// Whether `word` appears in `text` as a whole identifier, not inside a longer one.
pub(crate) fn mentions_word(text: &str, word: &str) -> bool {
    let is_identifier = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + word.len()..].chars().next();
        !before.is_some_and(is_identifier) && !after.is_some_and(is_identifier)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::GeneratedFile;
    use std::os::unix::fs::symlink;

    const GREETER: &str = "src/main/java/com/example/Greeter.java";
    const GREETER_TEST: &str = "src/test/java/com/example/GreeterTest.java";

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in [
            (GREETER, "package com.example;\npublic class Greeter { }\n"),
            (
                GREETER_TEST,
                "package com.example;\nclass GreeterTest { void t() { new Greeter(); } }\n",
            ),
            ("pom.xml", "<project/>"),
        ] {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    fn files(pairs: &[(&str, &str)]) -> GeneratedFiles {
        GeneratedFiles {
            files: pairs
                .iter()
                .map(|(p, c)| GeneratedFile {
                    path: p.to_string(),
                    content: c.to_string(),
                })
                .collect(),
        }
    }

    fn check(
        dir: &tempfile::TempDir,
        submitted: &[(&str, &str)],
        read: &[&str],
        kind: StepKind,
    ) -> Result<(), Rejection> {
        let read: BTreeSet<String> = read.iter().map(|s| s.to_string()).collect();
        check_submission(&files(submitted), dir.path(), &|p| read.contains(p), kind)
    }

    const NEW_GREETER: &str = "package com.example;\npublic class Greeter { String v2; }\n";
    const NEW_TEST: &str = "package com.example;\nclass GreeterTest { void t2() { new Greeter().v2(); } }\n";

    #[test]
    fn a_change_to_an_existing_file_that_was_read_with_a_changed_test_is_accepted() {
        let dir = repo();
        let r = check(
            &dir,
            &[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)],
            &[GREETER, GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn changing_an_existing_file_without_having_read_it_is_rejected_and_names_exactly_the_unread_ones() {
        let dir = repo();
        let r = check(
            &dir,
            &[(GREETER, NEW_GREETER), (GREETER_TEST, NEW_TEST)],
            &[GREETER],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Err(Rejection::Unread(vec![GREETER_TEST.to_string()])));
        let message = r.unwrap_err().message();
        assert!(
            message.contains(GREETER_TEST) && message.contains("read_file"),
            "{message}"
        );
    }

    #[test]
    fn a_submitted_path_in_another_spelling_is_matched_to_the_file_that_was_read() {
        let dir = repo();
        let r = check(
            &dir,
            &[(&format!("./{GREETER}"), NEW_GREETER), (GREETER_TEST, NEW_TEST)],
            &[GREETER, GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn a_brand_new_file_needs_no_read() {
        let dir = repo();
        let r = check(
            &dir,
            &[
                (
                    "src/main/java/com/example/Farewell.java",
                    "package com.example;\npublic class Farewell { }\n",
                ),
                (
                    "src/test/java/com/example/FarewellTest.java",
                    "class FarewellTest { void t() { new Farewell(); } }\n",
                ),
            ],
            &[],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn unsafe_paths_are_rejected_before_anything_else() {
        let dir = repo();
        for path in ["../outside.txt", "/etc/passwd", ".git/config", "a\\b", ""] {
            let r = check(&dir, &[(path, "x")], &[], StepKind::BehaviorChange);
            assert!(matches!(r, Err(Rejection::Unsafe(_))), "{path:?}: {r:?}");
        }
    }

    #[test]
    fn the_same_file_twice_even_in_different_spellings_is_rejected() {
        let dir = repo();
        let r = check(&dir, &[("a.txt", "1"), ("./a.txt", "2")], &[], StepKind::Refactor);
        assert!(
            matches!(r, Err(Rejection::Unsafe(ref m)) if m.contains("more than once")),
            "{r:?}"
        );
    }

    #[test]
    fn writing_onto_a_symlink_or_a_directory_is_rejected() {
        let dir = repo();
        symlink(dir.path().join("pom.xml"), dir.path().join("alias.xml")).unwrap();
        assert!(matches!(
            check(&dir, &[("alias.xml", "x")], &["alias.xml"], StepKind::Refactor),
            Err(Rejection::Unsafe(_))
        ));
        assert!(matches!(
            check(&dir, &[("src/main", "x")], &["src/main"], StepKind::Refactor),
            Err(Rejection::Unsafe(_))
        ));
    }

    #[test]
    fn an_empty_submission_is_rejected_not_treated_as_success() {
        let dir = repo();
        let r = check(&dir, &[], &[], StepKind::BehaviorChange);
        assert_eq!(r, Err(Rejection::Empty));
        let message = r.unwrap_err().message();
        assert!(
            message.contains("write your change now") && message.contains("complete content"),
            "{message}"
        );
    }

    #[test]
    fn a_new_file_under_a_symlinked_directory_is_rejected() {
        let dir = repo();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), dir.path().join("src/escape")).unwrap();
        let r = check(
            &dir,
            &[("src/escape/New.java", "class New {}")],
            &[],
            StepKind::Refactor,
        );
        assert!(matches!(r, Err(Rejection::Unsafe(_))), "{r:?}");
        assert!(!outside.path().join("New.java").exists());
    }

    #[test]
    fn changed_production_code_without_any_changed_test_is_rejected() {
        let dir = repo();
        let r = check(&dir, &[(GREETER, NEW_GREETER)], &[GREETER], StepKind::BehaviorChange);
        assert_eq!(r, Err(Rejection::NoTest(vec![GREETER.to_string()])));
        let message = r.unwrap_err().message();
        assert!(message.contains("ALWAYS include a test"), "{message}");
        assert!(
            message.contains(GREETER) && message.contains("together"),
            "names the file to keep: {message}"
        );
    }

    #[test]
    fn resubmitting_the_test_file_unchanged_does_not_count_as_a_test() {
        let dir = repo();
        let unchanged_test = std::fs::read_to_string(dir.path().join(GREETER_TEST)).unwrap();
        let r = check(
            &dir,
            &[(GREETER, NEW_GREETER), (GREETER_TEST, &unchanged_test)],
            &[GREETER, GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Err(Rejection::NoTest(vec![GREETER.to_string()])));
    }

    #[test]
    fn a_changed_test_that_never_mentions_the_changed_class_is_rejected() {
        let dir = repo();
        let r = check(
            &dir,
            &[
                (GREETER, NEW_GREETER),
                ("src/test/java/com/example/OtherTest.java", "class OtherTest { }\n"),
            ],
            &[GREETER],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Err(Rejection::TestsUnrelated(vec!["Greeter".to_string()])));
    }

    #[test]
    fn mentioning_the_class_means_the_whole_word_not_a_longer_name() {
        let dir = repo();
        // GreeterTest contains "Greeter" but is not the word Greeter.
        let r = check(
            &dir,
            &[
                (GREETER, NEW_GREETER),
                (GREETER_TEST, "package com.example;\nclass GreeterTest { }\n"),
            ],
            &[GREETER, GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Err(Rejection::TestsUnrelated(vec!["Greeter".to_string()])));
    }

    const REQUEST: &str = "src/main/java/com/example/Request.java";
    const CONTROLLER: &str = "src/main/java/com/example/Controller.java";
    const CONTROLLER_TEST: &str = "src/test/java/com/example/ControllerTest.java";

    #[test]
    fn a_class_used_only_by_a_covered_changed_class_counts_as_covered() {
        let dir = repo();
        let r = check(
            &dir,
            &[
                (REQUEST, "package com.example;\nrecord Request(String name) { }\n"),
                (
                    CONTROLLER,
                    "package com.example;\nclass Controller { void post(Request r) { } }\n",
                ),
                (
                    CONTROLLER_TEST,
                    "package com.example;\nclass ControllerTest { void t() { new Controller(); } }\n",
                ),
            ],
            &[],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn coverage_through_another_class_follows_a_chain() {
        let dir = repo();
        let r = check(
            &dir,
            &[
                (REQUEST, "package com.example;\nrecord Request(String name) { }\n"),
                (
                    "src/main/java/com/example/Mapper.java",
                    "package com.example;\nclass Mapper { Request map() { return null; } }\n",
                ),
                (CONTROLLER, "package com.example;\nclass Controller { Mapper m; }\n"),
                (
                    CONTROLLER_TEST,
                    "package com.example;\nclass ControllerTest { void t() { new Controller(); } }\n",
                ),
            ],
            &[],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn a_class_used_only_by_an_uncovered_changed_class_is_still_unrelated() {
        let dir = repo();
        let r = check(
            &dir,
            &[
                (REQUEST, "package com.example;\nrecord Request(String name) { }\n"),
                (
                    CONTROLLER,
                    "package com.example;\nclass Controller { void post(Request r) { } }\n",
                ),
                ("src/test/java/com/example/OtherTest.java", "class OtherTest { }\n"),
            ],
            &[],
            StepKind::BehaviorChange,
        );
        assert_eq!(
            r,
            Err(Rejection::TestsUnrelated(vec![
                "Request".to_string(),
                "Controller".to_string()
            ]))
        );
    }

    #[test]
    fn a_class_mentioned_only_by_an_unchanged_class_is_not_covered() {
        let dir = repo();
        // Greeter exists on disk and is not in the submission; Request is new and mentioned by nobody changed.
        let r = check(
            &dir,
            &[
                (REQUEST, "package com.example;\nrecord Request(String name) { }\n"),
                (
                    GREETER_TEST,
                    "package com.example;\nclass GreeterTest { void t() { new Greeter(); new Greeter(); } }\n",
                ),
            ],
            &[GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Err(Rejection::TestsUnrelated(vec!["Request".to_string()])));
    }

    const OTHER_PKG: &str = "src/main/java/com/example/other/Holder.java";

    #[test]
    fn a_submitted_file_that_uses_a_jdk_class_without_importing_it_is_refused_with_the_line_to_add() {
        let dir = repo();
        let r = check(
            &dir,
            &[
                (
                    OTHER_PKG,
                    "package com.example.other;\npublic class Holder { Optional<String> o; }\n",
                ),
                (GREETER_TEST, NEW_TEST),
            ],
            &[GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(
            r,
            Err(Rejection::MissingImports(vec![(
                OTHER_PKG.to_string(),
                vec!["java.util.Optional".to_string()]
            )]))
        );
        let message = r.unwrap_err().message();
        assert!(
            message.contains(OTHER_PKG) && message.contains("import java.util.Optional;"),
            "{message}"
        );
    }

    #[test]
    fn a_class_of_the_repository_in_another_package_must_be_imported_too_and_one_created_in_the_same_submission_counts()
    {
        let dir = repo();
        let holder = "package com.example.other;\npublic class Holder { Greeter g; Fresh f; }\n";
        let r = check(
            &dir,
            &[
                (OTHER_PKG, holder),
                (
                    "src/main/java/com/example/fresh/Fresh.java",
                    "package com.example.fresh;\npublic class Fresh { }\n",
                ),
                (GREETER_TEST, NEW_TEST),
            ],
            &[GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(
            r,
            Err(Rejection::MissingImports(vec![(
                OTHER_PKG.to_string(),
                vec!["com.example.Greeter".to_string(), "com.example.fresh.Fresh".to_string()]
            )]))
        );
    }

    #[test]
    fn imports_that_are_there_let_the_submission_through_and_test_files_are_checked_as_well() {
        let dir = repo();
        let ok =
            "package com.example.other;\nimport java.util.Optional;\npublic class Holder { Optional<String> o; }\n";
        assert_eq!(
            check(
                &dir,
                &[
                    (OTHER_PKG, ok),
                    (
                        "src/test/java/com/example/other/HolderTest.java",
                        "package com.example.other;\nclass HolderTest { Holder h; }\n"
                    )
                ],
                &[],
                StepKind::BehaviorChange
            ),
            Ok(())
        );
        let bad_test = "package com.example;\nclass GreeterTest { void t() { new Greeter(); UUID.randomUUID(); } }\n";
        let r = check(
            &dir,
            &[(GREETER, NEW_GREETER), (GREETER_TEST, bad_test)],
            &[GREETER, GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(
            r,
            Err(Rejection::MissingImports(vec![(
                GREETER_TEST.to_string(),
                vec!["java.util.UUID".to_string()]
            )]))
        );
    }

    #[test]
    fn a_refactor_step_is_checked_for_imports_too() {
        let dir = repo();
        let r = check(
            &dir,
            &[(
                GREETER,
                "package com.example;\npublic class Greeter { List<String> l; }\n",
            )],
            &[GREETER],
            StepKind::Refactor,
        );
        assert_eq!(
            r,
            Err(Rejection::MissingImports(vec![(
                GREETER.to_string(),
                vec!["java.util.List".to_string()]
            )]))
        );
    }

    fn declared(pairs: &[(&str, &str)]) -> Vec<crate::files::DeclaredImport> {
        pairs
            .iter()
            .map(|(f, i)| crate::files::DeclaredImport {
                file: f.to_string(),
                import: i.to_string(),
            })
            .collect()
    }

    #[test]
    fn a_required_declaration_that_is_missing_is_refused_and_an_optional_one_is_not() {
        let submitted = files(&[(GREETER, "package com.example;\nclass Greeter { }\n")]);
        assert_eq!(
            check_declared_imports(&submitted, None, true),
            Err(Rejection::ImportsNotDeclared)
        );
        assert!(Rejection::ImportsNotDeclared.message().contains("declared_imports"));
        assert_eq!(check_declared_imports(&submitted, None, false), Ok(()));
    }

    #[test]
    fn a_declared_import_the_file_does_not_contain_is_refused_naming_the_file_and_the_line() {
        let src = "package com.example;\nimport java.util.UUID;\nclass Greeter { }\n";
        let submitted = files(&[(GREETER, src)]);
        let ok = declared(&[(GREETER, "java.util.UUID")]);
        assert_eq!(check_declared_imports(&submitted, Some(&ok), true), Ok(()));
        let wildcard = files(&[(
            GREETER,
            "package com.example;\nimport java.util.*;\nclass Greeter { }\n",
        )]);
        assert_eq!(check_declared_imports(&wildcard, Some(&ok), true), Ok(()));
        let lie = declared(&[(GREETER, "java.util.Optional")]);
        let r = check_declared_imports(&submitted, Some(&lie), true);
        assert_eq!(
            r,
            Err(Rejection::DeclaredImportAbsent(vec![(
                GREETER.to_string(),
                "java.util.Optional".to_string()
            )]))
        );
        let message = r.unwrap_err().message();
        assert!(
            message.contains(GREETER) && message.contains("import java.util.Optional;"),
            "{message}"
        );
    }

    #[test]
    fn an_empty_declaration_is_a_valid_answer_and_a_declaration_for_a_file_not_submitted_is_ignored() {
        let submitted = files(&[(GREETER, "package com.example;\nclass Greeter { }\n")]);
        assert_eq!(check_declared_imports(&submitted, Some(&[]), true), Ok(()));
        let other = declared(&[("src/main/java/com/example/Elsewhere.java", "java.util.UUID")]);
        assert_eq!(check_declared_imports(&submitted, Some(&other), true), Ok(()));
    }

    #[test]
    fn a_refactor_needs_no_new_test() {
        let dir = repo();
        let r = check(&dir, &[(GREETER, NEW_GREETER)], &[GREETER], StepKind::Refactor);
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn changes_to_files_that_are_neither_production_nor_test_code_need_no_test() {
        let dir = repo();
        let r = check(&dir, &[("README.md", "hello")], &[], StepKind::BehaviorChange);
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn a_change_only_to_tests_is_fine() {
        let dir = repo();
        let r = check(
            &dir,
            &[(GREETER_TEST, NEW_TEST)],
            &[GREETER_TEST],
            StepKind::BehaviorChange,
        );
        assert_eq!(r, Ok(()));
    }
    #[test]
    fn a_submission_identical_to_what_is_on_disk_changes_nothing_and_is_rejected() {
        let dir = repo();
        let on_disk = std::fs::read_to_string(dir.path().join(GREETER)).unwrap();
        let r = check(&dir, &[(GREETER, &on_disk)], &[GREETER], StepKind::BehaviorChange);
        assert_eq!(r, Err(Rejection::NoChange));
        let message = r.unwrap_err().message();
        assert!(
            message.contains("identical") && message.contains("nothing would change"),
            "{message}"
        );
    }

    #[test]
    fn a_refactor_must_change_something_too() {
        let dir = repo();
        let on_disk = std::fs::read_to_string(dir.path().join(GREETER)).unwrap();
        assert_eq!(
            check(&dir, &[(GREETER, &on_disk)], &[GREETER], StepKind::Refactor),
            Err(Rejection::NoChange)
        );
    }

    #[test]
    fn one_changed_file_among_identical_ones_is_a_change() {
        let dir = repo();
        let unchanged_test = std::fs::read_to_string(dir.path().join(GREETER_TEST)).unwrap();
        let r = check(
            &dir,
            &[(GREETER, NEW_GREETER), (GREETER_TEST, &unchanged_test)],
            &[GREETER, GREETER_TEST],
            StepKind::Refactor,
        );
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn unread_files_are_still_reported_before_the_no_change_check() {
        let dir = repo();
        let on_disk = std::fs::read_to_string(dir.path().join(GREETER)).unwrap();
        let r = check(&dir, &[(GREETER, &on_disk)], &[], StepKind::BehaviorChange);
        assert_eq!(r, Err(Rejection::Unread(vec![GREETER.to_string()])));
    }
    fn scope(modify: &[&str], create: &[&str]) -> Scope {
        Scope {
            modify: modify.iter().map(|s| s.to_string()).collect(),
            create: create.iter().map(|s| s.to_string()).collect(),
            read_only: vec![],
        }
    }

    const FAREWELL: &str = "src/main/java/com/example/Farewell.java";

    #[test]
    fn a_submission_inside_the_scope_is_fine() {
        let dir = repo();
        let r = check_scope(
            &files(&[
                (GREETER, NEW_GREETER),
                (GREETER_TEST, NEW_TEST),
                (FAREWELL, "class Farewell {}"),
            ]),
            &scope(&[GREETER, GREETER_TEST], &[FAREWELL]),
            dir.path(),
        );
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn a_file_outside_the_scope_is_refused_and_the_message_lists_what_is_allowed() {
        let dir = repo();
        let r = check_scope(
            &files(&[
                (GREETER, NEW_GREETER),
                ("src/main/java/com/other/Invented.java", "class Invented {}"),
            ]),
            &scope(&[GREETER], &[]),
            dir.path(),
        );
        let Err(rejection @ Rejection::OutOfScope { .. }) = r else {
            panic!("{r:?}")
        };
        let message = rejection.message();
        assert!(
            message.contains("src/main/java/com/other/Invented.java") && message.contains("not among the files"),
            "{message}"
        );
        assert!(message.contains(GREETER), "the allowed files are named: {message}");
    }

    #[test]
    fn a_path_in_another_spelling_still_matches_the_scope() {
        let dir = repo();
        let r = check_scope(
            &files(&[(&format!("./{GREETER}"), NEW_GREETER)]),
            &scope(&[GREETER], &[]),
            dir.path(),
        );
        assert_eq!(r, Ok(()));
    }

    #[test]
    fn a_file_the_step_must_create_but_the_submission_lacks_is_refused() {
        let dir = repo();
        let r = check_scope(
            &files(&[(GREETER, NEW_GREETER)]),
            &scope(&[GREETER], &[FAREWELL]),
            dir.path(),
        );
        assert_eq!(r, Err(Rejection::MissingCreate(vec![FAREWELL.to_string()])));
        let message = r.unwrap_err().message();
        assert!(
            message.contains("did not create") && message.contains(FAREWELL),
            "{message}"
        );
    }

    #[test]
    fn a_file_the_step_must_create_that_already_exists_is_refused() {
        let dir = repo();
        let r = check_scope(&files(&[(GREETER, NEW_GREETER)]), &scope(&[], &[GREETER]), dir.path());
        assert_eq!(r, Err(Rejection::CreateExists(vec![GREETER.to_string()])));
        assert!(r.unwrap_err().message().contains("already exists"));
    }
    #[test]
    fn a_file_the_planner_said_must_change_but_the_submission_leaves_identical_is_refused() {
        let dir = repo();
        let unchanged_test = std::fs::read_to_string(dir.path().join(GREETER_TEST)).unwrap();
        let r = check_scope(
            &files(&[(GREETER, NEW_GREETER), (GREETER_TEST, &unchanged_test)]),
            &scope(&[GREETER, GREETER_TEST], &[]),
            dir.path(),
        );
        assert_eq!(r, Err(Rejection::ModifyUnchanged(vec![GREETER_TEST.to_string()])));
        let message = r.unwrap_err().message();
        assert!(
            message.contains("must change") && message.contains(GREETER_TEST),
            "{message}"
        );
    }

    #[test]
    fn a_file_the_planner_said_must_change_but_the_submission_omits_is_refused_too() {
        let dir = repo();
        let r = check_scope(
            &files(&[(GREETER, NEW_GREETER)]),
            &scope(&[GREETER, GREETER_TEST], &[]),
            dir.path(),
        );
        assert_eq!(r, Err(Rejection::ModifyUnchanged(vec![GREETER_TEST.to_string()])));
    }

    #[test]
    fn the_modify_check_comes_after_the_scope_and_create_checks() {
        let dir = repo();
        let r = check_scope(
            &files(&[(GREETER, NEW_GREETER), ("src/other/Extra.java", "class Extra {}")]),
            &scope(&[GREETER, GREETER_TEST], &[]),
            dir.path(),
        );
        assert!(matches!(r, Err(Rejection::OutOfScope { .. })), "{r:?}");
    }
}
