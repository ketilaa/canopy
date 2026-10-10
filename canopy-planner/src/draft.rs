//! The shape of a draft: the schema the model is held to, and turning its answer into a [`Plan`].

use serde_json::{json, Value};

use canopy_implementer::plan::Plan;

/// What the model must answer with. `covers` can only name scenarios the feature has, and every list is
/// present, so an omitted scope is a deliberate empty list rather than a forgotten field. There is no
/// `kind`: a refactor step is exempt from the test rule, so the model cannot be the one to declare it.
/// `tests` is required and cannot be empty, so a step without a test cannot be written: the model planned
/// production steps and test steps apart in 10 of 10 drafts when it was only told not to.
pub fn schema(scenario_ids: &[String]) -> Value {
    let paths = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "properties": {
            "steps": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "instruction": { "type": "string" },
                        "create": paths,
                        "modify": paths,
                        "read_only": paths,
                        "tests": {
                            "type": "array",
                            "minItems": 1,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "path": { "type": "string" },
                                    "action": { "type": "string", "enum": ["create", "modify"] }
                                },
                                "required": ["path", "action"],
                                "additionalProperties": false
                            }
                        },
                        "covers": { "type": "array", "minItems": 1, "items": { "type": "string", "enum": scenario_ids } }
                    },
                    "required": ["id", "instruction", "create", "modify", "read_only", "tests", "covers"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["steps"],
        "additionalProperties": false
    })
}

/// Moves a step's `tests` into its `create` and `modify` lists, by the action the model gave. Every test
/// path must be under `src/test`: a production file listed as a test would defeat the point of the field.
fn merge_tests(step: &mut serde_json::Map<String, Value>, id: &str) -> Result<(), String> {
    let tests = match step.remove("tests") {
        Some(Value::Array(tests)) if !tests.is_empty() => tests,
        _ => {
            return Err(format!(
                "step {id}: tests is missing or empty; every step needs at least one test"
            ))
        }
    };
    for test in tests {
        let path = test.get("path").and_then(Value::as_str).unwrap_or_default();
        let list = match test.get("action").and_then(Value::as_str) {
            Some("create") => "create",
            Some("modify") => "modify",
            other => {
                return Err(format!(
                    "step {id}: test {path}: action {other:?} is not create or modify"
                ))
            }
        };
        if !path.trim_start_matches("./").starts_with("src/test/") {
            return Err(format!("step {id}: test file {path} is not under src/test"));
        }
        match step.entry(list).or_insert_with(|| json!([])) {
            Value::Array(files) => files.push(json!(path)),
            _ => return Err(format!("step {id}: {list} is not a list")),
        }
    }
    Ok(())
}

/// The plan a draft describes. The id, the story, the feature path and every step's kind (a behaviour
/// change) come from the harness, never from the model. The plan format's own validation decides what is acceptable; its reason is returned as is.
pub fn into_plan(content: &str, id: &str, story: &str, feature: Option<&str>) -> Result<Plan, String> {
    let draft: Value = serde_json::from_str(content).map_err(|e| format!("the answer is not valid JSON: {e}"))?;
    let mut steps = draft
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "the answer has no steps".to_string())?;
    for step in &mut steps {
        let Some(fields) = step.as_object_mut() else { continue };
        if fields.contains_key("kind") {
            let id = fields.get("id").and_then(Value::as_str).unwrap_or("?");
            return Err(format!(
                "step {id}: the kind of a step is not yours to set; leave it out"
            ));
        }
        fields.insert("kind".into(), json!("behavior_change"));
        let id = fields.get("id").and_then(Value::as_str).unwrap_or("?").to_string();
        merge_tests(fields, &id)?;
    }
    let plan = json!({ "id": id, "story": story, "feature": feature, "steps": steps });
    Plan::from_yaml(&plan.to_string()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopy_implementer::implementer::StepKind;

    fn ids() -> Vec<String> {
        vec!["first".to_string(), "second".to_string()]
    }

    #[test]
    fn the_schema_closes_what_can_be_closed() {
        let schema = schema(&ids());
        let step = &schema["properties"]["steps"]["items"];
        assert_eq!(schema["properties"]["steps"]["minItems"], 1);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(step["additionalProperties"], false);
        assert!(
            step["properties"].get("kind").is_none(),
            "the model does not choose the kind of a step"
        );
        assert_eq!(
            step["properties"]["covers"]["items"]["enum"],
            serde_json::json!(["first", "second"])
        );
        assert_eq!(step["properties"]["covers"]["minItems"], 1);
        let required: Vec<&str> = step["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let tests = &step["properties"]["tests"];
        assert_eq!(tests["minItems"], 1, "a step cannot be written without a test");
        assert_eq!(
            tests["items"]["properties"]["action"]["enum"],
            serde_json::json!(["create", "modify"])
        );
        for field in ["id", "instruction", "create", "modify", "read_only", "tests", "covers"] {
            assert!(required.contains(&field), "{field} is required");
        }
    }

    const DRAFT: &str = r#"{"steps":[{"id":"a","instruction":"x","create":["./src/A.java"],"modify":[],"read_only":[],"tests":[{"path":"src/test/ATest.java","action":"create"}],"covers":["first"]}]}"#;

    #[test]
    fn a_draft_becomes_a_plan_with_the_harness_supplied_id_story_and_feature() {
        let plan = into_plan(DRAFT, "p", "the story", Some("f.feature")).unwrap();
        assert_eq!(plan.id, "p");
        assert_eq!(plan.story.as_deref(), Some("the story"));
        assert_eq!(plan.feature.as_deref(), Some("f.feature"));
        assert_eq!(plan.steps[0].covers, ["first"]);
        assert_eq!(
            plan.steps[0].scope.create,
            ["src/A.java", "src/test/ATest.java"],
            "paths are normalised by the plan"
        );
    }

    #[test]
    fn a_steps_tests_join_its_files_by_the_action_the_model_gave() {
        let draft = r#"{"steps":[{"id":"a","instruction":"x","create":["src/A.java"],"modify":["src/B.java"],"read_only":[],"tests":[{"path":"src/test/ATest.java","action":"create"},{"path":"src/test/BTest.java","action":"modify"}],"covers":["first"]}]}"#;
        let step = &into_plan(draft, "p", "s", None).unwrap().steps[0];
        assert_eq!(step.scope.create, ["src/A.java", "src/test/ATest.java"]);
        assert_eq!(step.scope.modify, ["src/B.java", "src/test/BTest.java"]);
    }

    #[test]
    fn a_test_outside_src_test_or_with_an_unknown_action_is_rejected_with_the_step() {
        let elsewhere = DRAFT.replace("src/test/ATest.java", "src/main/ATest.java");
        let e = into_plan(&elsewhere, "p", "s", None).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("src/main/ATest.java") && e.contains("src/test"),
            "{e}"
        );
        let odd = DRAFT.replace("\"create\"}]", "\"delete\"}]");
        let e = into_plan(&odd, "p", "s", None).unwrap_err();
        assert!(e.contains("step a") && e.contains("delete"), "{e}");
    }

    #[test]
    fn a_step_without_tests_is_rejected_even_if_the_schema_was_bypassed() {
        let none = DRAFT.replace(
            "\"tests\":[{\"path\":\"src/test/ATest.java\",\"action\":\"create\"}],",
            "",
        );
        let e = into_plan(&none, "p", "s", None).unwrap_err();
        assert!(e.contains("step a") && e.contains("tests"), "{e}");
    }

    #[test]
    fn every_step_is_a_behaviour_change_because_the_harness_says_so() {
        let draft = r#"{"steps":[{"id":"a","instruction":"x","create":[],"modify":[],"read_only":[],"tests":[{"path":"src/test/ATest.java","action":"create"}],"covers":["first"]},{"id":"b","instruction":"y","create":[],"modify":[],"read_only":[],"tests":[{"path":"src/test/ATest.java","action":"modify"}],"covers":["first"]}]}"#;
        let plan = into_plan(draft, "p", "s", None).unwrap();
        assert!(plan.steps.iter().all(|s| s.kind == StepKind::BehaviorChange));
    }

    #[test]
    fn a_step_that_names_its_own_kind_is_rejected_not_obeyed() {
        let sneaky = DRAFT.replace("\"instruction\":\"x\",", "\"instruction\":\"x\",\"kind\":\"refactor\",");
        let e = into_plan(&sneaky, "p", "s", None).unwrap_err();
        assert!(e.contains("kind") && e.contains("step a"), "{e}");
    }

    #[test]
    fn text_that_is_not_json_or_has_no_steps_is_rejected_with_a_reason() {
        let e = into_plan("not json", "p", "s", None).unwrap_err();
        assert!(e.contains("JSON"), "{e}");
        let e = into_plan("{}", "p", "s", None).unwrap_err();
        assert!(e.contains("steps"), "{e}");
    }

    #[test]
    fn a_draft_the_plan_format_refuses_is_rejected_with_the_plans_own_reason() {
        let unsafe_path = DRAFT.replace("./src/A.java", "../outside.java");
        let e = into_plan(&unsafe_path, "p", "s", None).unwrap_err();
        assert!(e.contains("../outside.java"), "{e}");
        let duplicate = DRAFT.replace("]}]}", "]},{\"id\":\"a\",\"instruction\":\"y\",\"create\":[],\"modify\":[],\"read_only\":[],\"tests\":[{\"path\":\"src/test/ATest.java\",\"action\":\"create\"}],\"covers\":[\"first\"]}]}");
        let e = into_plan(&duplicate, "p", "s", None).unwrap_err();
        assert!(e.contains("more than once"), "{e}");
    }
}
