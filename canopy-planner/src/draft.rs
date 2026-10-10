//! The shape of a draft: the schema the model is held to, and turning its answer into a [`Plan`].

use std::collections::{BTreeSet, HashSet};

use serde_json::{json, Map, Value};

use canopy_implementer::plan::Plan;
use canopy_implementer::sandbox::RepoPath;

/// What the model must answer with: for each step, its instruction, the production files it touches, its
/// test files, and the scenarios it covers. Intent, not bookkeeping.
///
/// Everything that can be decided without the model is not asked of it. There is no `kind` (a refactor
/// step is exempt from the test rule, so the model cannot be the one to declare it), no `create` or
/// `modify` (whether a file is new is a fact of the repository and the earlier steps; the model got it
/// wrong in 9 of 10 drafts) and no `read_only` (the model listed the files it changes there too, in 38 of
/// 38 duplicates; the harness works out what a step reads from the classes its instruction names).
/// `tests` is required and cannot be empty, so a step without a test cannot be written. Paths are
/// grammar-constrained: production under `src/main`, tests under `src/test`. `covers` can only name
/// scenarios the feature has.
pub fn schema(scenario_ids: &[String]) -> Value {
    let paths = |pattern: &str| json!({ "type": "array", "items": { "type": "string", "pattern": pattern } });
    let mut tests = paths("^src/test/[A-Za-z0-9_./-]+$");
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
                        "files": paths("^src/main/[A-Za-z0-9_./-]+$"),
                        "tests": tests,
                        "covers": { "type": "array", "minItems": 1, "items": { "type": "string", "enum": scenario_ids } }
                    },
                    "required": ["id", "instruction", "files", "tests", "covers"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["steps"],
        "additionalProperties": false
    })
}

/// The plan a draft describes. The id, the story, the feature path, every step's kind (a behaviour change),
/// every step's context (the repository facts), whether each file is created or modified, and what a step
/// reads all come from the harness, never from the model. `existing` is every file the repository has. A
/// file is modified when the repository has it or an earlier step creates it, and created otherwise. A step
/// reads each other production class (existing or created by an earlier step) that its instruction names.
/// The plan format's own validation decides what else is acceptable; its reason is returned as is.
pub fn into_plan(
    content: &str,
    id: &str,
    story: &str,
    feature: Option<&str>,
    context: &str,
    existing: &[String],
) -> Result<Plan, String> {
    let draft: Value = serde_json::from_str(content).map_err(|e| format!("the answer is not valid JSON: {e}"))?;
    let mut steps = draft
        .get("steps")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| "the answer has no steps".to_string())?;
    let known: HashSet<&String> = existing.iter().collect();
    let mut created: Vec<String> = Vec::new();
    for step in &mut steps {
        let Some(fields) = step.as_object_mut() else { continue };
        let step_id = fields.get("id").and_then(Value::as_str).unwrap_or("?").to_string();
        for decided in ["kind", "create", "modify", "read_only", "context"] {
            if fields.contains_key(decided) {
                return Err(format!("step {step_id}: {decided} is not yours to set; leave it out"));
            }
        }
        fields.insert("kind".into(), json!("behavior_change"));
        fields.insert("context".into(), json!(context));
        let instruction = fields
            .get("instruction")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let touched = classify(fields, &step_id, &known, &created)?;
        let read_only = read_only_for(&instruction, &touched.production, existing, &created);
        created.extend(touched.create.iter().cloned());
        fields.insert("create".into(), json!(touched.create));
        fields.insert("modify".into(), json!(touched.modify));
        fields.insert("read_only".into(), json!(read_only));
    }
    let plan = json!({ "id": id, "story": story, "feature": feature, "steps": steps });
    Plan::from_yaml(&plan.to_string()).map_err(|e| e.to_string())
}

struct Touched {
    create: Vec<String>,
    modify: Vec<String>,
    /// The production files the step lists, normalised.
    production: HashSet<String>,
}

/// Splits a step's production files and test files into the ones to create and the ones to modify,
/// production first. A test path must be under `src/test`: a production file listed as a test would defeat
/// the point of the field.
fn classify(
    step: &mut Map<String, Value>,
    id: &str,
    existing: &HashSet<&String>,
    created: &[String],
) -> Result<Touched, String> {
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
    let normalise = |path: &String| -> Result<String, String> {
        Ok(RepoPath::parse(path)
            .map_err(|e| format!("step {id}: path {path:?} is not allowed: {e}"))?
            .as_str())
    };
    let mut listed: HashSet<String> = HashSet::new();
    let (mut create, mut modify, mut production_set) = (Vec::new(), Vec::new(), HashSet::new());
    for (is_test, path) in production
        .iter()
        .map(|p| (false, p))
        .chain(tests.iter().map(|p| (true, p)))
    {
        let normalised = normalise(path)?;
        if !listed.insert(normalised.clone()) {
            return Err(format!(
                "step {id}: {normalised} is listed more than once; list each file once, in files or tests (exactly one)"
            ));
        }
        if is_test && !normalised.starts_with("src/test/") {
            return Err(format!("step {id}: test file {path} is not under src/test"));
        }
        if !is_test {
            production_set.insert(normalised.clone());
        }
        match known_or_created(&normalised, existing, created) {
            true => modify.push(normalised),
            false => create.push(normalised),
        }
    }
    Ok(Touched {
        create,
        modify,
        production: production_set,
    })
}

fn known_or_created(path: &str, existing: &HashSet<&String>, created: &[String]) -> bool {
    existing.iter().any(|e| e.as_str() == path) || created.iter().any(|c| c == path)
}

/// The production classes a step reads: every existing or earlier-created `.java` file under `src/main`
/// that the step does not itself list, whose class name its instruction names as a whole word. Sorted.
fn read_only_for(instruction: &str, own: &HashSet<String>, existing: &[String], created: &[String]) -> Vec<String> {
    existing
        .iter()
        .chain(created)
        .filter(|path| path.starts_with("src/main/") && path.ends_with(".java") && !own.contains(*path))
        .filter(|path| {
            std::path::Path::new(path.as_str())
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|class| mentions_word(instruction, class))
        })
        .cloned()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

/// Whether `word` occurs in `text` bounded by non-identifier characters on both sides.
fn mentions_word(text: &str, word: &str) -> bool {
    let is_identifier = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(start, found)| {
        let before = text[..start].chars().next_back();
        let after = text[start + found.len()..].chars().next();
        !before.is_some_and(is_identifier) && !after.is_some_and(is_identifier)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopy_implementer::implementer::StepKind;

    fn ids() -> Vec<String> {
        vec!["first".to_string(), "second".to_string()]
    }

    #[test]
    fn the_schema_asks_for_intent_only_and_closes_what_can_be_closed() {
        let schema = schema(&ids());
        let step = &schema["properties"]["steps"]["items"];
        assert_eq!(schema["properties"]["steps"]["minItems"], 1);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(step["additionalProperties"], false);
        for gone in ["create", "modify", "kind", "read_only", "context"] {
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
        assert_eq!(required, ["id", "instruction", "files", "tests", "covers"]);
    }

    const DRAFT: &str = r#"{"steps":[{"id":"a","instruction":"x","files":["./src/main/A.java"],"tests":["src/test/ATest.java"],"covers":["first"]}]}"#;

    fn plan_of(draft: &str, existing: &[&str]) -> Result<Plan, String> {
        let existing: Vec<String> = existing.iter().map(|s| s.to_string()).collect();
        into_plan(draft, "p", "the story", Some("f.feature"), "the facts", &existing)
    }

    #[test]
    fn a_draft_becomes_a_plan_with_the_harness_supplied_id_story_feature_and_kind() {
        let plan = plan_of(DRAFT, &[]).unwrap();
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
    fn every_step_carries_the_repository_facts_as_its_context() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"x","files":["src/main/A.java"],"tests":["src/test/ATest.java"],"covers":["first"]},
          {"id":"b","instruction":"y","files":["src/main/B.java"],"tests":["src/test/BTest.java"],"covers":["first"]}]}"#;
        let plan = plan_of(draft, &[]).unwrap();
        assert!(plan.steps.iter().all(|s| s.context.as_deref() == Some("the facts")));
        let requests = plan.step_requests("ignored: the steps bring their own");
        assert!(requests.iter().all(|r| r.context == "the facts"));
    }

    #[test]
    fn a_file_the_repository_has_is_modified_and_a_new_one_is_created() {
        let draft = r#"{"steps":[{"id":"a","instruction":"x","files":["src/main/Existing.java","src/main/New.java"],"tests":["src/test/ExistingTest.java","src/test/NewTest.java"],"covers":["first"]}]}"#;
        let scope = plan_of(draft, &["src/main/Existing.java", "src/test/ExistingTest.java"])
            .unwrap()
            .steps[0]
            .scope
            .clone();
        assert_eq!(scope.modify, ["src/main/Existing.java", "src/test/ExistingTest.java"]);
        assert_eq!(scope.create, ["src/main/New.java", "src/test/NewTest.java"]);
    }

    #[test]
    fn a_file_an_earlier_step_creates_is_modified_by_a_later_one() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"x","files":["src/main/A.java"],"tests":["src/test/ATest.java"],"covers":["first"]},
          {"id":"b","instruction":"y","files":["src/main/A.java"],"tests":["src/test/ATest.java"],"covers":["second"]}]}"#;
        let plan = plan_of(draft, &[]).unwrap();
        assert_eq!(plan.steps[0].scope.create, ["src/main/A.java", "src/test/ATest.java"]);
        assert_eq!(plan.steps[1].scope.modify, ["src/main/A.java", "src/test/ATest.java"]);
        assert!(plan.steps[1].scope.create.is_empty());
    }

    #[test]
    fn read_only_is_every_other_production_class_the_instruction_names() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"Create Widget.","files":["src/main/java/p/Widget.java"],"tests":["src/test/java/p/WidgetTest.java"],"covers":["first"]},
          {"id":"b","instruction":"Create WidgetService, which stores a Widget and reads Config.","files":["src/main/java/p/WidgetService.java"],"tests":["src/test/java/p/WidgetServiceTest.java"],"covers":["first"]},
          {"id":"c","instruction":"Change WidgetService to reject a blank name; Unrelated is not used.","files":["src/main/java/p/WidgetService.java"],"tests":["src/test/java/p/WidgetServiceTest.java"],"covers":["second"]}]}"#;
        let plan = plan_of(
            draft,
            &[
                "src/main/java/p/Config.java",
                "src/main/java/p/Unrelated2.java",
                "src/test/java/p/ConfigTest.java",
            ],
        )
        .unwrap();
        assert!(
            plan.steps[0].scope.read_only.is_empty(),
            "the step's own class is not read-only"
        );
        assert_eq!(
            plan.steps[1].scope.read_only,
            ["src/main/java/p/Config.java", "src/main/java/p/Widget.java"],
            "an existing and an earlier-created class, each named whole-word; sorted"
        );
        assert!(
            plan.steps[2].scope.read_only.is_empty(),
            "WidgetService is changed here, Unrelated is not a class, and Widget is only a prefix of a word"
        );
    }

    #[test]
    fn test_files_and_unnamed_classes_are_never_read_only() {
        let draft = r#"{"steps":[{"id":"a","instruction":"Create Thing using WidgetTest and Other.","files":["src/main/java/p/Thing.java"],"tests":["src/test/java/p/ThingTest.java"],"covers":["first"]}]}"#;
        let plan = plan_of(
            draft,
            &["src/test/java/p/WidgetTest.java", "src/main/java/p/Another.java"],
        )
        .unwrap();
        assert!(plan.steps[0].scope.read_only.is_empty());
    }

    #[test]
    fn a_path_named_twice_in_one_step_is_reported_with_its_lists() {
        let twice = DRAFT.replace(
            "\"files\":[\"./src/main/A.java\"]",
            "\"files\":[\"src/main/A.java\",\"./src/main/A.java\"]",
        );
        let e = plan_of(&twice, &[]).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("src/main/A.java") && e.contains("exactly one"),
            "{e}"
        );
    }

    #[test]
    fn a_step_without_tests_or_a_test_outside_src_test_is_rejected_even_if_the_schema_was_bypassed() {
        let none = DRAFT.replace("\"tests\":[\"src/test/ATest.java\"],", "");
        let e = plan_of(&none, &[]).unwrap_err();
        assert!(e.contains("step a") && e.contains("tests"), "{e}");
        let elsewhere = DRAFT.replace("src/test/ATest.java", "src/main/ATest.java");
        let e = plan_of(&elsewhere, &[]).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("src/main/ATest.java") && e.contains("src/test"),
            "{e}"
        );
    }

    #[test]
    fn a_step_that_sets_what_the_harness_decides_is_rejected_not_obeyed() {
        for field in [
            "\"kind\":\"refactor\"",
            "\"create\":[]",
            "\"modify\":[]",
            "\"read_only\":[]",
            "\"context\":\"x\"",
        ] {
            let sneaky = DRAFT.replace("\"instruction\":\"x\",", &format!("\"instruction\":\"x\",{field},"));
            let e = plan_of(&sneaky, &[]).unwrap_err();
            assert!(e.contains("step a") && e.contains("not yours to set"), "{field}: {e}");
        }
    }

    #[test]
    fn text_that_is_not_json_or_has_no_steps_is_rejected_with_a_reason() {
        let e = plan_of("not json", &[]).unwrap_err();
        assert!(e.contains("JSON"), "{e}");
        let e = plan_of("{}", &[]).unwrap_err();
        assert!(e.contains("steps"), "{e}");
    }

    #[test]
    fn a_draft_the_plan_format_refuses_is_rejected_with_the_plans_own_reason() {
        let unsafe_path = DRAFT.replace("./src/main/A.java", "../outside.java");
        let e = plan_of(&unsafe_path, &[]).unwrap_err();
        assert!(e.contains("../outside.java"), "{e}");
        let duplicate = DRAFT.replace(
            "]}]}",
            "]},{\"id\":\"a\",\"instruction\":\"y\",\"files\":[],\"tests\":[\"src/test/ATest.java\"],\"covers\":[\"first\"]}]}",
        );
        let e = plan_of(&duplicate, &[]).unwrap_err();
        assert!(e.contains("more than once"), "{e}");
    }
}
