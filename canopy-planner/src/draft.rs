//! The shape of a draft: the schema the model is held to, and turning its answer into a [`Plan`].

use std::collections::HashSet;
use std::path::Path;

use serde_json::{json, Map, Value};

use canopy_implementer::plan::Plan;
use canopy_implementer::sandbox::RepoPath;

/// What the model must answer with: for each step, the production files it touches, the files it only
/// reads, its test files, and the scenarios it covers.
///
/// Everything that can be decided without the model is not asked of it. There is no `kind` (a refactor
/// step is exempt from the test rule, so the model cannot be the one to declare it) and no `create` or
/// `modify` (whether a file is new is a fact of the repository and the earlier steps; the model got it
/// wrong in 9 of 10 drafts). `tests` is required and cannot be empty, so a step without a test cannot be
/// written. Paths are grammar-constrained: production under `src/main`, tests under `src/test`. `covers`
/// can only name scenarios the feature has.
pub fn schema(scenario_ids: &[String]) -> Value {
    let paths = |pattern: Option<&str>| match pattern {
        Some(pattern) => json!({ "type": "array", "items": { "type": "string", "pattern": pattern } }),
        None => json!({ "type": "array", "items": { "type": "string" } }),
    };
    let mut tests = paths(Some("^src/test/[A-Za-z0-9_./-]+$"));
    tests["minItems"] = json!(1);
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
                        "files": paths(Some("^src/main/[A-Za-z0-9_./-]+$")),
                        "read_only": paths(None),
                        "tests": tests,
                        "covers": { "type": "array", "minItems": 1, "items": { "type": "string", "enum": scenario_ids } }
                    },
                    "required": ["id", "instruction", "files", "read_only", "tests", "covers"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["steps"],
        "additionalProperties": false
    })
}

/// The plan a draft describes. The id, the story, the feature path, every step's kind (a behaviour change),
/// every step's context (the repository facts) and whether each file is created or modified come from the harness, never from the model. A file is
/// modified when the repository has it or an earlier step creates it, and created otherwise. The plan
/// format's own validation decides what else is acceptable; its reason is returned as is.
pub fn into_plan(
    content: &str,
    id: &str,
    story: &str,
    feature: Option<&str>,
    context: &str,
    root: &Path,
) -> Result<Plan, String> {
    let draft: Value = serde_json::from_str(content).map_err(|e| format!("the answer is not valid JSON: {e}"))?;
    let mut steps = draft
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "the answer has no steps".to_string())?;
    let mut created: HashSet<String> = HashSet::new();
    for step in &mut steps {
        let Some(fields) = step.as_object_mut() else { continue };
        let step_id = fields.get("id").and_then(Value::as_str).unwrap_or("?").to_string();
        for decided in ["kind", "create", "modify", "context"] {
            if fields.contains_key(decided) {
                return Err(format!("step {step_id}: {decided} is not yours to set; leave it out"));
            }
        }
        fields.insert("kind".into(), json!("behavior_change"));
        fields.insert("context".into(), json!(context));
        let (create, modify) = classify(fields, &step_id, root, &created)?;
        created.extend(create.iter().cloned());
        fields.insert("create".into(), json!(create));
        fields.insert("modify".into(), json!(modify));
    }
    let plan = json!({ "id": id, "story": story, "feature": feature, "steps": steps });
    Plan::from_yaml(&plan.to_string()).map_err(|e| e.to_string())
}

/// Splits a step's production files and test files into the ones to create and the ones to modify,
/// production first. A test path must be under `src/test`: a production file listed as a test would defeat
/// the point of the field.
fn classify(
    step: &mut Map<String, Value>,
    id: &str,
    root: &Path,
    created: &HashSet<String>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let take = |step: &mut Map<String, Value>, field: &str| -> Result<Vec<String>, String> {
        match step.remove(field) {
            None => Ok(vec![]),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("step {id}: {field} holds a non-text entry"))
                })
                .collect(),
            Some(_) => Err(format!("step {id}: {field} is not a list")),
        }
    };
    let production = take(step, "files")?;
    let tests = take(step, "tests")?;
    if tests.is_empty() {
        return Err(format!(
            "step {id}: tests is missing or empty; every step needs at least one test"
        ));
    }
    let read_only = match step.get("read_only") {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        _ => Vec::new(),
    };
    let mut listed: HashSet<String> = HashSet::new();
    for path in production.iter().chain(&tests).chain(&read_only) {
        let normalised = RepoPath::parse(path)
            .map_err(|e| format!("step {id}: path {path:?} is not allowed: {e}"))?
            .as_str();
        if !listed.insert(normalised.clone()) {
            return Err(format!(
                "step {id}: {normalised} is listed more than once; list each file once, in files, read_only or tests (exactly one)"
            ));
        }
    }
    let (mut create, mut modify) = (Vec::new(), Vec::new());
    for (is_test, path) in production
        .iter()
        .map(|p| (false, p))
        .chain(tests.iter().map(|p| (true, p)))
    {
        let normalised = RepoPath::parse(path)
            .map_err(|e| format!("step {id}: path {path:?} is not allowed: {e}"))?
            .as_str();
        if is_test && !normalised.starts_with("src/test/") {
            return Err(format!("step {id}: test file {path} is not under src/test"));
        }
        let exists = created.contains(&normalised) || std::fs::symlink_metadata(root.join(&normalised)).is_ok();
        match exists {
            true => modify.push(normalised),
            false => create.push(normalised),
        }
    }
    Ok((create, modify))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{scaffold, write};
    use canopy_implementer::implementer::StepKind;

    fn ids() -> Vec<String> {
        vec!["first".to_string(), "second".to_string()]
    }

    #[test]
    fn the_schema_asks_for_paths_only_and_closes_what_can_be_closed() {
        let schema = schema(&ids());
        let step = &schema["properties"]["steps"]["items"];
        assert_eq!(schema["properties"]["steps"]["minItems"], 1);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(step["additionalProperties"], false);
        for gone in ["create", "modify", "kind"] {
            assert!(
                step["properties"].get(gone).is_none(),
                "{gone} is decided by the harness"
            );
        }
        assert_eq!(
            step["properties"]["files"]["items"]["pattern"],
            "^src/main/[A-Za-z0-9_./-]+$"
        );
        let tests = &step["properties"]["tests"];
        assert_eq!(tests["minItems"], 1, "a step cannot be written without a test");
        assert_eq!(tests["items"]["pattern"], "^src/test/[A-Za-z0-9_./-]+$");
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
        for field in ["id", "instruction", "files", "read_only", "tests", "covers"] {
            assert!(required.contains(&field), "{field} is required");
        }
    }

    const DRAFT: &str = r#"{"steps":[{"id":"a","instruction":"x","files":["./src/main/A.java"],"read_only":[],"tests":["src/test/ATest.java"],"covers":["first"]}]}"#;

    fn plan_of(draft: &str, root: &std::path::Path) -> Result<Plan, String> {
        into_plan(draft, "p", "the story", Some("f.feature"), "the facts", root)
    }

    #[test]
    fn every_step_carries_the_repository_facts_as_its_context() {
        let dir = scaffold();
        let draft = r#"{"steps":[
          {"id":"a","instruction":"x","files":["src/main/A.java"],"read_only":[],"tests":["src/test/ATest.java"],"covers":["first"]},
          {"id":"b","instruction":"y","files":["src/main/B.java"],"read_only":[],"tests":["src/test/BTest.java"],"covers":["first"]}]}"#;
        let plan = plan_of(draft, dir.path()).unwrap();
        assert!(plan.steps.iter().all(|s| s.context.as_deref() == Some("the facts")));
        let requests = plan.step_requests("ignored: the steps bring their own");
        assert!(requests.iter().all(|r| r.context == "the facts"));
    }

    #[test]
    fn a_draft_becomes_a_plan_with_the_harness_supplied_id_story_feature_and_kind() {
        let dir = scaffold();
        let plan = plan_of(DRAFT, dir.path()).unwrap();
        assert_eq!(plan.id, "p");
        assert_eq!(plan.story.as_deref(), Some("the story"));
        assert_eq!(plan.feature.as_deref(), Some("f.feature"));
        assert_eq!(plan.steps[0].covers, ["first"]);
        assert_eq!(plan.steps[0].kind, StepKind::BehaviorChange);
        assert_eq!(
            plan.steps[0].scope.create,
            ["src/main/A.java", "src/test/ATest.java"],
            "production first, then tests, paths normalised"
        );
    }

    #[test]
    fn a_file_the_repository_has_is_modified_and_a_new_one_is_created() {
        let dir = scaffold();
        write(dir.path(), "src/main/Existing.java", "package x;");
        write(dir.path(), "src/test/ExistingTest.java", "package x;");
        let draft = r#"{"steps":[{"id":"a","instruction":"x","files":["src/main/Existing.java","src/main/New.java"],"read_only":[],"tests":["src/test/ExistingTest.java","src/test/NewTest.java"],"covers":["first"]}]}"#;
        let scope = plan_of(draft, dir.path()).unwrap().steps[0].scope.clone();
        assert_eq!(scope.modify, ["src/main/Existing.java", "src/test/ExistingTest.java"]);
        assert_eq!(scope.create, ["src/main/New.java", "src/test/NewTest.java"]);
    }

    #[test]
    fn a_file_an_earlier_step_creates_is_modified_by_a_later_one() {
        let dir = scaffold();
        let draft = r#"{"steps":[
          {"id":"a","instruction":"x","files":["src/main/A.java"],"read_only":[],"tests":["src/test/ATest.java"],"covers":["first"]},
          {"id":"b","instruction":"y","files":["src/main/A.java"],"read_only":[],"tests":["src/test/ATest.java"],"covers":["second"]}]}"#;
        let plan = plan_of(draft, dir.path()).unwrap();
        assert_eq!(plan.steps[0].scope.create, ["src/main/A.java", "src/test/ATest.java"]);
        assert_eq!(plan.steps[1].scope.modify, ["src/main/A.java", "src/test/ATest.java"]);
        assert!(plan.steps[1].scope.create.is_empty());
        assert_eq!(
            plan.verify_against(dir.path()),
            Vec::<String>::new(),
            "the classification matches the repository"
        );
    }

    #[test]
    fn a_path_named_twice_in_one_step_is_still_reported_with_its_lists() {
        let dir = scaffold();
        let twice = DRAFT.replace(
            "\"files\":[\"./src/main/A.java\"]",
            "\"files\":[\"src/main/A.java\",\"./src/main/A.java\"]",
        );
        let e = plan_of(&twice, dir.path()).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("src/main/A.java") && e.contains("files, read_only or tests"),
            "{e}"
        );
        let also_read = DRAFT.replace("\"read_only\":[]", "\"read_only\":[\"src/main/A.java\"]");
        let e = plan_of(&also_read, dir.path()).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("src/main/A.java") && e.contains("exactly one"),
            "{e}"
        );
    }

    #[test]
    fn a_step_without_tests_or_a_test_outside_src_test_is_rejected_even_if_the_schema_was_bypassed() {
        let dir = scaffold();
        let none = DRAFT.replace("\"tests\":[\"src/test/ATest.java\"],", "");
        let e = plan_of(&none, dir.path()).unwrap_err();
        assert!(e.contains("step a") && e.contains("tests"), "{e}");
        let elsewhere = DRAFT.replace("src/test/ATest.java", "src/main/ATest.java");
        let e = plan_of(&elsewhere, dir.path()).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("src/main/ATest.java") && e.contains("src/test"),
            "{e}"
        );
    }

    #[test]
    fn a_step_that_names_its_own_kind_create_or_modify_is_rejected_not_obeyed() {
        let dir = scaffold();
        for field in [
            "\"kind\":\"refactor\"",
            "\"create\":[]",
            "\"modify\":[]",
            "\"context\":\"x\"",
        ] {
            let sneaky = DRAFT.replace("\"instruction\":\"x\",", &format!("\"instruction\":\"x\",{field},"));
            let e = plan_of(&sneaky, dir.path()).unwrap_err();
            assert!(e.contains("step a") && e.contains("not yours to set"), "{field}: {e}");
        }
    }

    #[test]
    fn text_that_is_not_json_or_has_no_steps_is_rejected_with_a_reason() {
        let dir = scaffold();
        let e = plan_of("not json", dir.path()).unwrap_err();
        assert!(e.contains("JSON"), "{e}");
        let e = plan_of("{}", dir.path()).unwrap_err();
        assert!(e.contains("steps"), "{e}");
    }

    #[test]
    fn a_draft_the_plan_format_refuses_is_rejected_with_the_plans_own_reason() {
        let dir = scaffold();
        let unsafe_path = DRAFT.replace("./src/main/A.java", "../outside.java");
        let e = plan_of(&unsafe_path, dir.path()).unwrap_err();
        assert!(e.contains("../outside.java"), "{e}");
        let duplicate = DRAFT.replace("]}]}", "]},{\"id\":\"a\",\"instruction\":\"y\",\"files\":[],\"read_only\":[],\"tests\":[\"src/test/ATest.java\"],\"covers\":[\"first\"]}]}");
        let e = plan_of(&duplicate, dir.path()).unwrap_err();
        assert!(e.contains("more than once"), "{e}");
    }
}
