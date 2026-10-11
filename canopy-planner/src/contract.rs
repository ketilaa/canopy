//! The repository contract: decisions a person made about one repository, stable and not derivable from it
//! (which annotation a `*Service` class carries, where data is stored, how values are validated).
//!
//! It lives in the repository (`.canopy/contract.yaml`), so it is versioned with the code it describes and
//! differs per repository without a change to the harness. The harness only reads it, renders it into the
//! facts, and audits the repository against it. Anything that can be computed from the repository (the
//! classpath, the existing classes) is a fact, not part of the contract.

use std::path::Path;

use canopy_implementer::java;
use serde::Deserialize;

/// Where the contract is, relative to the repository root.
pub const CONTRACT_PATH: &str = ".canopy/contract.yaml";

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    /// A class whose name ends with `suffix` carries `@annotation`.
    #[serde(default)]
    pub stereotypes: Vec<Stereotype>,
    /// A decision about one topic, as a sentence the model is told.
    #[serde(default)]
    pub conventions: Vec<Convention>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stereotype {
    pub suffix: String,
    /// The annotation without the `@`.
    pub annotation: String,
    /// The decision this came from (an ADR id), when it came from one. Kept for people and for the audit;
    /// the model is not told it.
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Convention {
    pub topic: String,
    pub rule: String,
    #[serde(default)]
    pub source: Option<String>,
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Contract {
    pub fn from_yaml(text: &str) -> Result<Self, String> {
        let contract: Contract = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
        for s in &contract.stereotypes {
            if !is_identifier(&s.suffix) {
                return Err(format!(
                    "stereotype suffix {:?} must be a class-name suffix such as Service",
                    s.suffix
                ));
            }
            if !is_identifier(&s.annotation) {
                return Err(format!(
                    "stereotype annotation {:?} must be the annotation name without the @",
                    s.annotation
                ));
            }
        }
        for c in &contract.conventions {
            if c.topic.trim().is_empty() || c.rule.trim().is_empty() {
                return Err("a convention needs a topic and a rule".to_string());
            }
        }
        Ok(contract)
    }

    /// The contract in the repository at `root`: `None` when there is none, an error naming the file when it
    /// is there and not valid.
    pub fn read(root: &Path) -> Result<Option<Self>, String> {
        let Ok(text) = std::fs::read_to_string(root.join(CONTRACT_PATH)) else {
            return Ok(None);
        };
        Self::from_yaml(&text)
            .map(Some)
            .map_err(|e| format!("{CONTRACT_PATH}: {e}"))
    }

    /// The contract as the lines the model is told, in the order written.
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.stereotypes.is_empty() {
            let listed: Vec<String> = self
                .stereotypes
                .iter()
                .map(|s| format!("*{} is @{}", s.suffix, s.annotation))
                .collect();
            lines.push(format!("Stereotypes by class name: {}.", listed.join(", ")));
        }
        for c in &self.conventions {
            lines.push(format!(
                "Convention ({}): {}.",
                c.topic.trim(),
                c.rule.trim().trim_end_matches('.')
            ));
        }
        lines
    }

    /// Where the repository disagrees with the contract: a production class named with a stereotype suffix that
    /// does not carry the annotation. `files` is every file of the repository under `src`, relative to `root`.
    /// A report for a person, never a rewrite: the contract is not stale just because one class is wrong, and
    /// the class is not wrong just because the contract is old.
    pub fn audit(&self, root: &Path, files: &[String]) -> Vec<String> {
        let mut drift = Vec::new();
        for path in files
            .iter()
            .filter(|f| f.starts_with("src/main/java/") && f.ends_with(".java"))
        {
            let Some(qualified) = java::qualified_name_of_source(path) else {
                continue;
            };
            let class = qualified.rsplit('.').next().unwrap_or_default();
            let Ok(source) = std::fs::read_to_string(root.join(path)) else {
                continue;
            };
            for s in &self.stereotypes {
                if class.ends_with(&s.suffix) && !has_annotation(&source, &s.annotation) {
                    let from = s.source.as_ref().map(|id| format!(" ({id})")).unwrap_or_default();
                    drift.push(format!(
                        "{qualified} is named *{} but is not annotated @{}{from}",
                        s.suffix, s.annotation
                    ));
                }
            }
        }
        drift
    }
}

/// Whether `source` has `@annotation` as a whole name, not the start of a longer one.
fn has_annotation(source: &str, annotation: &str) -> bool {
    let needle = format!("@{annotation}");
    source.match_indices(&needle).any(|(start, found)| {
        !source[start + found.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{scaffold, write};

    const CONTRACT: &str = "
stereotypes:
  - {suffix: Controller, annotation: RestController}
  - {suffix: Service, annotation: Service}
conventions:
  - {topic: storage, rule: keep data in memory in a java.util.concurrent collection}
  - {topic: validation, rule: check values in code and throw IllegalArgumentException}
";

    #[test]
    fn a_contract_lists_stereotypes_and_conventions_in_the_order_written() {
        let contract = Contract::from_yaml(CONTRACT).unwrap();
        assert_eq!(
            contract.stereotypes,
            [
                Stereotype {
                    suffix: "Controller".into(),
                    annotation: "RestController".into(),
                    source: None
                },
                Stereotype {
                    suffix: "Service".into(),
                    annotation: "Service".into(),
                    source: None
                },
            ]
        );
        assert_eq!(contract.conventions[0].topic, "storage");
        assert_eq!(
            contract.conventions[1].rule,
            "check values in code and throw IllegalArgumentException"
        );
    }

    #[test]
    fn every_part_is_optional_and_an_empty_contract_is_valid() {
        let contract = Contract::from_yaml("{}").unwrap();
        assert!(contract.stereotypes.is_empty() && contract.conventions.is_empty());
        assert!(Contract::from_yaml("stereotypes: []").is_ok());
    }

    #[test]
    fn a_contract_that_is_not_valid_is_rejected_with_the_reason() {
        for bad in [
            "stereotypes: [not a map]",
            "conventions:\n  - {topic: storage}",
            "stereotypes:\n  - {suffix: '', annotation: Service}",
            "stereotypes:\n  - {suffix: Service, annotation: '@Service'}",
            "unknown_part: 1",
            "not: [valid",
        ] {
            assert!(Contract::from_yaml(bad).is_err(), "{bad}");
        }
        let e = Contract::from_yaml("stereotypes:\n  - {suffix: Service, annotation: '@Service'}").unwrap_err();
        assert!(e.contains("annotation") && e.contains("@"), "{e}");
    }

    #[test]
    fn the_contract_renders_as_lines_for_the_facts_in_the_order_written() {
        let lines = Contract::from_yaml(CONTRACT).unwrap().render();
        assert_eq!(
            lines,
            [
                "Stereotypes by class name: *Controller is @RestController, *Service is @Service.",
                "Convention (storage): keep data in memory in a java.util.concurrent collection.",
                "Convention (validation): check values in code and throw IllegalArgumentException.",
            ]
        );
        assert!(Contract::default().render().is_empty());
    }

    #[test]
    fn a_contract_is_read_from_the_repository_when_the_file_is_there_and_absent_when_it_is_not() {
        let dir = scaffold();
        assert_eq!(Contract::read(dir.path()), Ok(None));
        write(dir.path(), ".canopy/contract.yaml", CONTRACT);
        assert!(Contract::read(dir.path()).unwrap().is_some());
        write(dir.path(), ".canopy/contract.yaml", "stereotypes: [broken");
        let e = Contract::read(dir.path()).unwrap_err();
        assert!(e.contains(".canopy/contract.yaml"), "{e}");
    }

    fn classes(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<String>) {
        let dir = scaffold();
        for (path, text) in files {
            write(dir.path(), path, text);
        }
        let list = files.iter().map(|(p, _)| p.to_string()).collect();
        (dir, list)
    }

    #[test]
    fn the_audit_reports_a_class_whose_name_has_a_stereotype_suffix_but_not_its_annotation() {
        let contract = Contract::from_yaml(CONTRACT).unwrap();
        let (dir, files) = classes(&[
            (
                "src/main/java/com/example/widgets/WidgetService.java",
                "package x;\n@Service\npublic class WidgetService {}\n",
            ),
            (
                "src/main/java/com/example/widgets/GadgetService.java",
                "package x;\npublic class GadgetService {}\n",
            ),
            (
                "src/main/java/com/example/widgets/WidgetController.java",
                "package x;\n@RestController\nclass WidgetController {}\n",
            ),
            (
                "src/main/java/com/example/widgets/Widget.java",
                "package x;\nrecord Widget() {}\n",
            ),
        ]);
        let drift = contract.audit(dir.path(), &files);
        assert_eq!(drift.len(), 1, "{drift:?}");
        assert_eq!(
            drift[0],
            "com.example.widgets.GadgetService is named *Service but is not annotated @Service"
        );
    }

    #[test]
    fn the_audit_matches_the_annotation_as_a_whole_word_and_ignores_test_classes_and_other_files() {
        let contract = Contract::from_yaml(CONTRACT).unwrap();
        let (dir, files) = classes(&[
            (
                "src/main/java/com/example/widgets/AService.java",
                "package x;\n// mentions @ServiceLocator only\npublic class AService {}\n",
            ),
            (
                "src/test/java/com/example/widgets/AServiceTest.java",
                "package x;\nclass AServiceTest {}\n",
            ),
            ("src/main/resources/application.properties", "a=b"),
        ]);
        let drift = contract.audit(dir.path(), &files);
        assert_eq!(
            drift,
            ["com.example.widgets.AService is named *Service but is not annotated @Service"]
        );
    }

    #[test]
    fn an_entry_can_cite_the_decision_it_came_from_and_the_audit_cites_it_but_the_model_is_not_told() {
        let contract = Contract::from_yaml(
            "stereotypes:\n  - {suffix: Service, annotation: Service, source: adr-004}\nconventions:\n  - {topic: storage, rule: keep data in memory, source: adr-002}\n",
        )
        .unwrap();
        assert_eq!(contract.stereotypes[0].source.as_deref(), Some("adr-004"));
        assert_eq!(contract.conventions[0].source.as_deref(), Some("adr-002"));
        assert!(
            contract.render().iter().all(|l| !l.contains("adr-")),
            "{:?}",
            contract.render()
        );
        let (dir, files) = classes(&[(
            "src/main/java/com/example/widgets/AService.java",
            "package x;\nclass AService {}\n",
        )]);
        assert_eq!(
            contract.audit(dir.path(), &files),
            ["com.example.widgets.AService is named *Service but is not annotated @Service (adr-004)"]
        );
    }

    #[test]
    fn a_contract_with_no_stereotypes_finds_no_drift() {
        let (dir, files) = classes(&[(
            "src/main/java/com/example/widgets/AService.java",
            "package x;\nclass AService {}\n",
        )]);
        assert!(Contract::default().audit(dir.path(), &files).is_empty());
    }
}
