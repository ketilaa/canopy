//! What the model is told when it drafts a plan. Built from scratch for every call, so a retry carries
//! exactly what it needs and nothing stale.

use canopy_implementer::llm::Message;

use crate::facts::RepoFacts;
use crate::gherkin::{Feature, Scenario};
use crate::verify::MAX_FILES_PER_STEP;

pub struct Input<'a> {
    pub story: &'a str,
    pub feature: &'a Feature,
    pub facts: &'a RepoFacts,
}

/// The answer that failed and why. `raw` is empty when there is no usable answer to show.
pub struct Previous<'a> {
    pub raw: &'a str,
    pub problems: Vec<String>,
}

fn rules() -> String {
    format!(
        "\
You write implementation plans for a Java 21 Spring Boot 3.4 Maven project.
ALWAYS write each instruction so it stands alone: name the package, every class and every method signature with what it does.
ALWAYS state in each instruction every test the step needs: the exact input values and the expected result.
ALWAYS state in each instruction how a test builds the objects it uses: which constructors, which arguments.
For each step: ALWAYS use only classes that earlier steps create.
ALWAYS keep each step to at most {MAX_FILES_PER_STEP} files created or modified; split a larger step."
    )
}

fn scenario_section(scenario: &Scenario) -> String {
    let mut out = format!("### {}\nScenario: {}\n", scenario.id, scenario.title);
    for step in &scenario.steps {
        out.push_str(&format!("{:?} {}\n", step.keyword, step.text));
    }
    if let Some(examples) = &scenario.examples {
        out.push_str("Cases:\n");
        for row in &examples.rows {
            let cells: Vec<String> = examples
                .columns
                .iter()
                .zip(row)
                .map(|(column, cell)| format!("{column} = {cell}"))
                .collect();
            out.push_str(&format!("- {}\n", cells.join(", ")));
        }
    }
    out
}

pub fn messages(input: &Input, previous: Option<&Previous>) -> Vec<Message> {
    let mut user = format!("## Story\n{}\n\n## Scenarios\n", input.story.trim());
    for scenario in &input.feature.scenarios {
        user.push_str(&scenario_section(scenario));
        user.push('\n');
    }
    user.push_str(
        "For each scenario: ALWAYS name its id in covers of at least one step that helps make it pass.\n\
For each case of a scenario outline: ALWAYS write a test with a concrete input value, in a step that covers the scenario, not in a step of its own.\n\n",
    );
    user.push_str(&format!(
        "## Repository\n{}\nALWAYS create new files under the main and test source directories above.\nFor each step: ALWAYS name in read_only every existing or earlier-created file that its code or tests use.\n",
        input.facts.render()
    ));
    if let Some(previous) = previous {
        if !previous.raw.is_empty() {
            user.push_str(&format!("\n## Your previous answer\n```json\n{}\n```\n", previous.raw));
        }
        user.push_str("\n## Problems\n");
        for problem in &previous.problems {
            user.push_str(&format!("- {problem}\n"));
        }
        user.push_str("\nReturn the complete corrected plan.\n");
    }
    vec![Message::system(&rules()), Message::user(&user)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scaffold;

    const FEATURE: &str = "Feature: F\n  Scenario: Plain one\n    When a client does x\n    Then y happens\n  Scenario Outline: Many\n    When a client does <input> with <extra>\n    Then it is rejected\n    Examples:\n      | input | extra |\n      | a blank thing | e1 |\n      | no thing | e2 |\n";

    fn user_text(previous: Option<&Previous>) -> (String, String) {
        let dir = scaffold();
        let facts = RepoFacts::collect(dir.path()).unwrap();
        let feature = Feature::parse(FEATURE).unwrap();
        let messages = messages(
            &Input {
                story: "Tell the story.",
                feature: &feature,
                facts: &facts,
            },
            previous,
        );
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        (messages[0].content.clone(), messages[1].content.clone())
    }

    #[test]
    fn the_user_message_carries_the_story_every_scenario_with_its_id_and_every_case_verbatim() {
        let (_, user) = user_text(None);
        assert!(user.contains("Tell the story."), "{user}");
        for needle in ["plain-one", "Plain one", "When a client does x", "many", "Many"] {
            assert!(user.contains(needle), "{needle}: {user}");
        }
        assert!(user.contains("input = a blank thing, extra = e1"), "{user}");
        assert!(user.contains("input = no thing, extra = e2"), "{user}");
    }

    #[test]
    fn the_repository_facts_sit_in_the_user_message_with_the_path_rule_next_to_them() {
        let (_, user) = user_text(None);
        let facts_at = user.find("Root package: com.example.widgets").expect("facts present");
        let rule_at = user.find("ALWAYS create new files under").expect("path rule present");
        assert!(
            rule_at > facts_at && rule_at - facts_at < 1500,
            "the rule follows the facts closely"
        );
    }

    #[test]
    fn the_rules_state_the_step_cap_from_the_verifier_and_enumerate_outline_cases() {
        let (system, user) = user_text(None);
        assert!(
            system.contains(&format!("at most {MAX_FILES_PER_STEP} files")),
            "{system}"
        );
        assert!(user.contains("not in a step of its own"), "{user}");
        assert!(system.contains("ALWAYS"), "{system}");
    }

    #[test]
    fn the_scenario_rules_follow_the_scenarios_and_the_read_only_rule_follows_the_facts() {
        let (system, user) = user_text(None);
        let last_case = user.find("- input = no thing").unwrap();
        let scenario_rule = user.find("For each case of a scenario outline").unwrap();
        let repository = user.find("## Repository").unwrap();
        assert!(last_case < scenario_rule && scenario_rule < repository, "{user}");
        assert!(user.find("name in read_only").unwrap() > user.find("Root package:").unwrap());
        assert!(
            !system.contains("read_only") && !system.contains("covers"),
            "those rules sit next to their content: {system}"
        );
    }

    #[test]
    fn no_rule_is_stated_twice() {
        let (system, user) = user_text(None);
        let rules: Vec<&str> = system
            .lines()
            .chain(user.lines())
            .filter(|l| l.starts_with("ALWAYS") || l.starts_with("NEVER"))
            .collect();
        let mut seen = std::collections::HashSet::new();
        for rule in rules {
            assert!(seen.insert(rule), "{rule} appears twice");
        }
    }

    #[test]
    fn a_first_prompt_has_no_previous_answer_and_a_retry_shows_it_with_each_problem() {
        let (_, first) = user_text(None);
        assert!(!first.contains("previous answer"), "{first}");
        let previous = Previous {
            raw: "{\"steps\":[]}",
            problems: vec![
                "step a: covers no scenario".into(),
                "scenario many: no step covers it".into(),
            ],
        };
        let (_, retry) = user_text(Some(&previous));
        assert!(retry.contains("{\"steps\":[]}"), "{retry}");
        assert!(retry.contains("- step a: covers no scenario"), "{retry}");
        assert!(retry.contains("- scenario many: no step covers it"), "{retry}");
        assert!(
            retry.trim_end().ends_with("Return the complete corrected plan."),
            "{retry}"
        );
    }

    #[test]
    fn a_retry_without_a_previous_answer_still_lists_the_problems() {
        let previous = Previous {
            raw: "",
            problems: vec!["the model call failed".into()],
        };
        let (_, retry) = user_text(Some(&previous));
        assert!(retry.contains("- the model call failed"), "{retry}");
        assert!(!retry.contains("previous answer"), "{retry}");
    }
}
