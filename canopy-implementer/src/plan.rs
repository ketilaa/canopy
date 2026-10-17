//! A plan: an ordered list of small steps, written down before any of them runs.
//!
//! A plan is data. Today it is written by hand; later a planner produces the same thing from a story,
//! and a person approves it. Each step is self-contained: the Implementer sees one step at a time and
//! never the whole plan.

use serde::{Deserialize, Serialize};

use crate::implementer::{Scope, StepKind, StepRequest};
use crate::sandbox::RepoPath;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PlanStep {
    pub id: String,
    pub instruction: String,
    /// Behaviour change unless the plan says it is a refactor. Declared by the plan, never by the model.
    #[serde(default)]
    pub kind: StepKind,
    /// Replaces the project facts for this step only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// The files the planner decided this step may touch.
    #[serde(flatten)]
    pub scope: Scope,
    /// Ids of the feature's scenarios this step helps make pass. The Implementer ignores it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub covers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Plan {
    pub id: String,
    /// The story the plan implements, kept for the record. The Implementer is never shown it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story: Option<String>,
    /// The Gherkin feature the plan implements, as a path relative to the canopy-implementer crate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PlanError {
    #[error("the plan is not valid YAML for a plan: {0}")]
    Parse(String),
    #[error("the plan has no steps")]
    NoSteps,
    #[error("step id {0:?} is used more than once")]
    DuplicateId(String),
    #[error("step {0:?} has an empty instruction")]
    EmptyInstruction(String),
    #[error("a step has an empty id")]
    EmptyId,
    #[error("step {step:?}: path {path:?} is not allowed: {reason}")]
    BadPath { step: String, path: String, reason: String },
    #[error(
        "step {step:?}: path {path:?} is listed more than once ({lists}); a file belongs in exactly one list: \
         create for a new file, modify for an existing file the step changes, read_only for an existing file it only reads"
    )]
    ScopeOverlap {
        step: String,
        path: String,
        /// The lists the path was found in, sorted, as written: `create, modify`.
        lists: String,
    },
}

impl Plan {
    pub fn from_yaml(text: &str) -> Result<Self, PlanError> {
        let mut plan: Plan = serde_yaml::from_str(text).map_err(|e| PlanError::Parse(e.to_string()))?;
        if plan.steps.is_empty() {
            return Err(PlanError::NoSteps);
        }
        let mut seen = std::collections::HashSet::new();
        for step in &plan.steps {
            if step.id.trim().is_empty() {
                return Err(PlanError::EmptyId);
            }
            if !seen.insert(step.id.clone()) {
                return Err(PlanError::DuplicateId(step.id.clone()));
            }
            if step.instruction.trim().is_empty() {
                return Err(PlanError::EmptyInstruction(step.id.clone()));
            }
        }
        for step in &mut plan.steps {
            let step_id = step.id.clone();
            let mut seen: std::collections::HashMap<String, Vec<&'static str>> = std::collections::HashMap::new();
            for (list_name, list) in [
                ("modify", &mut step.scope.modify),
                ("create", &mut step.scope.create),
                ("read_only", &mut step.scope.read_only),
            ] {
                for path in list.iter_mut() {
                    let parsed = RepoPath::parse(path).map_err(|e| PlanError::BadPath {
                        step: step_id.clone(),
                        path: path.clone(),
                        reason: e.to_string(),
                    })?;
                    let normalised = parsed.as_str();
                    let lists = seen.entry(normalised.clone()).or_default();
                    lists.push(list_name);
                    if lists.len() > 1 {
                        let mut named = lists.clone();
                        named.sort_unstable();
                        return Err(PlanError::ScopeOverlap {
                            step: step_id,
                            path: normalised,
                            lists: named.join(", "),
                        });
                    }
                    *path = normalised;
                }
            }
        }
        Ok(plan)
    }

    /// Checks the plan against the repository as it is now, before any step runs: every file a step
    /// modifies or reads exists (or is created by an earlier step), and every file a step creates does
    /// not. Returns what is wrong, in step order; empty means the plan can be run.
    pub fn verify_against(&self, root: &std::path::Path) -> Vec<String> {
        let mut created: std::collections::HashSet<String> = std::collections::HashSet::new();
        let exists = |path: &str, created: &std::collections::HashSet<String>| {
            created.contains(path) || std::fs::symlink_metadata(root.join(path)).is_ok()
        };
        let mut issues = Vec::new();
        for step in &self.steps {
            for path in step.scope.modify.iter().chain(&step.scope.read_only) {
                if !exists(path, &created) {
                    issues.push(format!(
                        "step {}: {path} does not exist, and no earlier step creates it",
                        step.id
                    ));
                }
            }
            for path in &step.scope.create {
                if exists(path, &created) {
                    issues.push(format!(
                        "step {}: {path} already exists, but the step is meant to create it",
                        step.id
                    ));
                }
            }
            created.extend(step.scope.create.iter().cloned());
        }
        issues
    }

    /// The plan as YAML, in the form `from_yaml` reads back as the same plan.
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).expect("a plan is always serialisable")
    }

    /// Checks the plan against the scenarios of its feature: every scenario is covered by some step, every
    /// step covers at least one scenario, and no step covers a scenario the feature does not have.
    pub fn verify_coverage(&self, scenarios: &[String]) -> Vec<String> {
        let mut issues = Vec::new();
        for step in &self.steps {
            if step.covers.is_empty() {
                issues.push(format!("step {}: covers no scenario", step.id));
            }
            for id in &step.covers {
                if !scenarios.contains(id) {
                    issues.push(format!("step {}: covers {id}, an unknown scenario", step.id));
                }
            }
        }
        for scenario in scenarios {
            if !self.steps.iter().any(|s| s.covers.contains(scenario)) {
                issues.push(format!("scenario {scenario}: no step covers it"));
            }
        }
        issues
    }

    /// The requests to hand to the Implementer, in order. `project_context` is the standing facts every
    /// step gets unless it brings its own.
    pub fn step_requests(&self, project_context: &str) -> Vec<StepRequest> {
        self.steps
            .iter()
            .map(|step| StepRequest {
                id: step.id.clone(),
                instruction: step.instruction.trim().to_string(),
                context: step.context.clone().unwrap_or_else(|| project_context.to_string()),
                files: vec![],
                kind: step.kind,
                scope: step.scope.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = r#"
id: vat
story: "Orders include VAT."
steps:
  - id: vat-1
    instruction: "Add vatCents to PriceCalculator, with tests."
  - id: vat-2
    instruction: "Tidy the invoice code."
    kind: refactor
    context: "Only touch InvoiceFormatter."
"#;

    #[test]
    fn a_plan_parses_with_steps_in_order_and_defaults() {
        let plan = Plan::from_yaml(PLAN).unwrap();
        assert_eq!(plan.id, "vat");
        assert_eq!(plan.story.as_deref(), Some("Orders include VAT."));
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].id, "vat-1");
        assert_eq!(
            plan.steps[0].kind,
            StepKind::BehaviorChange,
            "behaviour change is the default"
        );
        assert_eq!(plan.steps[0].context, None);
        assert_eq!(plan.steps[1].kind, StepKind::Refactor);
        assert_eq!(plan.steps[1].context.as_deref(), Some("Only touch InvoiceFormatter."));
    }

    #[test]
    fn step_requests_carry_the_instruction_the_kind_and_the_right_context() {
        let requests = Plan::from_yaml(PLAN).unwrap().step_requests("Build: Maven");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].id, "vat-1");
        assert_eq!(requests[0].instruction, "Add vatCents to PriceCalculator, with tests.");
        assert_eq!(
            requests[0].context, "Build: Maven",
            "the standing facts when the step has none"
        );
        assert_eq!(requests[0].kind, StepKind::BehaviorChange);
        assert!(requests[0].files.is_empty(), "the model finds the files itself");
        assert_eq!(requests[1].context, "Only touch InvoiceFormatter.");
        assert_eq!(requests[1].kind, StepKind::Refactor);
    }

    #[test]
    fn a_plan_without_steps_is_rejected() {
        assert_eq!(Plan::from_yaml("id: p\nsteps: []\n"), Err(PlanError::NoSteps));
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let text = "id: p\nsteps:\n  - {id: a, instruction: x}\n  - {id: a, instruction: y}\n";
        assert_eq!(Plan::from_yaml(text), Err(PlanError::DuplicateId("a".into())));
    }

    #[test]
    fn an_empty_instruction_or_id_is_rejected() {
        let blank_instruction = "id: p\nsteps:\n  - {id: a, instruction: '  '}\n";
        assert_eq!(
            Plan::from_yaml(blank_instruction),
            Err(PlanError::EmptyInstruction("a".into()))
        );
        let blank_id = "id: p\nsteps:\n  - {id: '', instruction: x}\n";
        assert_eq!(Plan::from_yaml(blank_id), Err(PlanError::EmptyId));
    }

    #[test]
    fn invalid_yaml_and_unknown_kinds_are_rejected_not_defaulted() {
        assert!(matches!(Plan::from_yaml("not: [a plan"), Err(PlanError::Parse(_))));
        let unknown = "id: p\nsteps:\n  - {id: a, instruction: x, kind: rewrite}\n";
        assert!(matches!(Plan::from_yaml(unknown), Err(PlanError::Parse(_))));
    }
    #[test]
    fn every_plan_we_ship_for_a_dogfooding_task_is_valid() {
        for (name, text) in [
            ("vat", include_str!("../tasks/vat/plan.yaml")),
            ("naming", include_str!("../tasks/naming/plan.yaml")),
            ("widgets", include_str!("../tasks/widgets/plan.yaml")),
        ] {
            let plan = Plan::from_yaml(text).unwrap_or_else(|e| panic!("plan {name}: {e}"));
            assert_eq!(plan.id, name);
            assert!(plan.steps.len() >= 2, "plan {name} should have several small steps");
            assert!(plan.story.is_some(), "plan {name} keeps its story for the record");
        }
    }
    const COVERED: &str = r#"
id: p
feature: tasks/x/feature.feature
steps:
  - {id: a, instruction: x, covers: [first, second]}
  - {id: b, instruction: y, covers: [second]}
"#;

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_plan_names_its_feature_and_the_scenarios_each_step_covers() {
        let plan = Plan::from_yaml(COVERED).unwrap();
        assert_eq!(plan.feature.as_deref(), Some("tasks/x/feature.feature"));
        assert_eq!(plan.steps[0].covers, ["first", "second"]);
        assert_eq!(plan.steps[1].covers, ["second"]);
        assert!(
            Plan::from_yaml(PLAN).unwrap().steps[0].covers.is_empty(),
            "covers is optional"
        );
    }

    #[test]
    fn a_plan_that_covers_every_scenario_has_no_coverage_issues() {
        let plan = Plan::from_yaml(COVERED).unwrap();
        assert_eq!(plan.verify_coverage(&ids(&["first", "second"])), Vec::<String>::new());
    }

    #[test]
    fn a_scenario_no_step_covers_is_reported() {
        let plan = Plan::from_yaml(COVERED).unwrap();
        let issues = plan.verify_coverage(&ids(&["first", "second", "third"]));
        assert!(
            issues
                .iter()
                .any(|i| i.contains("third") && i.contains("no step covers")),
            "{issues:?}"
        );
    }

    #[test]
    fn a_step_covering_an_unknown_scenario_is_reported_with_the_step() {
        let plan = Plan::from_yaml(COVERED).unwrap();
        let issues = plan.verify_coverage(&ids(&["second"]));
        assert!(
            issues
                .iter()
                .any(|i| i.contains("step a") && i.contains("first") && i.contains("unknown")),
            "{issues:?}"
        );
    }

    #[test]
    fn a_step_that_covers_no_scenario_is_reported() {
        let plan = Plan::from_yaml(
            "id: p\nsteps:\n  - {id: a, instruction: x, covers: [first]}\n  - {id: b, instruction: y}\n",
        )
        .unwrap();
        let issues = plan.verify_coverage(&ids(&["first"]));
        assert!(
            issues
                .iter()
                .any(|i| i.contains("step b") && i.contains("covers no scenario")),
            "{issues:?}"
        );
    }

    #[test]
    fn the_widgets_plan_covers_the_scenarios_of_its_feature() {
        let plan = Plan::from_yaml(include_str!("../tasks/widgets/plan.yaml")).unwrap();
        let feature = include_str!("../tasks/widgets/feature.feature");
        assert_eq!(plan.feature.as_deref(), Some("tasks/widgets/feature.feature"));
        assert!(
            !plan.story.as_deref().unwrap().contains("Scenario"),
            "the Gherkin lives in the feature file"
        );
        for title in [
            "A valid name creates a widget with a generated id",
            "An invalid name is rejected",
        ] {
            assert!(feature.contains(title), "{title}");
        }
        let scenarios = ids(&[
            "a-valid-name-creates-a-widget-with-a-generated-id",
            "an-invalid-name-is-rejected",
        ]);
        assert_eq!(plan.verify_coverage(&scenarios), Vec::<String>::new());
    }

    #[test]
    fn the_order_plan_covers_the_scenarios_of_its_feature() {
        let plan = Plan::from_yaml(include_str!("../tasks/order/plan.yaml")).unwrap();
        let feature = include_str!("../tasks/order/feature.feature");
        assert_eq!(plan.feature.as_deref(), Some("tasks/order/feature.feature"));
        for title in [
            "Place an order for one product",
            "A net total of 500.00 or more gets a 10 percent volume discount",
            "A quantity that is not positive is rejected",
            "A placed order can be looked up by its id",
        ] {
            assert!(feature.contains(title), "{title}");
        }
        let scenarios = ids(&[
            "place-an-order-for-one-product",
            "a-net-total-of-500-00-or-more-gets-a-10-percent-volume-discount",
            "a-quantity-that-is-not-positive-is-rejected",
            "a-placed-order-can-be-looked-up-by-its-id",
        ]);
        assert_eq!(plan.verify_coverage(&scenarios), Vec::<String>::new());
    }

    #[test]
    fn the_checklist_order_plan_has_the_same_steps_and_coverage_as_the_order_plan() {
        let base = Plan::from_yaml(include_str!("../tasks/order/plan.yaml")).unwrap();
        let checklist = Plan::from_yaml(include_str!("../tasks/order/plan-checklist.yaml")).unwrap();
        let ids_of = |p: &Plan| p.steps.iter().map(|s| (s.id.clone(), s.scope.clone(), s.covers.clone())).collect::<Vec<_>>();
        assert_eq!(ids_of(&base), ids_of(&checklist));
        assert_ne!(base.steps[0].instruction, checklist.steps[0].instruction);
        assert_eq!(base.steps[1].instruction, checklist.steps[1].instruction);
        assert!(checklist.steps[0].instruction.contains("\n1. Unit price 100.00, quantity 1"), "the checklist keeps one item per line");
        assert!(checklist.steps[0].instruction.contains("MUST"));
    }

    #[test]
    fn the_order_plans_differ_in_instruction_detail_only() {
        let plans = [
            include_str!("../tasks/order/plan-checklist.yaml"),
            include_str!("../tasks/order/plan-signatures.yaml"),
            include_str!("../tasks/order/plan-oneline.yaml"),
        ]
        .map(|text| Plan::from_yaml(text).unwrap());
        let shape = |p: &Plan| p.steps.iter().map(|s| (s.id.clone(), s.scope.clone(), s.covers.clone())).collect::<Vec<_>>();
        let scenarios = ids(&[
            "place-an-order-for-one-product",
            "a-net-total-of-500-00-or-more-gets-a-10-percent-volume-discount",
            "a-quantity-that-is-not-positive-is-rejected",
            "a-placed-order-can-be-looked-up-by-its-id",
        ]);
        for plan in &plans {
            assert_eq!(shape(plan), shape(&plans[0]));
            assert_eq!(plan.verify_coverage(&scenarios), Vec::<String>::new());
            for step in &plan.steps {
                assert!(step.instruction.contains("no.experis.orders"), "{}", step.id);
            }
        }
        let length = |p: &Plan| p.steps.iter().map(|s| s.instruction.len()).sum::<usize>();
        assert!(length(&plans[0]) > length(&plans[1]) && length(&plans[1]) > length(&plans[2]));
    }

    /// Lowercase, every run of non-alphanumerics one `-`, none at either end (the planner's scenario id).
    fn slug(title: &str) -> String {
        let mut out = String::new();
        for c in title.chars() {
            if c.is_alphanumeric() {
                out.extend(c.to_lowercase());
            } else if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        }
        out.trim_end_matches('-').to_string()
    }

    fn scenario_ids(feature: &str) -> Vec<String> {
        feature
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                t.strip_prefix("Scenario Outline:").or_else(|| t.strip_prefix("Scenario:")).map(|r| slug(r.trim()))
            })
            .collect()
    }

    #[test]
    fn the_story_plans_cover_the_scenarios_of_their_features_and_differ_in_detail_only() {
        let stories: [(&str, &str, [&str; 2]); 3] = [
            (
                include_str!("../tasks/loans/feature.feature"),
                "loans",
                [include_str!("../tasks/loans/plan-signatures.yaml"), include_str!("../tasks/loans/plan-oneline.yaml")],
            ),
            (
                include_str!("../tasks/shipping/feature.feature"),
                "shipping",
                [include_str!("../tasks/shipping/plan-signatures.yaml"), include_str!("../tasks/shipping/plan-oneline.yaml")],
            ),
            (
                include_str!("../tasks/stock/feature.feature"),
                "stock",
                [include_str!("../tasks/stock/plan-signatures.yaml"), include_str!("../tasks/stock/plan-oneline.yaml")],
            ),
        ];
        for (feature, name, plans) in stories {
            let scenarios = scenario_ids(feature);
            assert!(scenarios.len() >= 5, "{name}");
            let parsed: Vec<Plan> = plans.iter().map(|t| Plan::from_yaml(t).unwrap()).collect();
            for plan in &parsed {
                assert_eq!(plan.verify_coverage(&scenarios), Vec::<String>::new(), "{name}");
            }
            let shape = |p: &Plan| p.steps.iter().map(|s| (s.id.clone(), s.scope.clone(), s.covers.clone())).collect::<Vec<_>>();
            assert_eq!(shape(&parsed[0]), shape(&parsed[1]), "{name}");
            let length = |p: &Plan| p.steps.iter().map(|s| s.instruction.len()).sum::<usize>();
            assert!(length(&parsed[0]) > length(&parsed[1]), "{name}");
        }
    }

    #[test]
    fn the_checklist_plans_have_the_shape_of_the_other_plans_of_their_story_and_list_their_tests() {
        let stories = [
            (include_str!("../tasks/loans/plan-checklist.yaml"), include_str!("../tasks/loans/plan-signatures.yaml")),
            (include_str!("../tasks/shipping/plan-checklist.yaml"), include_str!("../tasks/shipping/plan-signatures.yaml")),
            (include_str!("../tasks/shipping/plan-checklist-rules.yaml"), include_str!("../tasks/shipping/plan-signatures.yaml")),
            (include_str!("../tasks/stock/plan-checklist.yaml"), include_str!("../tasks/stock/plan-signatures.yaml")),
            (include_str!("../tasks/loans/plan-wording-only.yaml"), include_str!("../tasks/loans/plan-signatures.yaml")),
        ];
        let shape = |p: &Plan| p.steps.iter().map(|s| (s.id.clone(), s.scope.clone(), s.covers.clone())).collect::<Vec<_>>();
        for (checklist, signatures) in stories {
            let checklist = Plan::from_yaml(checklist).unwrap();
            let signatures = Plan::from_yaml(signatures).unwrap();
            assert_eq!(shape(&checklist), shape(&signatures));
            for step in &checklist.steps {
                assert!(step.instruction.contains("MUST") && step.instruction.contains("\n1. "), "{}", step.id);
            }
        }
    }

    #[test]
    fn the_loans_values_only_plan_lists_the_values_without_the_must_wording() {
        let values = Plan::from_yaml(include_str!("../tasks/loans/plan-values-only.yaml")).unwrap();
        let checklist = Plan::from_yaml(include_str!("../tasks/loans/plan-checklist.yaml")).unwrap();
        let shape = |p: &Plan| p.steps.iter().map(|s| (s.id.clone(), s.scope.clone(), s.covers.clone())).collect::<Vec<_>>();
        assert_eq!(shape(&values), shape(&checklist));
        for step in &values.steps {
            assert!(!step.instruction.contains("MUST") && !step.instruction.contains("go through the list"), "{}", step.id);
            assert!(step.instruction.contains("\n1. ") && step.instruction.contains("BigDecimal(\"") || step.id.contains("service"), "{}", step.id);
        }
        assert!(values.steps[0].instruction.contains("new BigDecimal(\"0.00\")"));
        let wording = Plan::from_yaml(include_str!("../tasks/loans/plan-wording-only.yaml")).unwrap();
        assert!(!wording.steps[0].instruction.contains("new BigDecimal(\"0.00\")"), "no concrete values in the wording-only plan");
    }

    #[test]
    fn a_plan_written_as_yaml_reads_back_as_the_same_plan() {
        for text in [PLAN, COVERED, SCOPED, include_str!("../tasks/widgets/plan.yaml")] {
            let plan = Plan::from_yaml(text).unwrap();
            assert_eq!(Plan::from_yaml(&plan.to_yaml()).unwrap(), plan);
        }
    }

    #[test]
    fn a_plan_written_as_yaml_leaves_out_what_is_empty() {
        let yaml = Plan::from_yaml("id: p\nsteps:\n  - {id: a, instruction: x}\n")
            .unwrap()
            .to_yaml();
        for absent in ["story", "feature", "context", "modify", "create", "read_only", "covers"] {
            assert!(!yaml.contains(absent), "{absent} should be left out:\n{yaml}");
        }
        assert!(yaml.contains("kind: behavior_change"), "{yaml}");
    }

    const SCOPED: &str = r#"
id: p
steps:
  - id: a
    instruction: x
    modify: [src/main/A.java]
    create: [src/main/B.java]
    read_only: [src/main/C.java]
  - id: b
    instruction: y
"#;

    #[test]
    fn a_step_can_name_the_files_it_may_modify_create_and_read_and_the_request_carries_them() {
        let plan = Plan::from_yaml(SCOPED).unwrap();
        assert_eq!(plan.steps[0].scope.modify, ["src/main/A.java"]);
        assert_eq!(plan.steps[0].scope.create, ["src/main/B.java"]);
        assert_eq!(plan.steps[0].scope.read_only, ["src/main/C.java"]);
        assert!(plan.steps[1].scope.is_empty(), "a step without files is unscoped");
        let requests = plan.step_requests("facts");
        assert_eq!(requests[0].scope, plan.steps[0].scope);
        assert!(requests[1].scope.is_empty());
    }

    #[test]
    fn unsafe_or_empty_paths_in_a_scope_are_rejected_and_the_others_are_normalised() {
        for bad in ["/etc/passwd", "../x", "", ".git/config"] {
            let text = format!("id: p\nsteps:\n  - {{id: a, instruction: x, modify: ['{bad}']}}\n");
            assert!(
                matches!(Plan::from_yaml(&text), Err(PlanError::BadPath { .. })),
                "{bad:?}"
            );
        }
        let plan = Plan::from_yaml("id: p\nsteps:\n  - {id: a, instruction: x, modify: ['./src/A.java']}\n").unwrap();
        assert_eq!(
            plan.steps[0].scope.modify,
            ["src/A.java"],
            "a leading ./ is normalised away"
        );
    }

    #[test]
    fn the_same_file_twice_or_in_two_lists_of_one_step_is_rejected() {
        let both = "id: p\nsteps:\n  - {id: a, instruction: x, modify: [src/A.java], create: [src/A.java]}\n";
        assert!(matches!(Plan::from_yaml(both), Err(PlanError::ScopeOverlap { .. })));
        let twice = "id: p\nsteps:\n  - {id: a, instruction: x, modify: [src/A.java, ./src/A.java]}\n";
        assert!(matches!(Plan::from_yaml(twice), Err(PlanError::ScopeOverlap { .. })));
        let read_and_change =
            "id: p\nsteps:\n  - {id: a, instruction: x, modify: [src/A.java], read_only: [src/A.java]}\n";
        assert!(matches!(
            Plan::from_yaml(read_and_change),
            Err(PlanError::ScopeOverlap { .. })
        ));
    }

    #[test]
    fn a_file_listed_twice_is_reported_with_the_lists_it_is_in_and_what_each_list_is_for() {
        let both = "id: p\nsteps:\n  - {id: a, instruction: x, modify: [src/A.java], create: [./src/A.java]}\n";
        let message = Plan::from_yaml(both).unwrap_err().to_string();
        for needle in ["step \"a\"", "src/A.java", "create, modify", "new file", "existing"] {
            assert!(message.contains(needle), "{needle}: {message}");
        }
        let twice = "id: p\nsteps:\n  - {id: a, instruction: x, read_only: [src/A.java, src/A.java]}\n";
        assert!(Plan::from_yaml(twice)
            .unwrap_err()
            .to_string()
            .contains("read_only, read_only"));
    }

    fn repo(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            let p = dir.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        dir
    }

    fn two_steps() -> Plan {
        Plan::from_yaml(
            "id: p\nsteps:\n  - {id: one, instruction: x, modify: [src/A.java], create: [src/B.java]}\n  - {id: two, instruction: y, modify: [src/B.java], read_only: [src/A.java]}\n",
        )
        .unwrap()
    }

    #[test]
    fn a_plan_whose_files_exist_or_are_created_in_time_has_no_issues() {
        let dir = repo(&["src/A.java"]);
        assert_eq!(two_steps().verify_against(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn a_file_to_modify_or_read_that_does_not_exist_is_reported_with_the_step() {
        let dir = repo(&[]);
        let issues = two_steps().verify_against(dir.path());
        assert!(
            issues
                .iter()
                .any(|i| i.contains("one") && i.contains("src/A.java") && i.contains("does not exist")),
            "{issues:?}"
        );
    }

    #[test]
    fn a_file_to_create_that_already_exists_is_reported() {
        let dir = repo(&["src/A.java", "src/B.java"]);
        let issues = two_steps().verify_against(dir.path());
        assert!(
            issues
                .iter()
                .any(|i| i.contains("one") && i.contains("src/B.java") && i.contains("already exists")),
            "{issues:?}"
        );
    }

    #[test]
    fn using_a_file_before_the_step_that_creates_it_is_reported() {
        let dir = repo(&["src/A.java"]);
        let plan = Plan::from_yaml(
            "id: p\nsteps:\n  - {id: first, instruction: x, modify: [src/B.java]}\n  - {id: second, instruction: y, create: [src/B.java]}\n",
        )
        .unwrap();
        let issues = plan.verify_against(dir.path());
        assert!(
            issues.iter().any(|i| i.contains("first") && i.contains("src/B.java")),
            "{issues:?}"
        );
    }

    #[test]
    fn two_steps_creating_the_same_file_is_reported() {
        let dir = repo(&[]);
        let plan = Plan::from_yaml(
            "id: p\nsteps:\n  - {id: first, instruction: x, create: [src/B.java]}\n  - {id: second, instruction: y, create: [src/B.java]}\n",
        )
        .unwrap();
        let issues = plan.verify_against(dir.path());
        assert!(
            issues
                .iter()
                .any(|i| i.contains("second") && i.contains("already exists")),
            "{issues:?}"
        );
    }
}
