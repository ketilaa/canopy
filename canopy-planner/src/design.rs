//! The design of a step's classes, decided before any code is written.
//!
//! A drafted plan says what each step is for in a sentence; the hand-written plan that passes states, per step, the
//! shape of every class, every signature, how a test builds its subject and which values it checks. This is the pass
//! that asks for those, one step at a time, in a schema that cannot leave them out, and writes them into the step's
//! instruction. The test and the implementation of a step then start from the same signatures and the same cases,
//! where they used to choose separately (a `findAll` returning an `Iterable`, an id supplied by the caller or by the
//! constructor, a status code nobody stated).
//!
//! The text the Implementer reads is rendered from the validated fields, not written by the model.

use std::collections::HashSet;
use std::path::Path;

use canopy_implementer::classpath::ClassIndex;
use canopy_implementer::implementer::Scope;
use canopy_implementer::java;
use canopy_implementer::llm::{Generator, Message, StructuredRequest};
use canopy_implementer::plan::Plan;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::facts::RepoFacts;
use crate::gherkin::Feature;

/// A design and one retry that names what is wrong; a second failure leaves the step as it was.
const MAX_ATTEMPTS: u32 = 2;

const TYPE_TEXT: &str = "[A-Za-z0-9_<>,.? \\[\\]]";

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StepDesign {
    pub types: Vec<TypeDesign>,
    /// The Java expression that builds the object under test, its collaborators created with `new`.
    pub subject_setup: String,
    pub tests: Vec<TestCase>,
}

/// One test: the Java expression that makes the call, with every input written out, and what must happen.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TestCase {
    pub call: String,
    pub expect: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TypeDesign {
    pub class: String,
    /// `record`, `class` or `interface`.
    pub shape: String,
    pub fields: Vec<FieldDesign>,
    /// The constructor's parameter list with parentheses, or empty.
    pub constructor: String,
    pub methods: Vec<MethodDesign>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FieldDesign {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MethodDesign {
    pub signature: String,
    pub does: String,
}

/// The schema of one step's design: the class names are the step's own, every list and every string is bounded.
pub fn schema(classes: &[String]) -> Value {
    let name = "^[A-Za-z_][A-Za-z0-9_]{0,39}$";
    let ty = format!("^[A-Za-z]{TYPE_TEXT}{{0,60}}$");
    let params = format!("\\({TYPE_TEXT}{{0,100}}\\)");
    let signature = format!("^[A-Za-z]{TYPE_TEXT}{{0,70}}{params}( throws [A-Za-z0-9_, ]{{1,60}})?$");
    let sentence = json!({ "type": "string", "minLength": 3, "maxLength": 240 });
    json!({
        "type": "object",
        "properties": {
            "types": {
                "type": "array", "minItems": 1, "maxItems": classes.len().max(1),
                "items": {
                    "type": "object",
                    "properties": {
                        "class": { "type": "string", "enum": classes },
                        "shape": { "type": "string", "enum": ["record", "class", "interface"] },
                        "fields": { "type": "array", "maxItems": 8, "items": {
                            "type": "object",
                            "properties": { "name": { "type": "string", "pattern": name }, "type": { "type": "string", "pattern": ty } },
                            "required": ["name", "type"], "additionalProperties": false } },
                        "constructor": { "type": "string", "pattern": format!("^({params})?$") },
                        "methods": { "type": "array", "maxItems": 8, "items": {
                            "type": "object",
                            "properties": { "signature": { "type": "string", "pattern": signature }, "does": sentence },
                            "required": ["signature", "does"], "additionalProperties": false } }
                    },
                    "required": ["class", "shape", "fields", "constructor", "methods"],
                    "additionalProperties": false
                }
            },
            "subject_setup": { "type": "string", "minLength": 3, "maxLength": 240 },
            "tests": { "type": "array", "minItems": 1, "maxItems": 12, "items": {
                "type": "object",
                "properties": {
                    "call": { "type": "string", "minLength": 3, "maxLength": 160 },
                    "expect": sentence
                },
                "required": ["call", "expect"], "additionalProperties": false } }
        },
        "required": ["types", "subject_setup", "tests"],
        "additionalProperties": false
    })
}

fn simple(class: &str) -> &str {
    class.rsplit('.').next().unwrap_or(class)
}

/// What is wrong with a design of these classes; empty when it is complete.
pub fn problems(design: &StepDesign, classes: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for class in classes {
        match design.types.iter().filter(|t| &t.class == class).count() {
            0 => problems.push(format!("{class} has no design; design every class of the step")),
            1 => {}
            _ => problems.push(format!("{class} is designed more than once; design each class once")),
        }
    }
    for t in &design.types {
        if !classes.contains(&t.class) {
            problems.push(format!("{} is not a class of this step", t.class));
        } else if t.fields.is_empty() && t.methods.is_empty() && t.constructor.is_empty() {
            problems.push(format!("{} has no fields, constructor or methods", t.class));
        }
        if t.shape == "record" && !t.constructor.is_empty() && declared_arity(&t.constructor) != t.fields.len() {
            problems.push(format!(
                "{} is a record, whose constructor takes exactly its components; a value it generates belongs in a static factory method",
                t.class
            ));
        }
    }
    for test in &design.tests {
        if !test.call.contains('(') {
            problems.push(format!(
                "the call `{}` is not a Java call; write the expression with its inputs",
                test.call
            ));
        }
    }
    if design.subject_setup.trim().is_empty() {
        problems.push("subject_setup is empty; give the expression that builds the object under test".into());
    }
    problems
}

/// The capitalised names a design uses as types (in fields, the constructor, method signatures and the subject
/// expression) that are not a class of this or an earlier step, nor in the project or on its classpath. A one-letter
/// name is a type variable. With no resolved index nothing is claimed: no data is not absence.
pub fn unknown_types(design: &StepDesign, known: &HashSet<String>, index: Option<&ClassIndex>) -> Vec<String> {
    let Some(index) = index.filter(|i| i.resolved()) else {
        return Vec::new();
    };
    let mut text = vec![design.subject_setup.clone()];
    for t in &design.types {
        text.push(t.constructor.clone());
        text.extend(t.fields.iter().map(|f| f.ty.clone()));
        text.extend(t.methods.iter().map(|m| m.signature.clone()));
    }
    let mut unknown: Vec<String> = Vec::new();
    let without_strings: Vec<String> = text.iter().map(|t| strip_strings(t)).collect();
    for word in without_strings
        .iter()
        .flat_map(|t| t.split(|c: char| !c.is_alphanumeric() && c != '_'))
    {
        let capital = word.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        let all_caps = word
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !capital || word.len() < 2 || all_caps {
            continue;
        }
        if !known.contains(word) && index.candidates(word).is_empty() && !unknown.iter().any(|u| u == word) {
            unknown.push(word.to_string());
        }
    }
    unknown
        .into_iter()
        .map(|u| {
            format!("{u} is not a class of this step, of an earlier step, of the project or of its libraries; use only those")
        })
        .collect()
}

/// The text with every double-quoted literal removed, so the words inside one are not read as names.
fn strip_strings(text: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    let mut escaped = false;
    for c in text.chars() {
        match (inside, escaped, c) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (true, false, '"') => inside = false,
            (true, ..) => {}
            (false, _, '"') => inside = true,
            (false, ..) => out.push(c),
        }
    }
    out
}

/// The number of arguments in the parenthesised text that opens at `open` (a `(`), or `None` when it never closes.
fn arity_at(text: &str, open: usize) -> Option<usize> {
    let rest = &text[open + 1..];
    let (mut depth, mut commas, mut any) = (0usize, 0usize, false);
    let mut inside = false;
    let mut escaped = false;
    for c in rest.chars() {
        if inside {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => inside = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => {
                inside = true;
                any = true;
            }
            '(' | '[' | '{' | '<' => {
                depth += 1;
                any = true;
            }
            ')' | ']' | '}' | '>' if depth > 0 => depth -= 1,
            ')' => return Some(if any { commas + 1 } else { 0 }),
            ',' if depth == 0 => commas += 1,
            c if !c.is_whitespace() => any = true,
            _ => {}
        }
    }
    None
}

/// The number of parameters in a declaration's parameter list (`(String name, Map<String, Integer> m)` is two).
fn declared_arity(params: &str) -> usize {
    let open = params.find('(').unwrap_or(0);
    arity_at(params, open).unwrap_or(0)
}

/// Calls in the tests whose argument count differs from the design: a method or constructor of this step's classes
/// called with a different number of arguments than the design declares (and no overload with that many).
pub fn call_mismatches(design: &StepDesign) -> Vec<String> {
    let mut problems = Vec::new();
    for test in &design.tests {
        let call = &test.call;
        let masked = strip_strings(call);
        // Where in the masked text the original call's quotes were removed the offsets differ; recompute on the
        // original text instead, ignoring matches inside literals by checking the masked text contains the name.
        for t in &design.types {
            let name = simple(&t.class);
            let mut targets: Vec<(String, Vec<usize>)> =
                t.methods.iter().fold(Vec::<(String, Vec<usize>)>::new(), |mut acc, m| {
                    let head = m.signature.split('(').next().unwrap_or("").trim();
                    let method = head.rsplit(' ').next().unwrap_or("").to_string();
                    let arity = declared_arity(&m.signature);
                    match acc.iter_mut().find(|(n, _)| *n == method) {
                        Some((_, arities)) => arities.push(arity),
                        None => acc.push((method, vec![arity])),
                    }
                    acc
                });
            if !t.constructor.is_empty() {
                targets.push((format!("new {name}"), vec![declared_arity(&t.constructor)]));
            } else if t.shape == "class" {
                targets.push((format!("new {name}"), vec![0]));
            }
            for (target, arities) in targets {
                let needle = format!("{target}(");
                if !masked.contains(&needle) {
                    continue;
                }
                let mut from = 0;
                while let Some(at) = call[from..].find(&needle) {
                    let open = from + at + needle.len() - 1;
                    if let Some(used) = arity_at(call, open) {
                        if !arities.contains(&used) {
                            let declared: Vec<String> = arities.iter().map(|a| a.to_string()).collect();
                            problems.push(format!(
                                "the call `{call}` passes {used} argument(s) to {target}, which {} declares with {} parameter(s)",
                                t.class,
                                declared.join(" or ")
                            ));
                        }
                    }
                    from = open + 1;
                }
            }
        }
    }
    problems.sort();
    problems.dedup();
    problems
}

/// A call that names the subject (`subject_setup.f(x)` or `subject.f(x)`) means the expression `subject_setup` defines.
/// The name is the prompt's own, so the call is written out in full here instead of sending the model back to do it.
pub fn resolve_subject(design: &mut StepDesign) {
    let setup = design.subject_setup.trim().to_string();
    if setup.is_empty() {
        return;
    }
    for test in &mut design.tests {
        let mut out = String::new();
        let mut word = String::new();
        let chars: Vec<char> = test.call.chars().collect();
        let mut i = 0;
        let mut inside = false;
        while i < chars.len() {
            let c = chars[i];
            if c == '"' && (i == 0 || chars[i - 1] != '\\') {
                inside = !inside;
            }
            if !inside && (c.is_alphanumeric() || c == '_') {
                word.push(c);
            } else {
                let alias = matches!(word.as_str(), "subject_setup" | "subject") && c == '.' && !out.ends_with('.');
                out.push_str(if alias { &setup } else { &word });
                word.clear();
                out.push(c);
            }
            i += 1;
        }
        out.push_str(&word);
        test.call = out;
    }
}

pub fn parse(content: &str, classes: &[String]) -> Result<StepDesign, Vec<String>> {
    let mut design: StepDesign =
        serde_json::from_str(content).map_err(|e| vec![format!("the answer is not a design: {e}")])?;
    resolve_subject(&mut design);
    match problems(&design, classes) {
        p if p.is_empty() => Ok(design),
        p => Err(p),
    }
}

/// The design as the step's instruction carries it: each class's declaration and the cases to test.
pub fn render(design: &StepDesign) -> String {
    let mut out = String::from("Design:\n");
    for t in &design.types {
        let name = simple(&t.class);
        let fields: Vec<String> = t.fields.iter().map(|f| format!("{} {}", f.ty, f.name)).collect();
        match t.shape.as_str() {
            "record" => out.push_str(&format!("record {name}({})\n", fields.join(", "))),
            shape => {
                out.push_str(&format!("{shape} {name}\n"));
                for f in &t.fields {
                    out.push_str(&format!("  field {} {}\n", f.ty, f.name));
                }
                if !t.constructor.is_empty() {
                    out.push_str(&format!("  constructor {name}{}\n", t.constructor));
                }
            }
        }
        for m in &t.methods {
            out.push_str(&format!("  {}: {}\n", m.signature, m.does));
        }
    }
    out.push_str(&format!("Test subject: {}\nTests:\n", design.subject_setup));
    for test in &design.tests {
        out.push_str(&format!("- {} -> {}\n", test.call, test.expect));
    }
    out
}

/// The qualified production classes a step creates.
fn classes_of(scope: &Scope) -> Vec<String> {
    scope
        .create
        .iter()
        .filter(|p| p.starts_with("src/main/") && p.ends_with(".java"))
        .filter_map(|p| java::qualified_name_of_source(p))
        .collect()
}

fn system_rules() -> &'static str {
    "\
You design the classes of one step of an implementation plan for a Java Maven project.
ALWAYS design every class listed for the step, each once.
ALWAYS give each class its shape (record, class or interface), its fields with their Java types, its constructor parameters and every method signature with what it does.
ALWAYS use only the classes listed in the earlier designs, the classes this step creates, and the JDK.
ALWAYS write each test's call as a Java expression with every input written out as a value, never as a description of a value; a string of a given length is written \"x\".repeat(50).
ALWAYS write expect as the result the test asserts.
ALWAYS write subject_setup as the Java expression that builds the object under test with its collaborators created with new."
}

fn messages(
    story: &str,
    instruction: &str,
    classes: &[String],
    scenarios: &[String],
    facts: &str,
    earlier: &[String],
    previous: Option<&[String]>,
) -> Vec<Message> {
    let mut user = String::new();
    if !story.trim().is_empty() {
        user.push_str(&format!("## Story\n{}\n\n", story.trim()));
    }
    user.push_str(&format!("## Step\n{instruction}\n\n## Classes to design\n"));
    for c in classes {
        user.push_str(&format!("- {c}\n"));
    }
    if !scenarios.is_empty() {
        user.push_str("\n## Scenarios the step helps make pass\n");
        user.push_str(&scenarios.join("\n"));
    }
    user.push_str(&format!("\n## Project\n{facts}\n"));
    if !earlier.is_empty() {
        user.push_str("\n## Designs of earlier steps\n");
        user.push_str(&earlier.join("\n"));
    }
    if let Some(problems) = previous {
        user.push_str("\n## Your previous design had these problems\n");
        for p in problems {
            user.push_str(&format!("- {p}\n"));
        }
        user.push_str("Design the step again and fix every one of them.\n");
    }
    vec![Message::system(system_rules()), Message::user(&user)]
}

/// A step left as it was, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub step_id: String,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Designed {
    pub plan: Plan,
    /// The steps whose design failed twice; they keep their original instruction.
    pub notes: Vec<Note>,
}

/// Designs every step that creates production classes and appends the design to its instruction.
pub fn design_plan(
    generator: &dyn Generator,
    plan: &Plan,
    facts: &RepoFacts,
    feature: &Feature,
    root: &Path,
    classpath: Option<&ClassIndex>,
) -> Designed {
    let mut designed = plan.clone();
    let mut notes = Vec::new();
    let mut earlier: Vec<String> = Vec::new();
    // The simple names of the classes the plan creates up to this step: a design may name these, besides what exists.
    let mut known: HashSet<String> = HashSet::new();
    for step in &mut designed.steps {
        let classes = classes_of(&step.scope);
        if classes.is_empty() {
            continue;
        }
        known.extend(classes.iter().map(|c| simple(c).to_string()));
        let scenarios: Vec<String> = step
            .covers
            .iter()
            .filter_map(|id| feature.render_scenario(id))
            .collect();
        let layers = facts.layers_of_classes(root, &classes);
        let facts_text = facts.render_for_step(&layers);
        let mut previous: Option<Vec<String>> = None;
        let mut outcome: Result<StepDesign, Vec<String>> = Err(vec![]);
        for attempt in 1..=MAX_ATTEMPTS {
            let request = StructuredRequest {
                purpose: format!("design-{}-{attempt}", step.id),
                messages: messages(
                    plan.story.as_deref().unwrap_or_default(),
                    &step.instruction,
                    &classes,
                    &scenarios,
                    &facts_text,
                    &earlier,
                    previous.as_deref(),
                ),
                schema_name: "design".into(),
                schema: schema(&classes),
                thinking: false,
            };
            outcome = match generator.complete_structured(&request) {
                Err(e) => Err(vec![format!("the model call failed: {e}")]),
                Ok(r) if r.finish_reason == "length" => Err(vec![
                    "The answer hit the token limit; design with fewer or shorter entries.".into(),
                ]),
                Ok(r) => parse(&r.content, &classes).and_then(|design| {
                    // Only the classes of this and the earlier steps are known: a later class does not exist yet.
                    let mut p = unknown_types(&design, &known, classpath);
                    p.extend(call_mismatches(&design));
                    match p {
                        p if p.is_empty() => Ok(design),
                        p => Err(p),
                    }
                }),
            };
            match &outcome {
                Ok(_) => break,
                Err(p) => previous = Some(p.clone()),
            }
        }
        match outcome {
            Ok(design) => {
                let text = render(&design);
                step.instruction = format!("{}\n\n{}", step.instruction.trim_end(), text);
                earlier.push(format!("Step {}:\n{text}", step.id));
            }
            Err(problems) => notes.push(Note {
                step_id: step.id.clone(),
                problems,
            }),
        }
    }
    Designed { plan: designed, notes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scaffold;
    use canopy_implementer::llm::{LlmError, LlmResponse};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    const WIDGET: &str = "com.example.widgets.Widget";
    const SERVICE: &str = "com.example.widgets.WidgetService";

    fn classes(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    const RECORD_DESIGN: &str = r#"{
      "types": [{"class": "com.example.widgets.Widget", "shape": "record",
                 "fields": [{"name": "id", "type": "UUID"}, {"name": "name", "type": "String"}],
                 "constructor": "",
                 "methods": [{"signature": "static Widget create(String name)", "does": "gives the widget a random id"}]}],
      "subject_setup": "Widget.create(\"Alpha\")",
      "tests": [{"call": "Widget.create(\"Alpha\")", "expect": "keeps the name Alpha and gives an id"},
                {"call": "Widget.create(\"x\".repeat(51))", "expect": "throws IllegalArgumentException"}]
    }"#;

    const SERVICE_DESIGN: &str = r#"{
      "types": [{"class": "com.example.widgets.WidgetService", "shape": "class",
                 "fields": [{"name": "widgets", "type": "List<Widget>"}],
                 "constructor": "()",
                 "methods": [{"signature": "Widget create(String name)", "does": "creates and stores a widget"}]}],
      "subject_setup": "new WidgetService()",
      "tests": [{"call": "new WidgetService().create(\"Alpha\")", "expect": "returns a widget named Alpha"}]
    }"#;

    #[test]
    fn the_schema_offers_only_the_steps_own_classes_and_bounds_every_list_and_string() {
        let schema = schema(&classes(&[WIDGET]));
        let item = &schema["properties"]["types"]["items"]["properties"];
        assert_eq!(item["class"]["enum"], json!([WIDGET]));
        assert_eq!(item["shape"]["enum"], json!(["record", "class", "interface"]));
        assert_eq!(schema["properties"]["tests"]["minItems"], 1);
        assert!(schema["properties"]["tests"]["maxItems"].is_number());
        assert!(item["methods"]["maxItems"].is_number() && item["fields"]["maxItems"].is_number());
        let text = schema.to_string();
        assert!(
            !text.contains("\"*\"") && !text.contains(")*") && !text.contains("]*"),
            "an unbounded quantifier: {text}"
        );
    }

    #[test]
    fn a_design_must_cover_every_class_of_the_step_once_and_say_how_the_subject_is_built() {
        let design: StepDesign = serde_json::from_str(RECORD_DESIGN).unwrap();
        assert!(problems(&design, &classes(&[WIDGET])).is_empty());
        let missing = problems(&design, &classes(&[WIDGET, SERVICE]));
        assert_eq!(missing.len(), 1, "{missing:?}");
        assert!(missing[0].contains("WidgetService has no design"), "{missing:?}");
        let other = problems(&design, &classes(&[SERVICE]));
        assert!(
            other.iter().any(|p| p.contains("Widget is not a class of this step")),
            "{other:?}"
        );
        let mut empty = design.clone();
        empty.subject_setup = " ".into();
        assert!(problems(&empty, &classes(&[WIDGET]))
            .iter()
            .any(|p| p.contains("subject_setup")));
    }

    #[test]
    fn a_record_whose_constructor_differs_from_its_components_is_a_problem() {
        let mut design: StepDesign = serde_json::from_str(RECORD_DESIGN).unwrap();
        assert!(problems(&design, &classes(&[WIDGET])).is_empty());
        design.types[0].constructor = "(String name)".into();
        let found = problems(&design, &classes(&[WIDGET]));
        assert!(
            found
                .iter()
                .any(|p| p.contains("is a record") && p.contains("static factory")),
            "{found:?}"
        );
        design.types[0].constructor = "(UUID id, String name)".into();
        assert!(problems(&design, &classes(&[WIDGET])).is_empty());
    }

    #[test]
    fn a_test_whose_call_is_a_description_is_a_problem() {
        let mut design: StepDesign = serde_json::from_str(RECORD_DESIGN).unwrap();
        design.tests[0].call = "a name of 51 characters".into();
        let found = problems(&design, &classes(&[WIDGET]));
        assert!(found.iter().any(|p| p.contains("is not a Java call")), "{found:?}");
    }

    #[test]
    fn a_call_on_the_subject_is_written_out_with_the_expression_that_builds_it() {
        let reply = SERVICE_DESIGN.replace(
            "new WidgetService().create(\\\"Alpha\\\")",
            "subject.create(\\\"subject.x\\\") and subject_setup.create(\\\"Beta\\\")",
        );
        let design = parse(&reply, &classes(&[SERVICE])).unwrap();
        assert_eq!(
            design.tests[0].call, "new WidgetService().create(\"subject.x\") and new WidgetService().create(\"Beta\")",
            "an alias becomes the setup, a word inside a string literal stays"
        );
        let mut plain = design.clone();
        plain.tests[0].call = "mysubject.create(1) + subjects.x()".into();
        resolve_subject(&mut plain);
        assert_eq!(
            plain.tests[0].call, "mysubject.create(1) + subjects.x()",
            "only the whole word is an alias"
        );
    }

    #[test]
    fn a_type_that_is_nowhere_is_a_problem_only_when_the_classpath_is_known() {
        use canopy_implementer::classpath::Origin;
        let design: StepDesign =
            serde_json::from_str(SERVICE_DESIGN.replace("List<Widget>", "WidgetStore").as_str()).unwrap();
        let known: HashSet<String> = ["Widget", "WidgetService"].iter().map(|s| s.to_string()).collect();
        let mut index = ClassIndex::resolved_empty();
        index.add_entries(["java/lang/String.class", "java/util/List.class"], Origin::Jdk);
        let found = unknown_types(&design, &known, Some(&index));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("WidgetStore is not a class"), "{found:?}");
        assert!(unknown_types(&design, &known, Some(&ClassIndex::unresolved())).is_empty());
        assert!(unknown_types(&design, &known, None).is_empty());
        let fine: StepDesign = serde_json::from_str(SERVICE_DESIGN).unwrap();
        assert!(
            unknown_types(&fine, &known, Some(&index)).is_empty(),
            "List, String and Widget are all known"
        );
    }

    #[test]
    fn words_inside_a_string_literal_are_not_type_names() {
        let design: StepDesign = serde_json::from_str(RECORD_DESIGN).unwrap();
        let known: HashSet<String> = ["Widget"].iter().map(|s| s.to_string()).collect();
        let mut index = ClassIndex::resolved_empty();
        index.add_entries(
            ["java/lang/String.class", "java/util/UUID.class"],
            canopy_implementer::classpath::Origin::Jdk,
        );
        assert!(
            unknown_types(&design, &known, Some(&index)).is_empty(),
            "\"Alpha\" is a literal"
        );
    }

    #[test]
    fn a_test_call_with_a_different_number_of_arguments_than_the_design_declares_is_a_problem() {
        let mut design: StepDesign = serde_json::from_str(SERVICE_DESIGN).unwrap();
        assert!(call_mismatches(&design).is_empty(), "{:?}", call_mismatches(&design));
        design.tests[0].call = "new WidgetService().create(\"Alpha\", \"Beta\")".into();
        let found = call_mismatches(&design);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("passes 2 argument(s) to create") && found[0].contains("1 parameter(s)"),
            "{found:?}"
        );
        design.tests[0].call = "new WidgetService(1, 2).create(\"a,b\")".into();
        let ctor = call_mismatches(&design);
        assert!(
            ctor.iter()
                .any(|p| p.contains("to new WidgetService") && p.contains("0 parameter(s)")),
            "{ctor:?}"
        );
        assert_eq!(declared_arity("(Map<String, Integer> m, int n)"), 2);
        assert_eq!(declared_arity("()"), 0);
    }

    #[test]
    fn a_design_renders_each_declaration_the_subject_and_the_cases_for_the_implementer_to_read() {
        let text = render(&serde_json::from_str(RECORD_DESIGN).unwrap());
        assert!(text.contains("record Widget(UUID id, String name)"), "{text}");
        assert!(
            text.contains("static Widget create(String name): gives the widget a random id"),
            "{text}"
        );
        assert!(text.contains("Test subject: Widget.create(\"Alpha\")"), "{text}");
        assert!(
            text.contains("- Widget.create(\"x\".repeat(51)) -> throws IllegalArgumentException"),
            "{text}"
        );
        let service = render(&serde_json::from_str(SERVICE_DESIGN).unwrap());
        assert!(
            service.contains("class WidgetService\n  field List<Widget> widgets\n  constructor WidgetService()"),
            "{service}"
        );
    }

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

    const PLAN: &str = r#"
id: p
steps:
  - id: model
    instruction: Create the widget.
    create: [src/main/java/com/example/widgets/Widget.java, src/test/java/com/example/widgets/WidgetTest.java]
    covers: [first]
  - id: service
    instruction: Create the service.
    create: [src/main/java/com/example/widgets/WidgetService.java, src/test/java/com/example/widgets/WidgetServiceTest.java]
    covers: [first]
  - id: more-tests
    instruction: Add tests only.
    modify: [src/test/java/com/example/widgets/WidgetTest.java]
    covers: [first]
"#;

    fn run(replies: Vec<Result<LlmResponse, LlmError>>) -> (Designed, Vec<StructuredRequest>) {
        let dir = scaffold();
        let requests = Arc::new(Mutex::new(vec![]));
        let generator = Script {
            replies: Mutex::new(replies.into()),
            requests: requests.clone(),
        };
        let feature =
            Feature::parse("Feature: F\n  Scenario: First\n    When x\n    Then the status is 201\n").unwrap();
        let facts = RepoFacts::collect(dir.path()).unwrap();
        let plan = Plan::from_yaml(PLAN).unwrap();
        let designed = design_plan(&generator, &plan, &facts, &feature, dir.path(), None);
        let seen = requests.lock().unwrap().clone();
        (designed, seen)
    }

    #[test]
    fn every_step_that_creates_production_classes_is_designed_and_the_design_joins_its_instruction() {
        let (designed, requests) = run(vec![reply(RECORD_DESIGN), reply(SERVICE_DESIGN)]);
        assert!(designed.notes.is_empty(), "{:?}", designed.notes);
        assert_eq!(requests.len(), 2, "a step that only adds tests is not designed");
        assert_eq!(requests[0].purpose, "design-model-1");
        let model = &designed.plan.steps[0].instruction;
        assert!(
            model.starts_with("Create the widget.") && model.contains("record Widget(UUID id, String name)"),
            "{model}"
        );
        assert_eq!(designed.plan.steps[2].instruction, "Add tests only.");
    }

    #[test]
    fn the_story_is_in_the_prompt_because_its_limits_decide_the_boundary_values() {
        let dir = scaffold();
        let requests = Arc::new(Mutex::new(vec![]));
        let generator = Script {
            replies: Mutex::new(vec![reply(RECORD_DESIGN)].into()),
            requests: requests.clone(),
        };
        let feature = Feature::parse("Feature: F\n  Scenario: First\n    When x\n").unwrap();
        let facts = RepoFacts::collect(dir.path()).unwrap();
        let mut plan = Plan::from_yaml(PLAN).unwrap();
        plan.steps.truncate(1);
        plan.story = Some("A name longer than 50 characters is rejected.".into());
        design_plan(&generator, &plan, &facts, &feature, dir.path(), None);
        let prompt = requests.lock().unwrap()[0].messages.last().unwrap().content.clone();
        assert!(
            prompt.starts_with("## Story\nA name longer than 50 characters is rejected."),
            "{prompt}"
        );
    }

    #[test]
    fn a_step_is_shown_the_scenarios_it_covers_the_project_and_the_designs_of_the_earlier_steps() {
        let (_, requests) = run(vec![reply(RECORD_DESIGN), reply(SERVICE_DESIGN)]);
        let first = &requests[0].messages.last().unwrap().content;
        assert!(
            first.contains("the status is 201") && first.contains("Root package"),
            "{first}"
        );
        assert!(!first.contains("Designs of earlier steps"), "{first}");
        let second = &requests[1].messages.last().unwrap().content;
        assert!(
            second.contains("Designs of earlier steps") && second.contains("record Widget(UUID id, String name)"),
            "{second}"
        );
        assert!(second.contains("- com.example.widgets.WidgetService"), "{second}");
    }

    #[test]
    fn a_design_with_problems_is_retried_once_with_the_problems_named_and_then_the_step_keeps_its_instruction() {
        let wrong_class = RECORD_DESIGN.replace("com.example.widgets.Widget\"", "com.example.widgets.Other\"");
        let (designed, requests) = run(vec![reply(&wrong_class), reply(&wrong_class), reply(SERVICE_DESIGN)]);
        assert_eq!(requests.len(), 3);
        assert!(requests[1]
            .messages
            .last()
            .unwrap()
            .content
            .contains("is not a class of this step"));
        assert_eq!(designed.plan.steps[0].instruction, "Create the widget.");
        assert_eq!(designed.notes.len(), 1);
        assert_eq!(designed.notes[0].step_id, "model");
        assert!(designed.plan.steps[1].instruction.contains("class WidgetService"));
    }
}
