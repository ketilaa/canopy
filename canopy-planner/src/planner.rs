//! The Planner's flow: draft, verify, and one retry that names what is wrong.

use std::fmt;
use std::path::Path;

use canopy_implementer::llm::{Generator, StructuredRequest};
use canopy_implementer::plan::Plan;

use crate::draft;
use crate::facts::RepoFacts;
use crate::gherkin::Feature;
use crate::prompt::{self, Input, Previous};
use crate::verify::{self, Issue};

/// The first draft and one retry. A second failure ends the run: the problems are recorded, not worked around.
const MAX_ATTEMPTS: u32 = 2;

pub struct Request<'a> {
    pub id: &'a str,
    pub story: &'a str,
    pub feature_path: Option<&'a str>,
    pub feature: &'a Feature,
    pub facts: &'a RepoFacts,
    pub root: &'a Path,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// The call failed, or the answer was cut off.
    Model(String),
    /// The answer is not a plan the plan format accepts.
    Unparseable(String),
    /// The verifier rejected a well-formed plan.
    Rule(Issue),
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Model(text) | Problem::Unparseable(text) => f.write_str(text),
            Problem::Rule(issue) => issue.fmt(f),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attempt {
    /// What the model answered, when it answered. Kept even when it was rejected.
    pub raw: Option<String>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The plan, only when the last attempt had no problems.
    pub plan: Option<Plan>,
    pub attempts: Vec<Attempt>,
}

impl Outcome {
    pub fn is_valid(&self) -> bool {
        self.plan.is_some()
    }
}

pub struct Planner {
    generator: Box<dyn Generator>,
}

impl Planner {
    pub fn new(generator: Box<dyn Generator>) -> Self {
        Self { generator }
    }

    pub fn plan(&self, request: &Request) -> Outcome {
        let ids = request.feature.scenario_ids();
        let input = Input {
            story: request.story,
            feature: request.feature,
            facts: request.facts,
        };
        let mut attempts: Vec<Attempt> = Vec::new();
        for attempt in 1..=MAX_ATTEMPTS {
            let previous = attempts.last().map(|last| Previous {
                // A cut-off or missing answer is not worth showing; the problem says what to do.
                raw: match last.problems.iter().any(|p| matches!(p, Problem::Model(_))) {
                    true => "",
                    false => last.raw.as_deref().unwrap_or(""),
                },
                problems: last.problems.iter().map(Problem::to_string).collect(),
            });
            let structured = StructuredRequest {
                purpose: if attempt == 1 {
                    "draft-plan".into()
                } else {
                    format!("retry-{}", attempt - 1)
                },
                messages: prompt::messages(&input, previous.as_ref()),
                schema_name: "plan".into(),
                schema: draft::schema(&ids, &request.facts.root_package),
                thinking: false,
            };
            let context = request.facts.render();
            let (raw, plan, problems) = match self.generator.complete_structured(&structured) {
                Err(e) => (None, None, vec![Problem::Model(e.to_string())]),
                Ok(response) if response.finish_reason == "length" => (
                    Some(response.content),
                    None,
                    vec![Problem::Model(
                        "The answer hit the token limit before it was complete. Return fewer or shorter steps.".into(),
                    )],
                ),
                Ok(response) => {
                    match draft::into_plan(
                        &response.content,
                        request.id,
                        request.story,
                        request.feature_path,
                        &context,
                        &request.facts.root_package,
                        &request.facts.files,
                    ) {
                        Err(reason) => (Some(response.content), None, vec![Problem::Unparseable(reason)]),
                        Ok(plan) => {
                            let issues = verify::verify(&plan, request.feature, request.facts, request.root);
                            (
                                Some(response.content),
                                Some(plan),
                                issues.into_iter().map(Problem::Rule).collect(),
                            )
                        }
                    }
                }
            };
            let valid = problems.is_empty();
            attempts.push(Attempt { raw, problems });
            if valid {
                return Outcome { plan, attempts };
            }
        }
        Outcome { plan: None, attempts }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scaffold;
    use crate::verify::Rule;
    use canopy_implementer::llm::{LlmError, LlmResponse, Message, StructuredRequest};
    use canopy_implementer::observe::FailureKind;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    const FEATURE: &str = "Feature: F\n  Scenario: First\n    When x\n  Scenario: Second\n    When y\n";

    const GOOD: &str = r#"{"steps":[
      {"id":"a","instruction":"make Widget",
       "classes":["com.example.widgets.domain.Widget"],
       "test_classes":["com.example.widgets.domain.WidgetTest"],"covers":["first","second"]}]}"#;
    const UNCOVERED: &str = r#"{"steps":[
      {"id":"a","instruction":"make Widget",
       "classes":["com.example.widgets.domain.Widget"],
       "test_classes":["com.example.widgets.domain.WidgetTest"],"covers":["first"]}]}"#;

    struct Script {
        replies: Mutex<VecDeque<Result<LlmResponse, LlmError>>>,
        requests: Arc<Mutex<Vec<StructuredRequest>>>,
    }

    impl Generator for Script {
        fn complete_structured(&self, request: &StructuredRequest) -> Result<LlmResponse, LlmError> {
            self.requests.lock().unwrap().push(request.clone());
            self.replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("the script ran out of replies")
        }
    }

    fn reply(content: &str) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            call_id: 1,
            content: content.into(),
            finish_reason: "stop".into(),
            prompt_tokens: 1,
            completion_tokens: 1,
        })
    }

    /// Runs the planner against the scaffold with these replies; returns the outcome and the requests seen.
    fn run(replies: Vec<Result<LlmResponse, LlmError>>) -> (Outcome, Vec<StructuredRequest>) {
        let dir = scaffold();
        let requests = Arc::new(Mutex::new(vec![]));
        let planner = Planner::new(Box::new(Script {
            replies: Mutex::new(replies.into()),
            requests: requests.clone(),
        }));
        let feature = Feature::parse(FEATURE).unwrap();
        let facts = RepoFacts::collect(dir.path()).unwrap();
        let outcome = planner.plan(&Request {
            id: "p",
            story: "the story",
            feature_path: Some("f.feature"),
            feature: &feature,
            facts: &facts,
            root: dir.path(),
        });
        let seen = requests.lock().unwrap().clone();
        (outcome, seen)
    }

    fn user(request: &StructuredRequest) -> &Message {
        request.messages.last().unwrap()
    }

    #[test]
    fn a_good_first_draft_is_the_plan_after_one_call() {
        let (outcome, requests) = run(vec![reply(GOOD)]);
        assert_eq!(requests.len(), 1);
        let plan = outcome.plan.expect("valid");
        assert_eq!(plan.id, "p");
        assert_eq!(plan.feature.as_deref(), Some("f.feature"));
        assert_eq!(outcome.attempts.len(), 1);
        assert!(outcome.attempts[0].problems.is_empty());
        assert_eq!(outcome.attempts[0].raw.as_deref(), Some(GOOD));
    }

    #[test]
    fn the_call_is_schema_constrained_without_thinking_and_the_schema_knows_the_scenarios() {
        let (_, requests) = run(vec![reply(GOOD)]);
        let request = &requests[0];
        assert_eq!(request.purpose, "draft-plan");
        assert_eq!(request.schema_name, "plan");
        assert!(!request.thinking);
        assert_eq!(
            request.schema["properties"]["steps"]["items"]["properties"]["covers"]["items"]["enum"],
            serde_json::json!(["first", "second"])
        );
    }

    #[test]
    fn a_draft_that_breaks_a_rule_gets_one_retry_that_names_the_problem_and_shows_the_answer() {
        let (outcome, requests) = run(vec![reply(UNCOVERED), reply(GOOD)]);
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].purpose, "retry-1");
        let retry = &user(&requests[1]).content;
        assert!(retry.contains("scenario second: no step covers it"), "{retry}");
        assert!(retry.contains("\"covers\":[\"first\"]"), "{retry}");
        assert!(outcome.plan.is_some());
        assert_eq!(outcome.attempts.len(), 2);
        assert_eq!(outcome.attempts[0].problems.len(), 1);
        assert!(matches!(&outcome.attempts[0].problems[0], Problem::Rule(issue) if issue.rule == Rule::Coverage));
    }

    #[test]
    fn two_failed_drafts_end_as_invalid_with_both_attempts_kept_and_no_third_call() {
        let (outcome, requests) = run(vec![reply(UNCOVERED), reply(UNCOVERED)]);
        assert_eq!(requests.len(), 2);
        assert_eq!(outcome.plan, None);
        assert_eq!(outcome.attempts.len(), 2);
        assert!(!outcome.is_valid());
        assert_eq!(outcome.attempts[1].raw.as_deref(), Some(UNCOVERED));
    }

    #[test]
    fn an_unparseable_answer_is_retried_with_the_reason() {
        let (outcome, requests) = run(vec![reply("not json"), reply(GOOD)]);
        assert!(outcome.is_valid());
        assert!(
            user(&requests[1]).content.contains("not valid JSON"),
            "{}",
            user(&requests[1]).content
        );
        assert!(matches!(outcome.attempts[0].problems[0], Problem::Unparseable(_)));
    }

    #[test]
    fn a_truncated_answer_is_a_problem_and_is_retried_without_trusting_its_content() {
        let mut truncated = reply(GOOD);
        truncated.as_mut().unwrap().finish_reason = "length".into();
        let (outcome, requests) = run(vec![truncated, reply(GOOD)]);
        assert!(outcome.is_valid());
        assert!(
            user(&requests[1]).content.contains("token limit"),
            "{}",
            user(&requests[1]).content
        );
        assert!(matches!(outcome.attempts[0].problems[0], Problem::Model(_)));
    }

    #[test]
    fn a_failed_model_call_counts_as_an_attempt_and_is_retried() {
        let failed = Err(LlmError {
            call_id: 1,
            kind: FailureKind::Timeout,
            status: None,
            message: "timed out".into(),
        });
        let (outcome, requests) = run(vec![failed, reply(GOOD)]);
        assert!(outcome.is_valid());
        assert_eq!(outcome.attempts[0].raw, None);
        assert!(
            user(&requests[1]).content.contains("timed out"),
            "{}",
            user(&requests[1]).content
        );
        assert!(!user(&requests[1]).content.contains("previous answer"));
    }

    #[test]
    fn a_plan_the_verifier_would_reject_is_never_returned_as_valid() {
        let (outcome, _) = run(vec![reply(UNCOVERED), reply(UNCOVERED)]);
        assert!(outcome.plan.is_none());
    }
}
