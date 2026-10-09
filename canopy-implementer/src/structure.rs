//! Checks that a finished change has the shape the plan asked for.
//!
//! A hidden behavioural test cannot tell whether the layers exist and are connected the way the plan
//! intended, because a model can satisfy behaviour by putting everything in one class. These checks are
//! mechanical: a file with a given name exists, and one file does or does not mention another type.

use std::path::Path;

use crate::gates::mentions_word;
use crate::hints;

/// One structural expectation. Files are found by name anywhere under `src/main/java`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Check {
    /// A file with this name exists.
    Exists(&'static str),
    /// The named file mentions this identifier as a whole word.
    Mentions(&'static str, &'static str),
    /// The named file does not mention this identifier.
    Lacks(&'static str, &'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    pub description: String,
    pub passed: bool,
}

pub fn run(root: &Path, checks: &[Check]) -> Vec<CheckResult> {
    checks
        .iter()
        .map(|check| match *check {
            Check::Exists(file) => CheckResult {
                description: format!("{file} exists"),
                passed: find(root, file).is_some(),
            },
            Check::Mentions(file, word) => {
                let description = format!("{file} mentions {word}");
                match read(root, file) {
                    Some(text) => CheckResult {
                        description,
                        passed: mentions_word(&text, word),
                    },
                    None => not_found(description),
                }
            }
            Check::Lacks(file, word) => {
                let description = format!("{file} does not mention {word}");
                match read(root, file) {
                    Some(text) => CheckResult {
                        description,
                        passed: !mentions_word(&text, word),
                    },
                    None => not_found(description),
                }
            }
        })
        .collect()
}

fn not_found(description: String) -> CheckResult {
    CheckResult {
        description: format!("{description} (file not found)"),
        passed: false,
    }
}

fn find(root: &Path, file: &str) -> Option<String> {
    hints::find_production_file(root, file.strip_suffix(".java")?)
}

fn read(root: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(root.join(find(root, file)?)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in [
            (
                "src/main/java/app/domain/Widget.java",
                "public record Widget(UUID id, String name) {}",
            ),
            (
                "src/main/java/app/service/WidgetService.java",
                "public class WidgetService { private final WidgetRepository repository; }",
            ),
            (
                "src/main/java/app/web/WidgetController.java",
                "public class WidgetController { private final WidgetService service; }",
            ),
            (
                "src/main/java/app/web/LeakyController.java",
                "public class LeakyController { private final WidgetService service; WidgetRepository repository; }",
            ),
            (
                "src/main/java/app/web/Fuzzy.java",
                "class Fuzzy { WidgetServiceImpl impl; }",
            ),
        ] {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    fn passed(results: &[CheckResult]) -> Vec<bool> {
        results.iter().map(|r| r.passed).collect()
    }

    #[test]
    fn exists_finds_a_file_by_name_wherever_it_is_under_src_main() {
        let r = run(
            repo().path(),
            &[Check::Exists("Widget.java"), Check::Exists("Missing.java")],
        );
        assert_eq!(passed(&r), [true, false]);
        assert_eq!(r[0].description, "Widget.java exists");
        assert_eq!(r[1].description, "Missing.java exists");
    }

    #[test]
    fn mentions_requires_the_whole_identifier_not_a_longer_name() {
        let dir = repo();
        let r = run(
            dir.path(),
            &[
                Check::Mentions("WidgetService.java", "WidgetRepository"),
                Check::Mentions("WidgetController.java", "WidgetService"),
                Check::Mentions("Fuzzy.java", "WidgetService"),
            ],
        );
        assert_eq!(
            passed(&r),
            [true, true, false],
            "WidgetServiceImpl is not WidgetService"
        );
        assert_eq!(r[0].description, "WidgetService.java mentions WidgetRepository");
    }

    #[test]
    fn lacks_passes_when_absent_and_fails_when_present() {
        let dir = repo();
        let r = run(
            dir.path(),
            &[
                Check::Lacks("WidgetController.java", "WidgetRepository"),
                Check::Lacks("LeakyController.java", "WidgetRepository"),
            ],
        );
        assert_eq!(passed(&r), [true, false]);
        assert_eq!(
            r[0].description,
            "WidgetController.java does not mention WidgetRepository"
        );
    }

    #[test]
    fn a_mention_or_absence_check_on_a_missing_file_fails_and_says_why() {
        let r = run(
            repo().path(),
            &[Check::Mentions("Nope.java", "X"), Check::Lacks("Nope.java", "X")],
        );
        assert_eq!(passed(&r), [false, false]);
        assert!(r.iter().all(|c| c.description.contains("file not found")), "{r:?}");
    }

    #[test]
    fn no_checks_means_no_results() {
        assert!(run(repo().path(), &[]).is_empty());
    }
}
