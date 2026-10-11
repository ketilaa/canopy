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
/// are grammar-constrained to the root package; a test class ends in `Test` and a production class never
/// does (the model listed the class of a test-only step under `classes` in 13 of 14 attempts, retries
/// included, when this was only an audit). `covers` can only name
/// scenarios the feature has.
pub fn schema(scenario_ids: &[String], root_package: &str) -> Value {
    let root = root_package.replace('.', "[.]");
    // Every part is bounded: an unbounded `[A-Za-z0-9_]*Test` let the model write TestTestTest... until the token
    // cap in 8 of 13 capped drafts, and the grammar had nothing to stop it.
    let package = format!("^{root}([.][a-z][a-z0-9_]{{0,19}}){{0,6}}[.]");
    // A production class is any class name that does not end in `Test`, written as positive character
    // classes (the grammar has no lookahead): the last character is not `t`, or it is `t` not after `s`, or
    // `st` not after `e`, or `est` not after `T`. A one-letter name has no tail to check.
    let production = format!(
        "{package}[A-Z]([A-Za-z0-9_]{{0,38}}([A-Za-su-z0-9_]|[A-Za-rt-z0-9_]t|[A-Za-df-z0-9_]st|[A-SU-Za-z0-9_]est))?$"
    );
    let test = format!("{package}[A-Z][A-Za-z0-9_]{{0,35}}Test$");
    let names =
        |pattern: &str| json!({ "type": "array", "maxItems": 6, "items": { "type": "string", "pattern": pattern } });
    json!({
        "type": "object",
        "properties": {
            "steps": {
                "type": "array",
                "minItems": 1,
                "maxItems": 30,
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "instruction": { "type": "string" },
                        "classes": names(&production),
                        "test_classes": names(&test),
                        "covers": { "type": "array", "minItems": 1, "maxItems": scenario_ids.len().max(1), "items": { "type": "string", "enum": scenario_ids } }
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

/// Who wrote each field of a drafted plan: the step fields the model is asked for (read from the schema, so
/// the list cannot drift), the step fields the harness derives, and the plan-level fields the harness sets.
/// Recorded beside every generated plan so a run can be read as "what did the model decide".
pub fn provenance(scenario_ids: &[String], root_package: &str) -> Value {
    let schema = schema(scenario_ids, root_package);
    let authored: Vec<&String> = schema["properties"]["steps"]["items"]["properties"]
        .as_object()
        .map(|properties| properties.keys().collect())
        .unwrap_or_default();
    json!({
        "model_authored": authored,
        "harness_derived_per_step": ["context", "create", "kind", "modify", "read_only"],
        "harness_derived_per_plan": ["feature", "id", "story"],
    })
}

/// Something about a draft worth recording that does not make it invalid. An instruction that does not name a
/// class its step lists was an audit that rejected the draft; it rejected 9 of 10 plans in one batch and 3 of 3 in
/// each of two more, and a retry naming the problem did not change that. It is recorded so the rate can be read
/// against how the steps then fare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// A class in `classes` that the step's instruction never names.
    UnnamedClass,
    /// `test_classes` named by a step that has classes: ignored, the step's tests are derived from its classes.
    IgnoredTestClasses,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub step: String,
    pub kind: DiagnosticKind,
    pub class: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            DiagnosticKind::IgnoredTestClasses => write!(
                f,
                "step {}: test_classes {} is ignored; the tests of a step's classes are derived",
                self.step, self.class
            ),
            DiagnosticKind::UnnamedClass => write!(
                f,
                "step {}: {} is listed in classes but the instruction never names it",
                self.step, self.class
            ),
        }
    }
}

/// The diagnostics of a draft's text; empty for anything that is not a draft.
pub fn diagnostics(content: &str) -> Vec<Diagnostic> {
    let Ok(draft) = serde_json::from_str::<Value>(content) else {
        return vec![];
    };
    let Some(steps) = draft.get("steps").and_then(Value::as_array) else {
        return vec![];
    };
    let mut found = Vec::new();
    for step in steps {
        let id = step.get("id").and_then(Value::as_str).unwrap_or("?");
        let instruction = step.get("instruction").and_then(Value::as_str).unwrap_or_default();
        let simple_names = |key: &str| -> Vec<String> {
            step.get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|c| c.rsplit('.').next().unwrap_or(c).to_string())
                .collect()
        };
        let ignored = simple_names("test_classes");
        if !simple_names("classes").is_empty() && !ignored.is_empty() {
            found.push(Diagnostic {
                step: id.to_string(),
                kind: DiagnosticKind::IgnoredTestClasses,
                class: ignored.join(", "),
            });
        }
        for class in step
            .get("classes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let simple = class.rsplit('.').next().unwrap_or(class);
            if !mentions_word(instruction, simple) {
                found.push(Diagnostic {
                    step: id.to_string(),
                    kind: DiagnosticKind::UnnamedClass,
                    class: simple.to_string(),
                });
            }
        }
    }
    found
}

/// The plan a draft describes. The id, the story, the feature path, every step's kind (a behaviour change),
/// every step's context (the repository facts routed to the step's classes), the source path of every class, whether each file is created
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
    context: &dyn Fn(&[String]) -> String,
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
        let instruction = fields
            .get("instruction")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let production = class_names(fields, "classes", &step_id, root_package, false)?;
        let listed_tests = class_names(fields, "test_classes", &step_id, root_package, true)?;
        // The tests of a step's classes are derived (`XTest` for `X`); only a step with no class names its tests.
        let tests = match (production.is_empty(), listed_tests.is_empty()) {
            // Named alongside classes, they are ignored (and recorded by `diagnostics`): the model named an existing,
            // unrelated test in 43% of plans, and the harness would then have to make the step change it.
            (false, false) => production
                .iter()
                .filter_map(|path| java::qualified_name_of_source(path))
                .map(|class| java::test_source_path(&format!("{class}Test")))
                .collect(),
            (true, true) => {
                return Err(format!(
                    "step {step_id}: a step needs classes, or test_classes when it only adds tests"
                ))
            }
            (true, false) => listed_tests,
            (false, true) => production
                .iter()
                .filter_map(|path| java::qualified_name_of_source(path))
                .map(|class| java::test_source_path(&format!("{class}Test")))
                .collect(),
        };
        let step_classes: Vec<String> = production
            .iter()
            .chain(&tests)
            .filter_map(|path| java::qualified_name_of_source(path))
            .collect();
        fields.insert(
            "context".into(),
            json!(context_for_step(&context(&step_classes), &created)),
        );
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
            if !is_test && class.ends_with("Test") {
                return Err(format!(
                    "step {id}: {name} looks like a test class; list it in test_classes, not in classes"
                ));
            }
            Ok(match is_test {
                true => java::test_source_path(name),
                false => java::main_source_path(name),
            })
        })
        .collect()
}

/// The facts plus what the plan itself has created so far: the facts are about the repository before the
/// plan, and a later step would otherwise be told that classes its own earlier steps made do not exist.
fn context_for_step(facts: &str, created: &[String]) -> String {
    let names = |prefix: &str| -> Vec<String> {
        created
            .iter()
            .filter(|path| path.starts_with(prefix))
            .filter_map(|path| java::qualified_name_of_source(path))
            .collect()
    };
    let mut context = facts.to_string();
    for (label, classes) in [
        ("Classes created by earlier steps", names("src/main/java/")),
        ("Test classes created by earlier steps", names("src/test/java/")),
    ] {
        if !classes.is_empty() {
            context.push_str(&format!("\n{label}: {}", classes.join(", ")));
        }
    }
    context
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
    fn the_tests_of_a_steps_classes_are_derived_one_per_class_after_the_production_files() {
        let draft = r#"{"steps":[{"id":"a","instruction":"Create A and B.","classes":["com.example.widgets.A","com.example.widgets.domain.B"],"covers":["first"]}]}"#;
        let scope = plan_of(draft, &[]).unwrap().steps[0].scope.clone();
        assert_eq!(
            scope.create,
            [
                "src/main/java/com/example/widgets/A.java",
                "src/main/java/com/example/widgets/domain/B.java",
                "src/test/java/com/example/widgets/ATest.java",
                "src/test/java/com/example/widgets/domain/BTest.java",
            ]
        );
    }

    #[test]
    fn a_derived_test_is_modified_when_it_exists_and_created_when_it_does_not() {
        let draft = r#"{"steps":[{"id":"a","instruction":"Change A and B.","classes":["com.example.widgets.A","com.example.widgets.B"],"covers":["first"]}]}"#;
        let scope = plan_of(
            draft,
            &[
                "src/main/java/com/example/widgets/A.java",
                "src/main/java/com/example/widgets/B.java",
                "src/test/java/com/example/widgets/ATest.java",
            ],
        )
        .unwrap()
        .steps[0]
            .scope
            .clone();
        assert_eq!(
            scope.modify,
            [
                "src/main/java/com/example/widgets/A.java",
                "src/main/java/com/example/widgets/B.java",
                "src/test/java/com/example/widgets/ATest.java"
            ]
        );
        assert_eq!(scope.create, ["src/test/java/com/example/widgets/BTest.java"]);
    }

    #[test]
    fn a_step_with_no_classes_names_the_test_classes_it_adds_to() {
        let draft = r#"{"steps":[{"id":"a","instruction":"Add a case.","classes":[],"test_classes":["com.example.widgets.SomeTest"],"covers":["first"]}]}"#;
        let scope = plan_of(draft, &["src/test/java/com/example/widgets/SomeTest.java"])
            .unwrap()
            .steps[0]
            .scope
            .clone();
        assert_eq!(scope.modify, ["src/test/java/com/example/widgets/SomeTest.java"]);
        assert!(scope.create.is_empty());
    }

    #[test]
    fn test_classes_named_by_a_step_with_classes_are_ignored_and_recorded_and_a_step_needs_classes_or_tests() {
        let both = r#"{"steps":[{"id":"a","instruction":"A.","classes":["com.example.widgets.A"],"test_classes":["com.example.widgets.WidgetTest"],"covers":["first"]}]}"#;
        let scope = plan_of(both, &["src/test/java/com/example/widgets/WidgetTest.java"])
            .unwrap()
            .steps[0]
            .scope
            .clone();
        assert_eq!(
            scope.create,
            [
                "src/main/java/com/example/widgets/A.java",
                "src/test/java/com/example/widgets/ATest.java"
            ],
            "the tests are derived from the classes; the unrelated existing test is not made a modify"
        );
        assert!(scope.modify.is_empty());
        let notes: Vec<String> = diagnostics(both).iter().map(|d| d.to_string()).collect();
        assert!(
            notes
                .iter()
                .any(|n| n.contains("step a") && n.contains("WidgetTest") && n.contains("derived")),
            "{notes:?}"
        );
        let neither =
            r#"{"steps":[{"id":"a","instruction":"Nothing.","classes":[],"test_classes":[],"covers":["first"]}]}"#;
        let e = plan_of(neither, &[]).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("classes") && e.contains("test_classes"),
            "{e}"
        );
    }

    #[test]
    fn the_context_closure_sees_the_derived_tests_so_the_test_layer_reaches_the_step() {
        let draft = r#"{"steps":[{"id":"a","instruction":"Create A.","classes":["com.example.widgets.A"],"covers":["first"]}]}"#;
        let plan = into_plan(draft, "p", "s", None, &|classes| classes.join("+"), ROOT, &[]).unwrap();
        assert!(plan.steps[0]
            .context
            .as_deref()
            .unwrap()
            .starts_with("com.example.widgets.A+com.example.widgets.ATest"));
    }

    #[test]
    fn the_schema_has_one_step_shape_and_asks_for_class_names_and_intent_only() {
        let schema = schema(&ids(), ROOT);
        assert_eq!(schema["properties"]["steps"]["minItems"], 1);
        assert_eq!(schema["additionalProperties"], false);
        let step = &schema["properties"]["steps"]["items"];
        assert!(
            step.get("oneOf").is_none() && step.get("anyOf").is_none(),
            "alternatives made the free text loop"
        );
        assert_eq!(step["additionalProperties"], false);
        for gone in ["files", "tests", "create", "modify", "kind", "read_only", "context"] {
            assert!(
                step["properties"].get(gone).is_none(),
                "{gone} is not the model's to give"
            );
        }
        assert_eq!(
            step["properties"]["covers"]["items"]["enum"],
            serde_json::json!(["first", "second"])
        );
        assert_eq!(step["properties"]["covers"]["minItems"], 1);
        assert!(
            step["properties"]["classes"].get("minItems").is_none(),
            "a test-only step has no classes"
        );
        assert!(
            step["properties"]["test_classes"].get("minItems").is_none(),
            "a step with classes has none to give"
        );
        let required: Vec<&str> = step["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required, ["id", "instruction", "classes", "test_classes", "covers"]);
    }

    fn class_pattern(list: &str) -> regex::Regex {
        let schema = schema(&ids(), ROOT);
        let pattern = schema["properties"]["steps"]["items"]["properties"][list]["items"]["pattern"]
            .as_str()
            .unwrap()
            .to_string();
        regex::Regex::new(&pattern).unwrap()
    }

    #[test]
    fn the_classes_pattern_admits_production_class_names_and_never_one_ending_in_test() {
        let classes = class_pattern("classes");
        for ok in [
            "com.example.widgets.Widget",
            "com.example.widgets.A",
            "com.example.widgets.domain.WidgetService",
            "com.example.widgets.Contest",
            "com.example.widgets.TestHelper",
            "com.example.widgets.Widgettest",
            "com.example.widgets.Ab",
            "com.example.widgets.Request2",
        ] {
            assert!(classes.is_match(ok), "{ok} should be a production class name");
        }
        for bad in [
            "com.example.widgets.WidgetTest",
            "com.example.widgets.Test",
            "com.example.widgets.ATest",
            "com.example.widgets.domain.WidgetControllerTest",
            "com.other.Widget",
            "com.example.widgets.widget",
            "com.example.widgets.Widget.java",
        ] {
            assert!(!classes.is_match(bad), "{bad} must not be a production class name");
        }
    }

    #[test]
    fn a_class_name_is_bounded_so_a_repeated_token_cannot_run_on() {
        let classes = class_pattern("classes");
        let tests = class_pattern("test_classes");
        let name = |len: usize| format!("com.example.widgets.W{}", "a".repeat(len - 1));
        assert!(classes.is_match(&name(40)), "a name of 40 characters is fine");
        assert!(!classes.is_match(&name(41)), "but not 41");
        let test = |len: usize| format!("com.example.widgets.W{}Test", "a".repeat(len - 5));
        assert!(tests.is_match(&test(40)) && !tests.is_match(&test(60)));
        assert!(
            !tests.is_match(&format!("com.example.widgets.Widget{}", "Test".repeat(20))),
            "TestTestTest... is the loop the model fell into"
        );
        assert!(
            !classes.is_match("com.example.widgets.a.b.c.d.e.f.g.Widget"),
            "seven sub-packages is too many"
        );
        assert!(
            classes.is_match("com.example.widgets.a.b.c.d.e.f.Widget"),
            "six is the limit"
        );
        let long_segment = format!("com.example.widgets.{}.Widget", "a".repeat(21));
        assert!(
            !classes.is_match(&long_segment),
            "a package segment is at most 20 characters"
        );
    }

    #[test]
    fn the_lists_are_bounded_too() {
        let schema = schema(&ids(), ROOT);
        let step = &schema["properties"]["steps"]["items"]["properties"];
        assert_eq!(step["classes"]["maxItems"], 6);
        assert_eq!(step["test_classes"]["maxItems"], 6);
        assert_eq!(step["covers"]["maxItems"], 2, "as many as there are scenarios");
        assert_eq!(schema["properties"]["steps"]["maxItems"], 30);
    }

    #[test]
    fn the_test_classes_pattern_admits_only_names_ending_in_test() {
        let tests = class_pattern("test_classes");
        assert!(tests.is_match("com.example.widgets.WidgetTest"));
        assert!(tests.is_match("com.example.widgets.domain.ATest"));
        assert!(!tests.is_match("com.example.widgets.Widget"));
        assert!(!tests.is_match("com.example.widgets.WidgetTests"));
    }

    const DRAFT: &str =
        r#"{"steps":[{"id":"a","instruction":"x A","classes":["com.example.widgets.A"],"covers":["first"]}]}"#;

    fn plan_of(draft: &str, existing: &[&str]) -> Result<Plan, String> {
        let existing: Vec<String> = existing.iter().map(|s| s.to_string()).collect();
        into_plan(
            draft,
            "p",
            "the story",
            Some("f.feature"),
            &|_| "the facts".to_string(),
            ROOT,
            &existing,
        )
    }

    #[test]
    fn a_steps_context_is_built_from_its_own_classes_production_and_test() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"Create A.","classes":["com.example.widgets.A"],"covers":["first"]},
          {"id":"b","instruction":"Create B.","classes":["com.example.widgets.domain.B"],"covers":["first"]}]}"#;
        let plan = into_plan(
            draft,
            "p",
            "s",
            None,
            &|classes| format!("facts for {}", classes.join(" + ")),
            ROOT,
            &[],
        )
        .unwrap();
        assert!(plan.steps[0]
            .context
            .as_deref()
            .unwrap()
            .starts_with("facts for com.example.widgets.A + com.example.widgets.ATest"));
        assert!(plan.steps[1]
            .context
            .as_deref()
            .unwrap()
            .starts_with("facts for com.example.widgets.domain.B + com.example.widgets.domain.BTest"));
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
          {"id":"a","instruction":"x A","classes":["com.example.widgets.A"],"covers":["first"]},
          {"id":"b","instruction":"y B","classes":["com.example.widgets.B"],"covers":["first"]}]}"#;
        let plan = plan_of(draft, &[]).unwrap();
        assert!(plan
            .steps
            .iter()
            .all(|s| s.context.as_deref().unwrap().starts_with("the facts")));
    }

    #[test]
    fn a_step_is_told_which_classes_earlier_steps_of_the_plan_create_and_the_first_step_is_not() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"x A","classes":["com.example.widgets.A"],"covers":["first"]},
          {"id":"b","instruction":"y B","classes":["com.example.widgets.domain.B"],"covers":["first"]},
          {"id":"c","instruction":"z A","classes":["com.example.widgets.A"],"covers":["second"]}]}"#;
        let plan = plan_of(draft, &[]).unwrap();
        let context = |i: usize| plan.steps[i].context.clone().unwrap();
        assert_eq!(context(0), "the facts", "nothing was created before the first step");
        assert_eq!(
            context(1),
            "the facts\nClasses created by earlier steps: com.example.widgets.A\nTest classes created by earlier steps: com.example.widgets.ATest"
        );
        assert_eq!(
            context(2),
            "the facts\nClasses created by earlier steps: com.example.widgets.A, com.example.widgets.domain.B\nTest classes created by earlier steps: com.example.widgets.ATest, com.example.widgets.domain.BTest",
            "each class once, in the order the plan creates them; a class changed again is not created again"
        );
    }

    #[test]
    fn a_class_the_repository_has_is_modified_and_a_new_one_is_created() {
        let draft = r#"{"steps":[{"id":"a","instruction":"x Existing New","classes":["com.example.widgets.Existing","com.example.widgets.New"],"covers":["first"]}]}"#;
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
          {"id":"a","instruction":"x A","classes":["com.example.widgets.A"],"covers":["first"]},
          {"id":"b","instruction":"y A","classes":["com.example.widgets.A"],"covers":["second"]}]}"#;
        let plan = plan_of(draft, &[]).unwrap();
        assert_eq!(plan.steps[0].scope.create.len(), 2);
        assert_eq!(plan.steps[1].scope.modify.len(), 2);
        assert!(plan.steps[1].scope.create.is_empty());
    }

    #[test]
    fn read_only_is_every_other_production_class_the_instruction_names() {
        let draft = r#"{"steps":[
          {"id":"a","instruction":"Create Widget.","classes":["com.example.widgets.Widget"],"covers":["first"]},
          {"id":"b","instruction":"Create WidgetService, which stores a Widget and reads Config.","classes":["com.example.widgets.WidgetService"],"covers":["first"]},
          {"id":"c","instruction":"Change WidgetService to reject a blank name; Unrelated is not used.","classes":["com.example.widgets.WidgetService"],"covers":["second"]}]}"#;
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
    fn a_production_class_whose_name_ends_in_test_is_rejected_with_where_it_belongs() {
        let draft = DRAFT
            .replace("com.example.widgets.A\"", "com.example.widgets.WidgetTest\"")
            .replace("\"instruction\":\"x\"", "\"instruction\":\"Test WidgetTest\"");
        let e = plan_of(&draft, &[]).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("WidgetTest") && e.contains("test_classes") && e.contains("classes"),
            "{e}"
        );
    }

    #[test]
    fn a_listed_class_the_instruction_never_names_is_a_diagnostic_and_not_a_rejection() {
        let draft = r#"{"steps":[{"id":"a","instruction":"Create Widget and WidgetRepository.","classes":["com.example.widgets.Widget","com.example.widgets.WidgetRepository","com.example.widgets.WidgetController"],"covers":["first"]}]}"#;
        assert!(plan_of(draft, &[]).is_ok(), "the plan is still accepted");
        assert_eq!(
            diagnostics(draft),
            [Diagnostic {
                step: "a".into(),
                kind: DiagnosticKind::UnnamedClass,
                class: "WidgetController".into()
            }]
        );
        assert_eq!(
            diagnostics(draft)[0].to_string(),
            "step a: WidgetController is listed in classes but the instruction never names it"
        );
    }

    #[test]
    fn naming_a_class_by_simple_or_qualified_name_counts_and_a_longer_name_does_not() {
        for instruction in [
            "Create Widget.",
            "Create com.example.widgets.Widget.",
            "Widget: a record.",
        ] {
            let draft = format!(
                r#"{{"steps":[{{"id":"a","instruction":"{instruction}","classes":["com.example.widgets.Widget"],"covers":["first"]}}]}}"#
            );
            assert!(diagnostics(&draft).is_empty(), "{instruction}");
        }
        let longer = r#"{"steps":[{"id":"a","instruction":"Create WidgetService.","classes":["com.example.widgets.Widget"],"covers":["first"]}]}"#;
        assert_eq!(diagnostics(longer).len(), 1, "Widget is not the word WidgetService");
    }

    #[test]
    fn diagnostics_of_something_that_is_not_a_draft_are_empty_and_a_step_without_classes_has_none() {
        assert!(diagnostics("not json").is_empty());
        assert!(diagnostics("{}").is_empty());
        let tests_only = r#"{"steps":[{"id":"a","instruction":"Add a case.","classes":[],"test_classes":["com.example.widgets.SomeTest"],"covers":["first"]}]}"#;
        assert!(diagnostics(tests_only).is_empty());
    }

    #[test]
    fn a_step_with_only_test_classes_has_no_class_to_name_and_test_classes_need_no_mention() {
        let draft = r#"{"steps":[{"id":"a","instruction":"Add a case.","classes":[],"test_classes":["com.example.widgets.SomeTest"],"covers":["first"]}]}"#;
        assert!(plan_of(draft, &["src/test/java/com/example/widgets/SomeTest.java"]).is_ok());
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
        let not_a_test = r#"{"steps":[{"id":"a","instruction":"x","classes":[],"test_classes":["com.example.widgets.AChecks"],"covers":["first"]}]}"#;
        let e = plan_of(not_a_test, &[]).unwrap_err();
        assert!(
            e.contains("step a") && e.contains("AChecks") && e.contains("Test"),
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
            "\"files\":[]",
            "\"tests\":[]",
        ] {
            let sneaky = DRAFT.replace("\"instruction\":\"x A\",", &format!("\"instruction\":\"x A\",{field},"));
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
