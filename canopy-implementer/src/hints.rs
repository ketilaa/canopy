//! Facts the harness can work out that make a build failure easier to act on.
//!
//! These add to what the model is told; they never change what it wrote.

use std::path::Path;

/// When a failing test belongs to a class the model did not change, say so. A test named `XTest`
/// exercises `X` by convention; if `X` exists in `src/main` and is not among the files just
/// submitted, the failure is most likely a change in the wrong place, which a small model repeats
/// even when told exactly which assertion failed.
pub fn subject_hint(digest: &str, submitted: &[String], root: &Path) -> Option<String> {
    let changed: Vec<&str> = submitted.iter().map(|p| p.trim_start_matches("./")).collect();
    let mut lines: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for line in digest.lines() {
        let Some(test_class) = failing_test_class(line) else {
            continue;
        };
        if seen.contains(&test_class) {
            continue;
        }
        seen.push(test_class.clone());
        let subject = test_class.trim_end_matches("Test");
        let Some(path) = find_production_file(root, subject) else {
            continue;
        };
        if changed.contains(&path.as_str()) {
            continue;
        }
        lines.push(format!(
            "{test_class} tests {subject} ({path}), which you did not change. Either change {subject} so that the \
             test passes, or correct the test."
        ));
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// `NameFormatterTest.trimsTheName:27 expected ...` or `...:17 » NullPointer ...` gives `NameFormatterTest`.
fn failing_test_class(line: &str) -> Option<String> {
    let token = line.split_whitespace().next()?;
    let (class, rest) = token.split_once('.')?;
    let is_identifier = class.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$');
    let starts_upper = class.chars().next().is_some_and(char::is_uppercase);
    (is_identifier && starts_upper && class.len() > "Test".len() && class.ends_with("Test") && !rest.is_empty())
        .then(|| class.to_string())
}

/// The path of `<subject>.java` under src/main/java, as a repository-relative path. Symlinks are skipped.
pub(crate) fn find_production_file(root: &Path, subject: &str) -> Option<String> {
    fn walk(dir: &Path, relative: &str, file_name: &str) -> Option<String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let file_type = entry.file_type().ok()?;
            if file_type.is_symlink() {
                continue;
            }
            let path = format!("{relative}/{name}");
            if file_type.is_dir() {
                if let Some(found) = walk(&entry.path(), &path, file_name) {
                    return Some(found);
                }
            } else if name == file_name {
                return Some(path);
            }
        }
        None
    }
    walk(&root.join("src/main/java"), "src/main/java", &format!("{subject}.java"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for rel in [
            "src/main/java/com/example/greeting/Greeter.java",
            "src/main/java/com/example/greeting/format/NameFormatter.java",
            "src/test/java/com/example/greeting/format/NameFormatterTest.java",
        ] {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "class X {}").unwrap();
        }
        dir
    }

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    const GREETER: &str = "src/main/java/com/example/greeting/Greeter.java";
    const FORMATTER: &str = "src/main/java/com/example/greeting/format/NameFormatter.java";

    #[test]
    fn a_failing_test_whose_subject_was_not_changed_gets_a_hint_naming_the_subject_and_its_path() {
        let dir = repo();
        let digest = "NameFormatterTest.trimsTheName:27 expected: <Ada> but was: <  ada  >\nTests run: 10, Failures: 1";
        let hint = subject_hint(digest, &paths(&[GREETER]), dir.path()).expect("a hint");
        assert!(hint.contains("NameFormatterTest"), "{hint}");
        assert!(hint.contains("NameFormatter") && hint.contains(FORMATTER), "{hint}");
        assert!(hint.contains("did not change"), "{hint}");
    }

    #[test]
    fn no_hint_when_the_subject_was_among_the_changed_files() {
        let dir = repo();
        let digest = "NameFormatterTest.trimsTheName:27 expected: <Ada> but was: <  ada  >";
        assert_eq!(subject_hint(digest, &paths(&[GREETER, FORMATTER]), dir.path()), None);
    }

    #[test]
    fn no_hint_when_the_test_has_no_production_class_of_that_name() {
        let dir = repo();
        assert_eq!(
            subject_hint(
                "OrphanTest.x:3 expected: <1> but was: <2>",
                &paths(&[GREETER]),
                dir.path()
            ),
            None
        );
    }

    #[test]
    fn errors_reported_with_the_exception_arrow_are_recognised_too() {
        let dir = repo();
        let digest = "GreeterTest.greetsGuestWhenTheNameIsNull:17 » NullPointer Cannot invoke \"String.trim()\"";
        let hint = subject_hint(digest, &paths(&[FORMATTER]), dir.path()).expect("a hint");
        assert!(hint.contains("GreeterTest") && hint.contains(GREETER), "{hint}");
    }

    #[test]
    fn several_failing_tests_give_one_line_per_unchanged_subject_and_none_for_changed_ones() {
        let dir = repo();
        let digest =
            "NameFormatterTest.a:1 expected: <1> but was: <2>\nNameFormatterTest.b:2 expected: <1> but was: <2>\n\
                      GreeterTest.c:3 expected: <1> but was: <2>";
        let hint = subject_hint(digest, &paths(&[GREETER]), dir.path()).expect("a hint");
        assert_eq!(
            hint.matches("NameFormatterTest").count(),
            1,
            "one line per subject: {hint}"
        );
        assert!(!hint.contains("GreeterTest"), "Greeter was changed: {hint}");
    }

    #[test]
    fn lines_that_are_not_failing_tests_give_no_hint() {
        let dir = repo();
        assert_eq!(
            subject_hint(
                "Tests run: 10, Failures: 1, Errors: 0, Skipped: 0",
                &paths(&[GREETER]),
                dir.path()
            ),
            None
        );
        assert_eq!(
            subject_hint(
                "src/main/A.java:3:4: cannot find symbol",
                &paths(&[GREETER]),
                dir.path()
            ),
            None
        );
        assert_eq!(subject_hint("", &paths(&[GREETER]), dir.path()), None);
    }
}
