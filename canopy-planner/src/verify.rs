//! The verifier: checks a draft plan against the repository, the feature and the shape the Implementer
//! needs, before any model time is spent running it.
//!
//! Every rule is an audit: it accepts or rejects, and names what is wrong and in which step. None of them
//! rewrites the draft.

use std::fmt;
use std::path::Path;

use canopy_implementer::implementer::StepKind;
use canopy_implementer::plan::Plan;

use crate::facts::RepoFacts;
use crate::gherkin::Feature;

/// Most files (created and modified) one step may touch: a proxy for the completion cap. Four is what the
/// widgets controller step needed once it was split; the files a step only reads do not count.
pub const MAX_FILES_PER_STEP: usize = 4;

/// Which rule an issue broke. Recorded per draft so a sweep can say what the model gets wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    /// A file to modify or read does not exist (and no earlier step creates it), or one to create already does.
    Repository,
    /// A scenario no step covers, a step that covers none, or a step that covers an unknown one.
    Coverage,
    /// A new file is outside the root package's source and test directories, or is not a `.java` file there.
    Location,
    /// A behaviour step changes production code and no test.
    MissingTest,
    /// A step touches more files than one answer can hold.
    StepSize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub rule: Rule,
    /// The step the issue belongs to, when it belongs to one.
    pub step: Option<String>,
    pub message: String,
}

/// Reads as the retry text names it: the step, then the problem.
impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.step {
            Some(step) => write!(f, "step {step}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// Issues in rule order, and within a rule in step order. Empty means the plan can be run.
pub fn verify(plan: &Plan, feature: &Feature, facts: &RepoFacts, root: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();
    issues.extend(
        plan.verify_against(root)
            .into_iter()
            .map(|text| from_text(Rule::Repository, text)),
    );
    issues.extend(
        plan.verify_coverage(&feature.scenario_ids())
            .into_iter()
            .map(|text| from_text(Rule::Coverage, text)),
    );
    for step in &plan.steps {
        for path in &step.scope.create {
            if !allowed_location(path, &facts.root_package) {
                issues.push(issue(
                    Rule::Location,
                    &step.id,
                    format!("{path} is not under the root package's source or test directory, or is not a .java file"),
                ));
            }
        }
    }
    for step in &plan.steps {
        let changed: Vec<&String> = step.scope.create.iter().chain(&step.scope.modify).collect();
        let changes = |prefix: &str| changed.iter().any(|p| p.starts_with(prefix));
        if step.kind == StepKind::BehaviorChange && changes("src/main/java/") && !changes("src/test/java/") {
            issues.push(issue(
                Rule::MissingTest,
                &step.id,
                "changes production code but creates or modifies no test".into(),
            ));
        }
    }
    for step in &plan.steps {
        let touched = step.scope.create.len() + step.scope.modify.len();
        if touched > MAX_FILES_PER_STEP {
            issues.push(issue(
                Rule::StepSize,
                &step.id,
                format!("touches {touched} files, more than {MAX_FILES_PER_STEP}; split the step"),
            ));
        }
    }
    issues
}

fn issue(rule: Rule, step: &str, message: String) -> Issue {
    Issue {
        rule,
        step: Some(step.to_string()),
        message,
    }
}

/// `Plan`'s own checks report plain text, `step <id>: <problem>` or a plan-wide sentence.
fn from_text(rule: Rule, text: String) -> Issue {
    match text.strip_prefix("step ").and_then(|rest| rest.split_once(": ")) {
        Some((step, message)) => Issue {
            rule,
            step: Some(step.to_string()),
            message: message.to_string(),
        },
        None => Issue {
            rule,
            step: None,
            message: text,
        },
    }
}

fn allowed_location(path: &str, root_package: &str) -> bool {
    let directory = root_package.replace('.', "/");
    let java = path.ends_with(".java")
        && (path.starts_with(&format!("src/main/java/{directory}/"))
            || path.starts_with(&format!("src/test/java/{directory}/")));
    java || path.starts_with("src/main/resources/") || path.starts_with("src/test/resources/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{baseline, scaffold};
    use canopy_implementer::plan::Plan;

    const FEATURE: &str = "Feature: F\n  Scenario: First\n    When x\n  Scenario: Second\n    When y\n";
    const DOMAIN: &str = "src/main/java/com/example/widgets/domain/Widget.java";
    const DOMAIN_TEST: &str = "src/test/java/com/example/widgets/domain/WidgetTest.java";

    /// One step creating a class and its test, covering both scenarios, then one that changes them.
    const VALID: &str = "
id: p
steps:
  - id: a
    instruction: x
    create: [src/main/java/com/example/widgets/domain/Widget.java, src/test/java/com/example/widgets/domain/WidgetTest.java]
    covers: [first, second]
  - id: b
    instruction: y
    modify: [src/main/java/com/example/widgets/domain/Widget.java, src/test/java/com/example/widgets/domain/WidgetTest.java]
    covers: [second]
";

    fn issues_for(plan_yaml: &str) -> Vec<Issue> {
        let dir = scaffold();
        let plan = Plan::from_yaml(plan_yaml).unwrap();
        let feature = Feature::parse(FEATURE).unwrap();
        let facts = RepoFacts::collect(dir.path()).unwrap();
        verify(&plan, &feature, &facts, dir.path())
    }

    fn only_rule(issues: &[Issue]) -> Rule {
        let rules: Vec<Rule> = issues.iter().map(|i| i.rule).collect();
        assert!(!rules.is_empty(), "expected an issue");
        assert!(
            rules.iter().all(|r| *r == rules[0]),
            "more than one rule fired: {issues:?}"
        );
        rules[0]
    }

    #[test]
    fn a_plan_that_follows_every_rule_has_no_issues() {
        assert_eq!(issues_for(VALID), vec![]);
    }

    #[test]
    fn a_file_that_does_not_exist_or_is_already_there_breaks_the_repository_rule() {
        let missing = VALID.replace(
            "modify: [src/main/java/com/example/widgets/domain/Widget.java,",
            "modify: [src/main/java/com/example/widgets/domain/Gone.java,",
        );
        let issues = issues_for(&missing);
        assert_eq!(only_rule(&issues), Rule::Repository);
        assert_eq!(issues[0].step.as_deref(), Some("b"));
        assert!(issues[0].message.contains("Gone.java"), "{issues:?}");

        let exists = VALID.replace(DOMAIN, "src/main/java/com/example/widgets/WidgetsApplication.java");
        assert_eq!(only_rule(&issues_for(&exists)), Rule::Repository);
    }

    #[test]
    fn a_scenario_no_step_covers_or_a_step_covering_none_breaks_the_coverage_rule() {
        let uncovered = VALID
            .replace("covers: [first, second]", "covers: [first]")
            .replace("covers: [second]", "covers: [first]");
        let issues = issues_for(&uncovered);
        assert_eq!(only_rule(&issues), Rule::Coverage);
        assert!(issues.iter().any(|i| i.message.contains("second")), "{issues:?}");

        let none = VALID.replace("    covers: [second]\n", "");
        assert_eq!(only_rule(&issues_for(&none)), Rule::Coverage);
    }

    #[test]
    fn a_new_file_outside_the_root_package_or_the_source_roots_breaks_the_location_rule() {
        let other_package = VALID.replace("com/example/widgets/domain/Widget.java", "com/other/Widget.java");
        let issues = issues_for(&other_package.replace("modify:", "read_only:"));
        assert_eq!(only_rule(&issues), Rule::Location);
        assert_eq!(issues[0].step.as_deref(), Some("a"));
        assert!(issues[0].message.contains("com/other/Widget.java"), "{issues:?}");

        let stray = VALID.replace(DOMAIN, "notes/Widget.java");
        assert_eq!(
            only_rule(&issues_for(&stray.replace("modify:", "read_only:"))),
            Rule::Location
        );

        let not_java = VALID.replace(DOMAIN, "src/main/java/com/example/widgets/domain/Widget.txt");
        assert_eq!(
            only_rule(&issues_for(&not_java.replace("modify:", "read_only:"))),
            Rule::Location
        );
    }

    #[test]
    fn resource_files_are_a_legitimate_place_for_a_new_file() {
        let plan = "
id: p
steps:
  - id: a
    instruction: x
    create: [src/main/resources/application.properties, src/main/java/com/example/widgets/domain/Widget.java, src/test/java/com/example/widgets/domain/WidgetTest.java]
    covers: [first, second]
";
        assert_eq!(issues_for(plan), vec![]);
    }

    #[test]
    fn a_behaviour_step_that_changes_production_code_without_a_test_breaks_the_test_rule() {
        let plan = "
id: p
steps:
  - id: a
    instruction: x
    create: [src/main/java/com/example/widgets/domain/Widget.java]
    covers: [first, second]
";
        let issues = issues_for(plan);
        assert_eq!(only_rule(&issues), Rule::MissingTest);
        assert_eq!(issues[0].step.as_deref(), Some("a"));
    }

    #[test]
    fn a_refactor_step_needs_no_test_and_a_test_only_step_is_fine() {
        let refactor = "
id: p
steps:
  - id: a
    instruction: x
    create: [src/main/java/com/example/widgets/domain/Widget.java, src/test/java/com/example/widgets/domain/WidgetTest.java]
    covers: [first, second]
  - id: b
    instruction: y
    kind: refactor
    modify: [src/main/java/com/example/widgets/domain/Widget.java]
    covers: [first]
  - id: c
    instruction: z
    modify: [src/test/java/com/example/widgets/domain/WidgetTest.java]
    covers: [second]
";
        assert_eq!(issues_for(refactor), vec![]);
    }

    #[test]
    fn a_step_touching_more_files_than_one_answer_can_hold_breaks_the_size_rule() {
        let files: Vec<String> = (0..MAX_FILES_PER_STEP)
            .map(|i| format!("src/main/java/com/example/widgets/domain/C{i}.java"))
            .collect();
        let plan = format!(
            "id: p\nsteps:\n  - id: a\n    instruction: x\n    create: [{}, {DOMAIN_TEST}, {DOMAIN}]\n    covers: [first, second]\n",
            files.join(", ")
        );
        let issues = issues_for(&plan);
        assert_eq!(only_rule(&issues), Rule::StepSize);
        assert!(
            issues[0].message.contains(&format!("{}", MAX_FILES_PER_STEP + 2)),
            "{issues:?}"
        );
    }

    #[test]
    fn files_a_step_only_reads_do_not_count_towards_its_size() {
        let reads: Vec<String> = (0..MAX_FILES_PER_STEP + 3)
            .map(|i| format!("src/main/java/com/example/widgets/R{i}.java"))
            .collect();
        let dir = scaffold();
        for r in &reads {
            crate::testing::write(dir.path(), r, "package x;");
        }
        let plan = format!(
            "id: p\nsteps:\n  - id: a\n    instruction: x\n    create: [{DOMAIN}, {DOMAIN_TEST}]\n    read_only: [{}]\n    covers: [first, second]\n",
            reads.join(", ")
        );
        let plan = Plan::from_yaml(&plan).unwrap();
        let facts = RepoFacts::collect(dir.path()).unwrap();
        assert_eq!(
            verify(&plan, &Feature::parse(FEATURE).unwrap(), &facts, dir.path()),
            vec![]
        );
    }

    #[test]
    fn an_issue_reads_as_its_step_and_message_for_the_retry_text() {
        let issue = Issue {
            rule: Rule::Coverage,
            step: Some("a".into()),
            message: "covers no scenario".into(),
        };
        assert_eq!(issue.to_string(), "step a: covers no scenario");
        let plan_wide = Issue {
            rule: Rule::Coverage,
            step: None,
            message: "scenario s: no step covers it".into(),
        };
        assert_eq!(plan_wide.to_string(), "scenario s: no step covers it");
    }

    #[test]
    fn the_hand_written_widgets_plan_passes_every_rule_against_the_baseline() {
        let Some(dir) = baseline("java-widgets", "baseline-00-widgets") else {
            return;
        };
        let plan = Plan::from_yaml(include_str!("../../canopy-implementer/tasks/widgets/plan.yaml")).unwrap();
        let feature = Feature::parse(include_str!("../../canopy-implementer/tasks/widgets/feature.feature")).unwrap();
        let facts = RepoFacts::collect(dir.path()).unwrap();
        assert_eq!(verify(&plan, &feature, &facts, dir.path()), vec![]);
    }
}
