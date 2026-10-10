//! The shape of a draft: the schema the model is held to, and turning its answer into a [`Plan`].

use serde_json::{json, Value};

use canopy_implementer::plan::Plan;

/// What the model must answer with. `covers` can only name scenarios the feature has, and every list is
/// present, so an omitted scope is a deliberate empty list rather than a forgotten field.
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
                        "kind": { "type": "string", "enum": ["behavior_change", "refactor"] },
                        "create": paths,
                        "modify": paths,
                        "read_only": paths,
                        "covers": { "type": "array", "minItems": 1, "items": { "type": "string", "enum": scenario_ids } }
                    },
                    "required": ["id", "instruction", "kind", "create", "modify", "read_only", "covers"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["steps"],
        "additionalProperties": false
    })
}

/// The plan a draft describes. The id, the story and the feature path come from the harness, never from
/// the model. The plan format's own validation decides what is acceptable; its reason is returned as is.
pub fn into_plan(content: &str, id: &str, story: &str, feature: Option<&str>) -> Result<Plan, String> {
    let draft: Value = serde_json::from_str(content).map_err(|e| format!("the answer is not valid JSON: {e}"))?;
    let steps = draft
        .get("steps")
        .cloned()
        .ok_or_else(|| "the answer has no steps".to_string())?;
    let plan = json!({ "id": id, "story": story, "feature": feature, "steps": steps });
    Plan::from_yaml(&plan.to_string()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            step["properties"]["kind"]["enum"],
            serde_json::json!(["behavior_change", "refactor"])
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
        for field in ["id", "instruction", "kind", "create", "modify", "read_only", "covers"] {
            assert!(required.contains(&field), "{field} is required");
        }
    }

    const DRAFT: &str = r#"{"steps":[{"id":"a","instruction":"x","kind":"behavior_change","create":["./src/A.java"],"modify":[],"read_only":[],"covers":["first"]}]}"#;

    #[test]
    fn a_draft_becomes_a_plan_with_the_harness_supplied_id_story_and_feature() {
        let plan = into_plan(DRAFT, "p", "the story", Some("f.feature")).unwrap();
        assert_eq!(plan.id, "p");
        assert_eq!(plan.story.as_deref(), Some("the story"));
        assert_eq!(plan.feature.as_deref(), Some("f.feature"));
        assert_eq!(plan.steps[0].covers, ["first"]);
        assert_eq!(
            plan.steps[0].scope.create,
            ["src/A.java"],
            "paths are normalised by the plan"
        );
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
        let duplicate = DRAFT.replace("]}]}", "]},{\"id\":\"a\",\"instruction\":\"y\",\"kind\":\"refactor\",\"create\":[],\"modify\":[],\"read_only\":[],\"covers\":[\"first\"]}]}");
        let e = into_plan(&duplicate, "p", "s", None).unwrap_err();
        assert!(e.contains("more than once"), "{e}");
    }
}
