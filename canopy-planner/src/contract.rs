//! The repository contract: decisions a person made about one repository, stable and not derivable from it
//! (which annotation a `*Service` class carries, where data is stored, how values are validated).
//!
//! It lives in the repository (`.canopy/contract.yaml`), so it is versioned with the code it describes and
//! differs per repository without a change to the harness. The harness reads it, routes each entry to the
//! model request that needs it, and audits the repository against it. Anything computable from the repository
//! (the classpath, the existing classes) is a fact, not part of the contract.
//!
//! A *layer* says how a class is recognised (a name suffix, optionally the annotation it carries). A *rule* is
//! a sentence the model is told, and `applies_to` says to whom (the planner or the implementer) and for which
//! layers. A request is given only the rules that apply to it; when the layers of a step cannot be worked out,
//! it is given all the rules for its consumer rather than none.
//!
//! Layers are recognised by name first, and by the annotation a class already carries when its source is
//! known. Other signals (the Roots graph, the build graph) can be added to [`Contract::layers_of`] without
//! changing the format.

use std::collections::BTreeSet;
use std::path::Path;

use canopy_implementer::java;
use serde::Deserialize;

/// Where the contract is, relative to the repository root.
pub const CONTRACT_PATH: &str = ".canopy/contract.yaml";

/// The layer every test class belongs to, besides the layer of what it tests. Always valid in `applies_to`.
pub const TEST_LAYER: &str = "test";

/// Who a rule is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Consumer {
    Planner,
    Implementer,
    #[default]
    Both,
}

impl Consumer {
    fn includes(self, wanted: Consumer) -> bool {
        self == Consumer::Both || self == wanted
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliesTo {
    #[serde(default)]
    pub consumer: Consumer,
    /// Layer names; empty means any layer.
    #[serde(default)]
    pub layers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub name: String,
    /// A class whose name ends with this is in the layer.
    pub suffix: String,
    /// The annotation without the `@` that a class in the layer carries.
    #[serde(default)]
    pub annotation: Option<String>,
    /// The decision this came from (an ADR id): for people and the audit, not shown to the model.
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    #[serde(default)]
    pub applies_to: AppliesTo,
    /// What the model is told, one sentence per line.
    pub content: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contract {
    pub layers: Vec<Layer>,
    pub rules: Vec<Rule>,
}

/// What the file says. The first format (`stereotypes` and `conventions`) is still read, as layers and rules
/// that apply everywhere.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    layers: Vec<Layer>,
    #[serde(default)]
    rules: Vec<Rule>,
    #[serde(default)]
    stereotypes: Vec<Stereotype>,
    #[serde(default)]
    conventions: Vec<Convention>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stereotype {
    suffix: String,
    annotation: String,
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Convention {
    topic: String,
    rule: String,
    #[serde(default)]
    source: Option<String>,
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_layer_name(text: &str) -> bool {
    text.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// A rule's sentences as one line, ending with a full stop.
fn one_line(content: &str) -> String {
    let joined = content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    match joined.ends_with('.') {
        true => joined,
        false => format!("{joined}."),
    }
}

impl Contract {
    pub fn from_yaml(text: &str) -> Result<Self, String> {
        let raw: Raw = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
        let mut contract = Contract {
            layers: raw.layers,
            rules: raw.rules,
        };
        for s in raw.stereotypes {
            contract.layers.push(Layer {
                name: s.suffix.to_lowercase(),
                suffix: s.suffix,
                annotation: Some(s.annotation),
                source: s.source,
            });
        }
        for c in raw.conventions {
            contract.rules.push(Rule {
                id: c.topic,
                applies_to: AppliesTo::default(),
                content: c.rule,
                source: c.source,
            });
        }
        contract.validate()?;
        Ok(contract)
    }

    fn validate(&self) -> Result<(), String> {
        let mut names = BTreeSet::new();
        for l in &self.layers {
            if !is_layer_name(&l.name) || l.name == TEST_LAYER {
                return Err(format!(
                    "layer name {:?} must be lowercase letters, digits or _, and not {TEST_LAYER:?}",
                    l.name
                ));
            }
            if !names.insert(l.name.as_str()) {
                return Err(format!("layer {:?} is declared twice", l.name));
            }
            if !is_identifier(&l.suffix) {
                return Err(format!(
                    "layer {:?}: suffix {:?} must be a class-name suffix such as Service",
                    l.name, l.suffix
                ));
            }
            if let Some(a) = &l.annotation {
                if !is_identifier(a) {
                    return Err(format!(
                        "layer {:?}: annotation {a:?} must be the annotation name without the @",
                        l.name
                    ));
                }
            }
        }
        let mut ids = BTreeSet::new();
        for r in &self.rules {
            if r.id.trim().is_empty() || r.content.trim().is_empty() {
                return Err(format!("rule {:?} needs an id and content", r.id));
            }
            if !ids.insert(r.id.as_str()) {
                return Err(format!("rule {:?} is declared twice", r.id));
            }
            for layer in &r.applies_to.layers {
                if layer != TEST_LAYER && !names.contains(layer.as_str()) {
                    return Err(format!(
                        "rule {:?} applies to the layer {layer:?}, which is not declared",
                        r.id
                    ));
                }
            }
        }
        Ok(())
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

    /// The layers of one class, by name and, when its source is known, by the annotation it carries. A test
    /// class is in the layer of what it tests and in `test`. Empty when nothing says: the class is unclassified.
    pub fn layers_of(&self, qualified: &str, source: Option<&str>) -> BTreeSet<String> {
        let simple = qualified.rsplit('.').next().unwrap_or(qualified);
        let is_test = simple.ends_with("Test");
        let subject = simple.strip_suffix("Test").unwrap_or(simple);
        let mut found = BTreeSet::new();
        for layer in &self.layers {
            let by_name = subject.ends_with(&layer.suffix);
            let by_annotation = !is_test
                && layer
                    .annotation
                    .as_deref()
                    .zip(source)
                    .is_some_and(|(annotation, source)| has_annotation(source, annotation));
            if by_name || by_annotation {
                found.insert(layer.name.clone());
            }
        }
        if is_test {
            found.insert(TEST_LAYER.to_string());
        }
        found
    }

    /// Whether a layer of the code itself is known. `test` alone does not count: a record and its test have no
    /// known layer, and a step with none gets every rule for its consumer.
    fn classified(layers: &BTreeSet<String>) -> bool {
        layers.iter().any(|l| l != TEST_LAYER)
    }

    fn rules_for(&self, consumer: Consumer, layers: &BTreeSet<String>) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| r.applies_to.consumer.includes(consumer))
            .filter(|r| {
                consumer == Consumer::Planner
                    || r.applies_to.layers.is_empty()
                    || !Self::classified(layers)
                    || r.applies_to.layers.iter().any(|l| layers.contains(l))
            })
            .collect()
    }

    /// The ids of the rules a request for `consumer` about a step in `layers` is given.
    pub fn rule_ids_for(&self, consumer: Consumer, layers: &BTreeSet<String>) -> Vec<String> {
        self.rules_for(consumer, layers)
            .into_iter()
            .map(|r| r.id.clone())
            .collect()
    }

    fn rule_line(rule: &Rule) -> String {
        format!("Convention ({}): {}", rule.id.trim(), one_line(&rule.content))
    }

    /// Everything, for a person reading the facts: the layers with annotations, then every rule.
    pub fn render(&self) -> Vec<String> {
        let mut lines = self.stereotype_line().into_iter().collect::<Vec<_>>();
        lines.extend(self.rules.iter().map(Self::rule_line));
        lines
    }

    fn stereotype_line(&self) -> Option<String> {
        let listed: Vec<String> = self
            .layers
            .iter()
            .filter_map(|l| l.annotation.as_ref().map(|a| format!("*{} is @{a}", l.suffix)))
            .collect();
        (!listed.is_empty()).then(|| format!("Stereotypes by class name: {}.", listed.join(", ")))
    }

    /// What one model request is told. The planner gets the stereotype line and the rules meant for it. An
    /// implementer step gets the stereotype of each of its layers and the rules for its layers and for every
    /// layer; a step with no known layer gets all of them for its consumer (noise over a missing rule).
    pub fn render_for(&self, consumer: Consumer, layers: &BTreeSet<String>) -> Vec<String> {
        let mut lines = Vec::new();
        match consumer {
            Consumer::Planner => lines.extend(self.stereotype_line()),
            _ => {
                for l in &self.layers {
                    if let Some(a) = &l.annotation {
                        if !Self::classified(layers) || layers.contains(&l.name) {
                            lines.push(format!("Stereotype: a class named *{} is @{a}.", l.suffix));
                        }
                    }
                }
            }
        }
        lines.extend(self.rules_for(consumer, layers).into_iter().map(Self::rule_line));
        lines
    }

    /// Where the repository disagrees with the contract: a production class named for a layer that has an
    /// annotation, without it. `files` is every file of the repository under `src`, relative to `root`. A report
    /// for a person, never a rewrite: the contract is not stale because one class is wrong.
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
            for layer in &self.layers {
                let Some(annotation) = &layer.annotation else { continue };
                if class.ends_with(&layer.suffix) && !has_annotation(&source, annotation) {
                    let from = layer.source.as_ref().map(|id| format!(" ({id})")).unwrap_or_default();
                    drift.push(format!(
                        "{qualified} is named *{} but is not annotated @{annotation}{from}",
                        layer.suffix
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
    use std::collections::BTreeSet;

    const V2: &str = "
layers:
  - {name: controller, suffix: Controller, annotation: RestController}
  - {name: service, suffix: Service, annotation: Service}
rules:
  - id: spring-controller
    applies_to: {consumer: implementer, layers: [controller]}
    content: |
      Use @RestController.
      Use constructor injection.
  - id: naming
    applies_to: {consumer: planner}
    content: Name a class after its layer, such as WidgetService.
  - id: records
    content: A record's accessor is name(), not getName().
";

    const V1: &str = "
stereotypes:
  - {suffix: Controller, annotation: RestController}
  - {suffix: Service, annotation: Service}
conventions:
  - {topic: storage, rule: keep data in memory in a java.util.concurrent collection}
  - {topic: validation, rule: check values in code and throw IllegalArgumentException}
";

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_contract_has_layers_and_rules_with_where_each_rule_applies() {
        let c = Contract::from_yaml(V2).unwrap();
        assert_eq!(c.layers.len(), 2);
        assert_eq!(c.layers[0].name, "controller");
        assert_eq!(c.layers[0].annotation.as_deref(), Some("RestController"));
        let ids: Vec<&str> = c.rules.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["spring-controller", "naming", "records"]);
        assert_eq!(c.rules[0].applies_to.consumer, Consumer::Implementer);
        assert_eq!(c.rules[0].applies_to.layers, ["controller"]);
        assert_eq!(c.rules[1].applies_to.consumer, Consumer::Planner);
        assert!(c.rules[1].applies_to.layers.is_empty(), "no layers means any");
        assert_eq!(c.rules[2].applies_to.consumer, Consumer::Both, "both is the default");
        assert_eq!(
            c.rules[0].content.trim(),
            "Use @RestController.\nUse constructor injection."
        );
    }

    #[test]
    fn the_first_format_still_reads_as_layers_and_rules_that_apply_everywhere() {
        let c = Contract::from_yaml(V1).unwrap();
        assert_eq!(c.layers[0].name, "controller");
        assert_eq!(c.layers[1].annotation.as_deref(), Some("Service"));
        assert_eq!(c.rules[0].id, "storage");
        assert_eq!(c.rules[0].applies_to, AppliesTo::default());
        assert_eq!(
            c.render(),
            [
                "Stereotypes by class name: *Controller is @RestController, *Service is @Service.",
                "Convention (storage): keep data in memory in a java.util.concurrent collection.",
                "Convention (validation): check values in code and throw IllegalArgumentException.",
            ]
        );
    }

    #[test]
    fn every_part_is_optional_and_an_empty_contract_is_valid() {
        let c = Contract::from_yaml("{}").unwrap();
        assert!(c.layers.is_empty() && c.rules.is_empty());
        assert!(c.render().is_empty());
    }

    #[test]
    fn a_contract_that_is_not_valid_is_rejected_with_the_reason() {
        for (bad, why) in [
            ("layers: [not a map]", ""),
            ("layers:\n  - {name: Service, suffix: Service}", "layer name"),
            ("layers:\n  - {name: service, suffix: ''}", "suffix"),
            (
                "layers:\n  - {name: service, suffix: Service, annotation: '@Service'}",
                "annotation",
            ),
            (
                "layers:\n  - {name: service, suffix: Service}\n  - {name: service, suffix: Svc}",
                "twice",
            ),
            ("rules:\n  - {id: a, content: x}\n  - {id: a, content: y}", "twice"),
            ("rules:\n  - {id: a, content: ''}", "content"),
            (
                "rules:\n  - {id: a, content: x, applies_to: {layers: [nowhere]}}",
                "nowhere",
            ),
            ("unknown_part: 1", ""),
            ("not: [valid", ""),
        ] {
            let e = Contract::from_yaml(bad).unwrap_err();
            assert!(e.contains(why), "{bad}: {e}");
        }
        assert!(
            Contract::from_yaml("rules:\n  - {id: a, content: x, applies_to: {layers: [test]}}").is_ok(),
            "test is a reserved layer"
        );
    }

    #[test]
    fn a_contract_is_read_from_the_repository_when_the_file_is_there_and_absent_when_it_is_not() {
        let dir = scaffold();
        assert_eq!(Contract::read(dir.path()), Ok(None));
        write(dir.path(), ".canopy/contract.yaml", V2);
        assert!(Contract::read(dir.path()).unwrap().is_some());
        write(dir.path(), ".canopy/contract.yaml", "layers: [broken");
        assert!(Contract::read(dir.path())
            .unwrap_err()
            .contains(".canopy/contract.yaml"));
    }

    #[test]
    fn a_class_gets_its_layer_from_its_name_and_from_the_annotation_it_carries_when_its_source_is_known() {
        let c = Contract::from_yaml(V2).unwrap();
        assert_eq!(c.layers_of("com.example.WidgetService", None), set(&["service"]));
        assert_eq!(
            c.layers_of("com.example.Widget", None),
            set(&[]),
            "no suffix, no annotation known: unclassified"
        );
        // A name that does not say what it is: the annotation does, when the class already exists.
        assert_eq!(
            c.layers_of(
                "com.example.WidgetManager",
                Some("package x;\n@Service\nclass WidgetManager {}\n")
            ),
            set(&["service"])
        );
        assert_eq!(
            c.layers_of("com.example.WidgetManager", Some("class WidgetManager {}")),
            set(&[])
        );
        assert_eq!(
            c.layers_of("com.example.WidgetServiceTest", None),
            set(&["service", "test"]),
            "a test class belongs to the layer of what it tests, and to test"
        );
        assert_eq!(c.layers_of("com.example.WidgetTest", None), set(&["test"]));
    }

    #[test]
    fn the_planner_is_told_the_layers_and_only_the_rules_meant_for_it() {
        let lines = Contract::from_yaml(V2)
            .unwrap()
            .render_for(Consumer::Planner, &set(&[]));
        assert_eq!(
            lines,
            [
                "Stereotypes by class name: *Controller is @RestController, *Service is @Service.",
                "Convention (naming): Name a class after its layer, such as WidgetService.",
                "Convention (records): A record's accessor is name(), not getName().",
            ]
        );
    }

    #[test]
    fn a_step_is_told_only_the_rules_for_its_layers_and_the_rules_that_apply_everywhere() {
        let c = Contract::from_yaml(V2).unwrap();
        let service = c.render_for(Consumer::Implementer, &set(&["service"]));
        assert!(!service.iter().any(|l| l.contains("spring-controller")), "{service:?}");
        assert!(
            service.iter().any(|l| l.contains("records")),
            "an any-layer rule still applies: {service:?}"
        );
        assert!(
            service
                .iter()
                .any(|l| l == "Stereotype: a class named *Service is @Service."),
            "{service:?}"
        );
        assert!(!service.iter().any(|l| l.contains("RestController")), "{service:?}");
        assert!(
            !service.iter().any(|l| l.contains("Convention (naming)")),
            "planner-only rule: {service:?}"
        );

        let controller = c.render_for(Consumer::Implementer, &set(&["controller"]));
        assert!(
            controller
                .iter()
                .any(|l| l == "Convention (spring-controller): Use @RestController. Use constructor injection."),
            "{controller:?}"
        );
        assert!(controller
            .iter()
            .any(|l| l == "Stereotype: a class named *Controller is @RestController."));
    }

    #[test]
    fn a_step_whose_layers_are_unknown_gets_every_implementer_rule_rather_than_none() {
        let c = Contract::from_yaml(V2).unwrap();
        let lines = c.render_for(Consumer::Implementer, &set(&[]));
        assert!(lines.iter().any(|l| l.contains("spring-controller")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("records")));
        assert!(lines
            .iter()
            .any(|l| l == "Stereotype: a class named *Service is @Service."));
        assert!(
            !lines.iter().any(|l| l.contains("Convention (naming)")),
            "still not the planner's: {lines:?}"
        );
    }

    #[test]
    fn a_step_that_only_has_the_test_layer_is_unclassified_and_gets_every_implementer_rule() {
        let c = Contract::from_yaml(V2).unwrap();
        // A plain record and its test: the test layer is there, but no layer of the code is known.
        let only_test = c.render_for(Consumer::Implementer, &set(&["test"]));
        assert!(
            only_test.iter().any(|l| l.contains("spring-controller")),
            "{only_test:?}"
        );
        assert!(only_test.iter().any(|l| l.contains("records")));
        assert!(only_test
            .iter()
            .any(|l| l == "Stereotype: a class named *Service is @Service."));
        assert_eq!(
            c.rule_ids_for(Consumer::Implementer, &set(&["test"])),
            ["spring-controller", "records"]
        );
        // Once a layer of the code is known, the step is scoped again, and `test` rules still apply.
        let scoped = c.render_for(Consumer::Implementer, &set(&["service", "test"]));
        assert!(!scoped.iter().any(|l| l.contains("spring-controller")), "{scoped:?}");
    }

    #[test]
    fn a_rule_for_the_test_layer_applies_to_steps_that_have_tests() {
        let c = Contract::from_yaml(
            "layers:\n  - {name: service, suffix: Service}\nrules:\n  - {id: test-style, applies_to: {layers: [test]}, content: Keep tests small.}\n  - {id: service-only, applies_to: {layers: [service]}, content: Be stateless.}\n",
        )
        .unwrap();
        assert_eq!(
            c.rule_ids_for(Consumer::Implementer, &set(&["service", "test"])),
            ["test-style", "service-only"]
        );
        assert_eq!(
            c.rule_ids_for(Consumer::Implementer, &set(&["service"])),
            ["service-only"]
        );
    }

    #[test]
    fn the_rules_for_a_consumer_and_layers_are_listed_by_id_for_the_record() {
        let c = Contract::from_yaml(V2).unwrap();
        assert_eq!(c.rule_ids_for(Consumer::Implementer, &set(&["service"])), ["records"]);
        assert_eq!(
            c.rule_ids_for(Consumer::Implementer, &set(&["controller"])),
            ["spring-controller", "records"]
        );
        assert_eq!(c.rule_ids_for(Consumer::Planner, &set(&[])), ["naming", "records"]);
    }

    fn classes(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<String>) {
        let dir = scaffold();
        for (path, text) in files {
            write(dir.path(), path, text);
        }
        (dir, files.iter().map(|(p, _)| p.to_string()).collect())
    }

    #[test]
    fn the_audit_reports_a_class_named_for_a_layer_that_lacks_the_layers_annotation() {
        let c = Contract::from_yaml(V2).unwrap();
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
                "src/main/java/com/example/widgets/AService.java",
                "package x;\n// @ServiceLocator only\nclass AService {}\n",
            ),
            (
                "src/test/java/com/example/widgets/GadgetServiceTest.java",
                "package x;\nclass GadgetServiceTest {}\n",
            ),
        ]);
        assert_eq!(
            c.audit(dir.path(), &files),
            [
                "com.example.widgets.GadgetService is named *Service but is not annotated @Service",
                "com.example.widgets.AService is named *Service but is not annotated @Service",
            ]
        );
    }

    #[test]
    fn an_entry_can_cite_its_decision_and_the_audit_cites_it_but_the_model_is_not_told() {
        let c = Contract::from_yaml(
            "layers:\n  - {name: service, suffix: Service, annotation: Service, source: adr-004}\nrules:\n  - {id: storage, content: keep data in memory, source: adr-002}\n",
        )
        .unwrap();
        assert_eq!(c.layers[0].source.as_deref(), Some("adr-004"));
        assert_eq!(c.rules[0].source.as_deref(), Some("adr-002"));
        assert!(c.render().iter().all(|l| !l.contains("adr-")));
        let (dir, files) = classes(&[(
            "src/main/java/com/example/widgets/AService.java",
            "package x;\nclass AService {}\n",
        )]);
        assert_eq!(
            c.audit(dir.path(), &files),
            ["com.example.widgets.AService is named *Service but is not annotated @Service (adr-004)"]
        );
    }

    #[test]
    fn a_contract_with_no_annotated_layers_finds_no_drift() {
        let c = Contract::from_yaml("layers:\n  - {name: service, suffix: Service}\n").unwrap();
        let (dir, files) = classes(&[(
            "src/main/java/com/example/widgets/AService.java",
            "package x;\nclass AService {}\n",
        )]);
        assert!(c.audit(dir.path(), &files).is_empty());
    }
}
