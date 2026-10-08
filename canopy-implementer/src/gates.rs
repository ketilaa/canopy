//! What the harness checks before it will write the model's submitted files.
//!
//! Everything here is mechanical and answers a question the harness can settle itself: is every path
//! safe, was every existing file read before being changed, does production code come with a test
//! that mentions it. A rejection is a message for the model; nothing is written.

use std::collections::BTreeSet;
use std::path::Path;

use crate::files::GeneratedFiles;
use crate::implementer::StepKind;
use crate::sandbox::RepoPath;

#[derive(Debug, Clone, PartialEq)]
pub enum Rejection {
    /// A path is unsafe, repeated, or names something that is not a plain file.
    Unsafe(String),
    /// Existing files the model submits changes to without having read them.
    Unread(Vec<String>),
    /// Nothing was submitted.
    Empty,
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
    if kind == StepKind::Refactor {
        return Ok(());
    }

    let changed: Vec<&(String, &str, bool)> = entries
        .iter()
        .filter(|(relative, content, exists)| {
            !*exists || std::fs::read_to_string(root.join(relative)).map_or(true, |on_disk| on_disk != *content)
        })
        .collect();
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
}
