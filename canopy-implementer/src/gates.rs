//! What the harness checks before it will write the model's submitted files.
//!
//! Everything here is mechanical and answers a question the harness can settle itself: is every path
//! safe, was every existing file read before being changed, does production code come with a test
//! that mentions it. A rejection is a message for the model; nothing is written.

use std::collections::BTreeSet;
use std::path::Path;

use crate::files::GeneratedFiles;
use crate::implementer::{Scope, StepKind};
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
    /// Every submitted file is identical to what is already there.
    NoChange,
    /// Production code changed and no test changed with it; holds the changed production paths.
    NoTest(Vec<String>),
    /// Production code changed and no changed test mentions it.
    TestsUnrelated(Vec<String>),
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
    if kind == StepKind::Refactor {
        return Ok(());
    }
    let production_paths: Vec<String> = changed
        .iter()
        .filter(|(relative, _, _)| is_production(relative))
        .map(|(relative, _, _)| relative.clone())
        .collect();
    let production: Vec<String> = production_paths
        .iter()
        .filter_map(|relative| Path::new(relative).file_stem())
        .map(|stem| stem.to_string_lossy().into_owned())
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
    let unrelated: Vec<String> = production
        .into_iter()
        .filter(|class| !tests.iter().any(|test| mentions_word(test, class)))
        .collect();
    if unrelated.is_empty() {
        Ok(())
    } else {
        Err(Rejection::TestsUnrelated(unrelated))
    }
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
fn mentions_word(text: &str, word: &str) -> bool {
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
}
