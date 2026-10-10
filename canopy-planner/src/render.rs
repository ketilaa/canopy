//! A plan for a person to read and approve, and a comparison with a reference plan.

use std::collections::BTreeSet;

use canopy_implementer::implementer::StepKind;
use canopy_implementer::plan::Plan;

use crate::gherkin::Feature;

/// The plan as a document. It is rendered from the plan, never a second source of truth.
pub fn render_markdown(plan: &Plan, feature: &Feature) -> String {
    let mut out = format!("# Plan: {}\n", plan.id);
    if let Some(story) = &plan.story {
        out.push_str(&format!("\n## Story\n{}\n", story.trim()));
    }
    out.push_str("\n## Scenarios\n");
    for scenario in &feature.scenarios {
        let covering: Vec<&str> = plan
            .steps
            .iter()
            .filter(|s| s.covers.contains(&scenario.id))
            .map(|s| s.id.as_str())
            .collect();
        let coverage = match covering.is_empty() {
            true => "NOT COVERED".to_string(),
            false => format!("steps {}", covering.join(", ")),
        };
        out.push_str(&format!("- `{}` {}: {coverage}\n", scenario.id, scenario.title));
    }
    out.push_str("\n## Steps\n");
    for (index, step) in plan.steps.iter().enumerate() {
        let kind = match step.kind {
            StepKind::Refactor => " (refactor)",
            StepKind::BehaviorChange => "",
        };
        out.push_str(&format!(
            "\n### {}. {}{kind}\n{}\n\n",
            index + 1,
            step.id,
            step.instruction.trim()
        ));
        for (label, paths) in [
            ("Create", &step.scope.create),
            ("Modify", &step.scope.modify),
            ("Read only", &step.scope.read_only),
        ] {
            if !paths.is_empty() {
                let listed: Vec<String> = paths.iter().map(|p| format!("`{p}`")).collect();
                out.push_str(&format!("{label}: {}\n", listed.join(", ")));
            }
        }
        if !step.covers.is_empty() {
            out.push_str(&format!("Covers: {}\n", step.covers.join(", ")));
        }
    }
    out
}

/// Which files a plan creates that a reference plan does not, and the other way round.
#[derive(Debug, Clone, PartialEq)]
pub struct FileDiff {
    /// Created by the reference plan, not by this one.
    pub missing: Vec<String>,
    /// Created by this plan, not by the reference.
    pub extra: Vec<String>,
}

impl FileDiff {
    pub fn matches(&self) -> bool {
        self.missing.is_empty() && self.extra.is_empty()
    }
}

pub fn compare_created_files(plan: &Plan, reference: &Plan) -> FileDiff {
    let created =
        |p: &Plan| -> BTreeSet<String> { p.steps.iter().flat_map(|s| s.scope.create.iter().cloned()).collect() };
    let (ours, theirs) = (created(plan), created(reference));
    FileDiff {
        missing: theirs.difference(&ours).cloned().collect(),
        extra: ours.difference(&theirs).cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopy_implementer::plan::Plan;

    const FEATURE: &str = "Feature: F\n  Scenario: First one\n    When x\n  Scenario: Second one\n    When y\n";

    const PLAN: &str = "
id: p
story: The story.
steps:
  - id: a
    instruction: Make the first thing.
    create: [src/A.java]
    covers: [first-one]
  - id: b
    instruction: Change it.
    modify: [src/A.java]
    read_only: [src/B.java]
    covers: [first-one, second-one]
";

    fn rendered() -> String {
        render_markdown(&Plan::from_yaml(PLAN).unwrap(), &Feature::parse(FEATURE).unwrap())
    }

    #[test]
    fn the_document_names_the_plan_and_the_story() {
        let text = rendered();
        assert!(text.starts_with("# Plan: p"), "{text}");
        assert!(text.contains("The story."), "{text}");
    }

    #[test]
    fn each_scenario_lists_the_steps_that_cover_it() {
        let text = rendered();
        assert!(text.contains("`first-one` First one: steps a, b"), "{text}");
        assert!(text.contains("`second-one` Second one: steps b"), "{text}");
    }

    #[test]
    fn a_scenario_nothing_covers_is_called_out() {
        let plan = Plan::from_yaml("id: p\nsteps:\n  - {id: a, instruction: x, covers: [first-one]}\n").unwrap();
        let text = render_markdown(&plan, &Feature::parse(FEATURE).unwrap());
        assert!(text.contains("`second-one` Second one: NOT COVERED"), "{text}");
    }

    #[test]
    fn steps_come_in_order_with_instruction_files_and_coverage_and_no_empty_lists() {
        let text = rendered();
        let a = text.find("### 1. a").expect("step 1");
        let b = text.find("### 2. b").expect("step 2");
        assert!(a < b);
        assert!(text.contains("Make the first thing."));
        assert!(text.contains("Create: `src/A.java`"), "{text}");
        assert!(text.contains("Modify: `src/A.java`"), "{text}");
        assert!(text.contains("Read only: `src/B.java`"), "{text}");
        assert!(text.contains("Covers: first-one, second-one"), "{text}");
        let first_step = &text[a..b];
        assert!(
            !first_step.contains("Modify:") && !first_step.contains("Read only:"),
            "{first_step}"
        );
    }

    #[test]
    fn a_refactor_step_says_so_in_its_heading() {
        let plan = Plan::from_yaml("id: p\nsteps:\n  - {id: a, instruction: x, kind: refactor, covers: [first-one]}\n")
            .unwrap();
        let text = render_markdown(&plan, &Feature::parse(FEATURE).unwrap());
        assert!(text.contains("### 1. a (refactor)"), "{text}");
    }

    #[test]
    fn the_golden_comparison_lists_files_the_plan_lacks_and_files_it_adds() {
        let golden = Plan::from_yaml("id: g\nsteps:\n  - {id: a, instruction: x, create: [src/A.java, src/T.java]}\n  - {id: b, instruction: y, create: [src/B.java]}\n").unwrap();
        let drafted = Plan::from_yaml("id: p\nsteps:\n  - {id: a, instruction: x, create: [src/A.java, src/Extra.java]}\n  - {id: b, instruction: y, create: [src/B.java]}\n").unwrap();
        let diff = compare_created_files(&drafted, &golden);
        assert_eq!(diff.missing, ["src/T.java"]);
        assert_eq!(diff.extra, ["src/Extra.java"]);
        assert!(!diff.matches());
        assert!(compare_created_files(&golden, &golden).matches());
    }
}
