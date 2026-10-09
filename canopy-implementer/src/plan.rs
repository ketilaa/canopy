//! A plan: an ordered list of small steps, written down before any of them runs.
//!
//! A plan is data. Today it is written by hand; later a planner produces the same thing from a story,
//! and a person approves it. Each step is self-contained: the Implementer sees one step at a time and
//! never the whole plan.

use serde::Deserialize;

use crate::implementer::{StepKind, StepRequest};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub instruction: String,
    /// Behaviour change unless the plan says it is a refactor. Declared by the plan, never by the model.
    #[serde(default)]
    pub kind: StepKind,
    /// Replaces the project facts for this step only.
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Plan {
    pub id: String,
    /// The story the plan implements, kept for the record. The Implementer is never shown it.
    #[serde(default)]
    pub story: Option<String>,
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
}

impl Plan {
    pub fn from_yaml(text: &str) -> Result<Self, PlanError> {
        let plan: Plan = serde_yaml::from_str(text).map_err(|e| PlanError::Parse(e.to_string()))?;
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
        Ok(plan)
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
}
