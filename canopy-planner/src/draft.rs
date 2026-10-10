//! The shape of a draft: the schema the model is held to, and turning its answer into a [`Plan`].

use std::collections::{BTreeSet, HashSet};

use serde_json::{json, Map, Value};

use canopy_implementer::java;
use canopy_implementer::plan::Plan;

/// What the model must answer with: for each step, its instruction, the classes it touches, its test
/// classes, and the scenarios it covers. Intent, not bookkeeping.
///
/// Everything the harness can decide is not asked of the model:
/// - no `kind` (a refactor step is exempt from the test rule, so the model cannot declare one);
/// - no `create` or `modify` (whether a file is new is a fact of the repository and the earlier steps; the
///   model got it wrong in 9 of 10 drafts);
/// - no `read_only` (the model listed the files it changes there too, 38 of 38; the harness works out what
///   a step reads from the classes its instruction names);
/// - no file paths: a class is named by its qualified name and the harness derives the path, since two
///   planners differ in package and class names and never in `src/main/java/` plus dots to slashes.
///
/// `test_classes` is required and cannot be empty, so a step without a test cannot be written. The names
/// are grammar-constrained to the root package, and a test class ends in `Test`. `covers` can only name
/// scenarios the feature has.
pub fn schema(scenario_ids: &[String], root_package: &str) -> Value {
    let root = root_package.replace('.', "[.]");
    let class_name = format!("^{root}([.][a-z][a-z0-9_]*)*[.][A-Z][A-Za-z0-9_]*");
    let names = |suffix: &str| json!({ "type": "array", "items": { "type": "string", "pattern": format!("{class_name}{suffix}$") } });
    let mut test_classes = names("Test");
    test_classes["minItems"] = json!(1);
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
                        "classes": names(""),
                        "test_classes": test_classes,
                        "covers": { "type": "array", "minItems": 1, "items": { "type": "string", "enum": scenario_ids } }
                    },
                    "required": ["id", "instruction", "classes", "test_classes", "covers"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["steps"],
        "additionalProperties": false
    })
}

/// The plan a draft describes. The id, the story, the feature path, every step's kind (a behaviour change),
/// every step's context (the repository facts), the source path of every class, whether each file is created
/// or modified, and what a step reads all come from the harness, never from the model. `existing` is every
/// file the repository has. A file is modified when the repository has it or an earlier step creates it, and
/// created otherwise. A step reads each other production class (existing or created by an earlier step) that
/// its instruction names. The plan format's own validation decides what else is acceptable; its reason is
/// returned as is.
pub fn into_plan(
    content: &str,
    id: &str,
    story: &str,
    feature: Option<&str>,
    context: &str,
    root_package: &str,
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
        for decided in ["kind", "create", "modify", "read_only", "context", "files", "tests"] {
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
        let production = class_names(fields, "classes", &step_id, root_package, false)?;
        let tests = class_names(fields, "test_classes", &step_id, root_package, true)?;
        if tests.is_empty() {
            return Err(format!(
                "step {step_id}: test_classes is missing or empty; every step needs at least one test"
            ));
        }
        let touched = classify(&production, &tests, &step_id, &known, &created)?;
        let read_only = read_only_for(&instruction, &touched.production, existing, &created);
        created.extend(touched.create.iter().cloned());
        fields.insert("create".into(), json!(touched.create));
        fields.insert("modify".into(), json!(touched.modify));
        fields.insert("read_only".into(), json!(read_only));
        fields.remove("classes");
        fields.remove("test_classes");
    }
    let plan = json!({ "id": id, "story": story, "feature": feature, "steps": steps });
    Plan::from_yaml(&plan.to_string()).map_err(|e| e.to_string())
}

/// The source paths of the class names a step lists under `field`, checked: each is a class in the root
/// package, and a test class ends in `Test`.
fn class_names(
    step: &Map<String, Value>,
    field: &str,
    id: &str,
    root_package: &str,
    is_test: bool,
) -> Result<Vec<String>, String> {
    let names = match step.get(field) {
        None => return Ok(vec![]),
        Some(Value::Array(items)) => items,
        Some(_) => return Err(format!("step {id}: {field} is not a list")),
    };
    names
        .iter()
        .map(|item| {
            let name = item
                .as_str()
                .ok_or_else(|| format!("step {id}: {field} holds a non-text entry"))?;
            let (package, class) = name.rsplit_once('.').unwrap_or(("", name));
            let in_root = package == root_package || package.starts_with(&format!("{root_package}."));
            if !in_root {
                return Err(format!(
                    "step {id}: {name} is not in the root package {root_package}; use a qualified class name"
                ));
            }
            let valid_class = class.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && class.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && package
                    .split('.')
                    .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            if !valid_class {
                return Err(format!(
                    "step {id}: {name} is not a class name; use a qualified class name such as {root_package}.Example"
                ));
            }
            if is_test && !class.ends_with("Test") {
                return Err(format!("step {id}: test class {name} must have a name ending in Test"));
            }
            Ok(match is_test {
                true => java::test_source_path(name),
                false => java::main_source_path(name),
            })
        })
        .collect()
}

struct Touched {
    create: Vec<String>,
    modify: Vec<String>,
    /// The production files the step lists.
    production: HashSet<String>,
}

/// Splits a step's production files and test files into the ones to create and the ones to modify,
/// production first.
fn classify(
    production: &[String],
    tests: &[String],
    id: &str,
    existing: &HashSet<&String>,
    created: &[String],
) -> Result<Touched, String> {
    let mut listed: HashSet<&String> = HashSet::new();
    let (mut create, mut modify) = (Vec::new(), Vec::new());
    for path in production.iter().chain(tests) {
        if !listed.insert(path) {
            return Err(format!(
                "step {id}: {path} is listed more than once; list each class once, in classes or test_classes (exactly one)"
            ));
        }
        match existing.contains(path) || created.contains(path) {
            true => modify.push(path.clone()),
            false => create.push(path.clone()),
        }
    }
    Ok(Touched {
        create,
        modify,
        production: production.iter().cloned().collect(),
    })
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

    const ROOT: &str = "com.example.widgets";

    fn ids() -> Vec<String> {
        vec!["first".to_string(), "second".to_string()]
    }

    #[test]
    fn the_schema_asks_for_class_names_and_intent_only_and_closes_what_can_be_closed() {
        let schema = schema(&ids(), ROOT);
        let step = &schema["properties"]["steps"]["items"];
        assert_eq!(schema["properties"]["steps"]["minItems"], 1);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(step["additionalProperties"], false);
        for gone in ["files", "tests", "create", "modify", "kind", "read_only", "context"] {
            assert!(
                step["properties"].get(gone).is_none(),
                "{gone} is not the model's to give"
            );
        }
        assert_eq!(
            step["properties"]["classes"]["items"]["pattern"],
            "^com[.]example[.]widgets([.][a-z][a-z0-9_]*)*[.][A-Z][A-Za-z0-9_]*$"
        );
        let tests = &step["properties"]["test_classes"];
        assert_eq!(tests["minItems"], 1, "a step cannot be written without a test");
        assert_eq!(
            tests["items"]["pattern"],
            "^com[.]example[.]widgets([.][a-z][a-z0-9_]*)*[.][A-Z][A-Za-z0-9_]*Test$"
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
        assert_eq!(required, ["id", "instruction", "classes", "test_classes", "covers"]);
    }

    const DRAFT: &str = r#"{"steps":[{"id":"a","instruction":"x","classes":["com.example.widgets.A"],"test_classes":["com.example.widgets.ATest"],"covers":["first"]}]}"#;

    fn plan_of(draft: &str, existing: &[&str]) -> Result<Plan, String> {
        let existing: Vec<String> = existing.iter().map(|s| s.to_string()).collect();
        into_plan(draft, "p", "the story", Some("f.feature"), "the facts", ROOT, &existing)
    }

    #[test]
    fn class_names_become_source_paths_and_the_plan_has_the_harness_supplied_fields() {
        let plan = plan_of(DRAFT, &[]).unwrap();
        assert_eq!(plan.id, "p");
        assert_eq!(plan.story.as_deref(), Some("the story"));
        assert_eq!(plan.feature.as_deref(), Some("f.feature"));
        assert_eq!(plan.steps[0].covers, ["first"]);
        assert_eq!(plan.steps[0].kind, StepKind::BehaviorChange);
        assert_eq!(
            plan.steps[0].scope.create,
            [
                "src/main/java/com/example/widgets/A.java",
                "src/test/java/com/example/widgets/ATest.java"
            ],
            "production first, then tests"
        );
    }

    #[test]
    fn a_subpackage_class_maps_to_its_directory() {
        let draft = DRAFT.replace("com.example.widgets.A\"", "com.example.widgets.domain.A\"");
        let plan = plan_of(&draft, &[]).unwrap();
        assert_eq!(
            plan.steps[0].scope.create[0],
            "src/main/java/com/example/widgets/domain/A.java"
        );
    }

    #[test]
    fn every_step_carries_the_repository_facts_as_its_context() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"x","classes":["com.example.widgets.A"],"test_classes":["com.example.widgets.ATest"],"covers":["first"]},
          {"id":"b","instruction":"y","classes":["com.example.widgets.B"],"test_classes":["com.example.widgets.BTest"],"covers":["first"]}]}"#;
        let plan = plan_of(draft, &[]).unwrap();
        assert!(plan.steps.iter().all(|s| s.context.as_deref() == Some("the facts")));
    }

    #[test]
    fn a_class_the_repository_has_is_modified_and_a_new_one_is_created() {
        let draft = r#"{"steps":[{"id":"a","instruction":"x","classes":["com.example.widgets.Existing","com.example.widgets.New"],"test_classes":["com.example.widgets.ExistingTest","com.example.widgets.NewTest"],"covers":["first"]}]}"#;
        let scope = plan_of(
            draft,
            &[
                "src/main/java/com/example/widgets/Existing.java",
                "src/test/java/com/example/widgets/ExistingTest.java",
            ],
        )
        .unwrap()
        .steps[0]
            .scope
            .clone();
        assert_eq!(
            scope.modify,
            [
                "src/main/java/com/example/widgets/Existing.java",
                "src/test/java/com/example/widgets/ExistingTest.java"
            ]
        );
        assert_eq!(
            scope.create,
            [
                "src/main/java/com/example/widgets/New.java",
                "src/test/java/com/example/widgets/NewTest.java"
            ]
        );
    }

    #[test]
    fn a_class_an_earlier_step_creates_is_modified_by_a_later_one() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"x","classes":["com.example.widgets.A"],"test_classes":["com.example.widgets.ATest"],"covers":["first"]},
          {"id":"b","instruction":"y","classes":["com.example.widgets.A"],"test_classes":["com.example.widgets.ATest"],"covers":["second"]}]}"#;
        let plan = plan_of(draft, &[]).unwrap();
        assert_eq!(plan.steps[0].scope.create.len(), 2);
        assert_eq!(plan.steps[1].scope.modify.len(), 2);
        assert!(plan.steps[1].scope.create.is_empty());
    }

    #[test]
    fn read_only_is_every_other_production_class_the_instruction_names() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"Create Widget.","classes":["com.example.widgets.Widget"],"test_classes":["com.example.widgets.WidgetTest"],"covers":["first"]},
          {"id":"b","instruction":"Create WidgetService, which stores a Widget and reads Config.","classes":["com.example.widgets.WidgetService"],"test_classes":["com.example.widgets.WidgetServiceTest"],"covers":["first"]},
          {"id":"c","instruction":"Change WidgetService to reject a blank name; Unrelated is not used.","classes":["com.example.widgets.WidgetService"],"test_classes":["com.example.widgets.WidgetServiceTest"],"covers":["second"]}]}"#;
        let plan = plan_of(
            draft,
            &[
                "src/main/java/com/example/widgets/Config.java",
                "src/main/java/com/example/widgets/Unrelated2.java",
                "src/test/java/com/example/widgets/ConfigTest.java",
            ],
        )
        .unwrap();
        assert!(
            plan.steps[0].scope.read_only.is_empty(),
            "the step's own class is not read-only"
        );
        assert_eq!(
            plan.steps[1].scope.read_only,
            [
                "src/main/java/com/example/widgets/Config.java",
                "src/main/java/com/example/widgets/Widget.java"
            ],
            "an existing and an earlier-created class, each named whole-word; sorted"
        );
        assert!(plan.steps[2].scope.read_only.is_empty());
    }

    #[test]
    fn a_class_named_twice_in_one_step_is_reported() {
        let twice = DRAFT.replace(
            "\"classes\":[\"com.example.widgets.A\"]",
            "\"classes\":[\"com.example.widgets.A\",\"com.example.widgets.A\"]",
        );
        let e = plan_of(&twice, &[]).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("A.java") && e.contains("exactly one"),
            "{e}"
        );
    }

    #[test]
    fn a_name_that_is_not_a_class_in_the_root_package_is_rejected_even_if_the_schema_was_bypassed() {
        for (bad, why) in [
            ("com.other.A", "root package"),
            ("src/main/java/com/example/widgets/A.java", "class name"),
            ("com.example.widgets.a", "class name"),
            ("A", "root package"),
        ] {
            let draft = DRAFT.replace("com.example.widgets.A\"", &format!("{bad}\""));
            let e = plan_of(&draft, &[]).unwrap_err();
            assert!(e.contains("step a") && e.contains(bad) && e.contains(why), "{bad}: {e}");
        }
        let not_a_test = DRAFT.replace("com.example.widgets.ATest", "com.example.widgets.AChecks");
        let e = plan_of(&not_a_test, &[]).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("AChecks") && e.contains("Test"),
            "{e}"
        );
    }

    #[test]
    fn a_step_without_test_classes_is_rejected_even_if_the_schema_was_bypassed() {
        let none = DRAFT.replace("\"test_classes\":[\"com.example.widgets.ATest\"],", "");
        let e = plan_of(&none, &[]).unwrap_err();
        assert!(e.contains("step a") && e.contains("test_classes"), "{e}");
    }

    #[test]
    fn a_step_that_sets_what_the_harness_decides_is_rejected_not_obeyed() {
        for field in [
            "\"kind\":\"refactor\"",
            "\"create\":[]",
            "\"modify\":[]",
            "\"read_only\":[]",
            "\"context\":\"x\"",
            "\"files\":[]",
            "\"tests\":[]",
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
    fn two_steps_with_the_same_id_are_rejected_with_the_plans_own_reason() {
        let duplicate = DRAFT.replace(
            "]}]}",
            "]},{\"id\":\"a\",\"instruction\":\"y\",\"classes\":[],\"test_classes\":[\"com.example.widgets.ATest\"],\"covers\":[\"first\"]}]}",
        );
        let e = plan_of(&duplicate, &[]).unwrap_err();
        assert!(e.contains("more than once"), "{e}");
    }
}
