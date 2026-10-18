//! The layered Planner: the steps come from the project's API, the model supplies only each step's rules and cases.
//!
//! The API input lists one unit per line (a record, or an interface with its implementation, or a service) in the order
//! they are built. Each unit becomes one step whose files follow from the type names, so the plan's shape is computed, not
//! drafted. For each unit the model is asked, in a schema that cannot leave them out, for the rules of the unit (exception
//! types, exact messages, thresholds) and for cases: a call with concrete values and the expected result. The text the
//! Implementer reads is rendered from those fields. This is the plan shape the hand-written plans that pass have (see
//! `canopy-implementer/docs/prompt-and-plan-changelog.md`, entries B1 to B10 and C17). Nothing here checks that the model's
//! rules or values are right; it checks that every literal the feature states reached some step.

use std::collections::BTreeSet;

use canopy_implementer::implementer::{Scope, StepKind};
use canopy_implementer::llm::{Generator, Message, StructuredRequest};
use canopy_implementer::plan::{Plan, PlanStep};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::gherkin::Feature;

const MAX_ATTEMPTS: u32 = 2;

/// One line of the API: the types it declares, in the order they appear.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub line: String,
    pub types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Api {
    pub package: String,
    pub units: Vec<Unit>,
}

/// `Package a.b.c.` on the first line, then one unit per line.
pub fn parse_api(text: &str) -> Result<Api, String> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next().ok_or("the API is empty")?;
    let package = first
        .strip_prefix("Package ")
        .map(|p| p.trim_end_matches('.').trim().to_string())
        .filter(|p| !p.is_empty())
        .ok_or_else(|| format!("the first line must be `Package <name>.`, found `{first}`"))?;
    let mut units = Vec::new();
    for line in lines {
        let types = types_of(line);
        if types.is_empty() {
            return Err(format!("no record, enum, class or interface in `{line}`"));
        }
        units.push(Unit { line: line.to_string(), types });
    }
    if units.is_empty() {
        return Err("the API declares no unit".into());
    }
    Ok(Api { package, units })
}

fn types_of(line: &str) -> Vec<String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut found: Vec<String> = Vec::new();
    for pair in words.windows(2) {
        if matches!(pair[0], "record" | "enum" | "class" | "interface") {
            let name: String = pair[1].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if name.starts_with(|c: char| c.is_uppercase()) && !found.contains(&name) {
                found.push(name);
            }
        }
    }
    found
}

impl Unit {
    /// The class whose test the step adds: the last type the line declares.
    pub fn primary(&self) -> &str {
        self.types.last().expect("a unit declares a type")
    }

    pub fn main_files(&self, package: &str) -> Vec<String> {
        let dir = package.replace('.', "/");
        self.types.iter().map(|t| format!("src/main/java/{dir}/{t}.java")).collect()
    }

    pub fn test_file(&self, package: &str) -> String {
        format!("src/test/java/{}/{}Test.java", package.replace('.', "/"), self.primary())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Case {
    pub call: String,
    pub expected: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Draft {
    pub rules: String,
    pub cases: Vec<Case>,
    pub covers: Vec<String>,
}

/// What the model may answer for one unit.
pub fn schema(scenario_ids: &[String]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["rules", "cases", "covers"],
        "properties": {
            "rules": {"type": "string", "minLength": 1, "maxLength": 1200},
            "cases": {
                "type": "array",
                "minItems": 1,
                "maxItems": 16,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["call", "expected"],
                    "properties": {
                        "call": {"type": "string", "minLength": 1, "maxLength": 240},
                        "expected": {"type": "string", "minLength": 1, "maxLength": 200}
                    }
                }
            },
            "covers": {"type": "array", "items": {"type": "string", "enum": scenario_ids}}
        }
    })
}

const MUST: &str = "It MUST contain one test for each item of this checklist, and you must not leave any item out. \
Before you submit, go through the list and check that every item has its test.";

/// The instruction the Implementer reads, written from the fields and nothing else.
pub fn render_instruction(api: &Api, index: usize, draft: &Draft) -> String {
    let unit = &api.units[index];
    let api_lines: Vec<&str> = api.units[..=index].iter().map(|u| u.line.as_str()).collect();
    let mut out = format!(
        "Create the following in the package {}: {}\nRules: {}\nAPI the tests use: {}\nAdd {}Test. {MUST}\n",
        api.package,
        unit.line,
        draft.rules.trim(),
        api_lines.join(" "),
        unit.primary()
    );
    for (i, case) in draft.cases.iter().enumerate() {
        out.push_str(&format!("{}. {}: {}\n", i + 1, case.call.trim().trim_end_matches('.'), case.expected.trim()));
    }
    out.trim_end().to_string()
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// The quoted strings and the numbers a feature states (dates included), as they appear in its scenarios.
pub fn literals(feature: &Feature) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for id in feature.scenario_ids() {
        let text = feature.render_scenario(&id).unwrap_or_default();
        for line in text.lines() {
            let parts: Vec<&str> = line.split('"').collect();
            for (i, part) in parts.iter().enumerate() {
                if i % 2 == 1 {
                    found.insert(part.to_string());
                }
            }
            let outside: String = parts.iter().step_by(2).copied().collect::<Vec<_>>().join(" ");
            for token in outside.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')) {
                let token = token.trim_matches(|c| c == '.' || c == '-');
                if token.starts_with(|c: char| c.is_ascii_digit()) {
                    found.insert(token.to_string());
                }
            }
        }
    }
    found
}

/// The literals of the feature that no step's instruction contains.
pub fn missing_literals(feature: &Feature, plan: &Plan) -> Vec<String> {
    let text: String = plan.steps.iter().map(|s| s.instruction.as_str()).collect::<Vec<_>>().join("\n");
    literals(feature).into_iter().filter(|l| !text.contains(l.as_str())).collect()
}

/// What the layered Planner could not do, kept beside the plan.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Notes {
    pub missing_literals: Vec<String>,
    pub uncovered_scenarios: Vec<String>,
    /// One entry per unit that never got a usable answer.
    pub failed_units: Vec<String>,
}

#[derive(Debug)]
pub struct Layered {
    /// `None` when a unit never got a usable answer.
    pub plan: Option<Plan>,
    pub notes: Notes,
}

pub struct Input<'a> {
    pub id: &'a str,
    pub story: &'a str,
    pub feature_path: Option<&'a str>,
    pub feature: &'a Feature,
    pub api: &'a Api,
}

fn system() -> String {
    "\
You write one step of an implementation plan for a Java Maven project, from the story, the scenarios and the project's fixed API.
ALWAYS state the rules of the unit as behaviour: every exception type, every exact message, every threshold and which side of it counts, every default.
ALWAYS write each case as a call of the API with concrete values and its expected result, the way a test asserts it.
ALWAYS write a case for every example row of every scenario that concerns this unit, and one case for each side of every threshold or step in the rules.
NEVER restate the API signatures in the rules."
        .to_string()
}

fn messages(input: &Input, index: usize, previous: Option<(&str, &[String])>) -> Vec<Message> {
    let mut user = format!("## Story\n{}\n\n## API (fixed)\n", input.story.trim());
    for unit in &input.api.units {
        user.push_str(&format!("{}\n", unit.line));
    }
    user.push_str("\n## Scenarios\n");
    for id in input.feature.scenario_ids() {
        if let Some(text) = input.feature.render_scenario(&id) {
            user.push_str(&format!("### {id}\n{text}\n"));
        }
    }
    let unit = &input.api.units[index];
    user.push_str(&format!(
        "\n## This step\nUnit {} of {}: {}\nWrite the rules and the cases of this unit only.\n",
        index + 1,
        input.api.units.len(),
        unit.line
    ));
    if let Some((raw, problems)) = previous {
        user.push_str(&format!("\n## Your previous answer\n```json\n{raw}\n```\n\n## Problems\n"));
        for problem in problems {
            user.push_str(&format!("- {problem}\n"));
        }
        user.push_str("\nReturn the complete corrected answer.\n");
    }
    vec![Message::system(&system()), Message::user(&user)]
}

fn parse(raw: &str, scenario_ids: &[String]) -> Result<Draft, Vec<String>> {
    let draft: Draft = serde_json::from_str(raw).map_err(|e| vec![format!("the answer is not valid: {e}")])?;
    let mut problems = Vec::new();
    if draft.rules.trim().is_empty() {
        problems.push("rules is empty".to_string());
    }
    if draft.cases.is_empty() {
        problems.push("cases is empty".to_string());
    }
    for (i, case) in draft.cases.iter().enumerate() {
        if case.call.trim().is_empty() || case.expected.trim().is_empty() {
            problems.push(format!("case {} has an empty call or an empty expected result", i + 1));
        }
    }
    for id in &draft.covers {
        if !scenario_ids.contains(id) {
            problems.push(format!("covers names `{id}`, which is not a scenario"));
        }
    }
    if problems.is_empty() {
        Ok(draft)
    } else {
        Err(problems)
    }
}

pub fn plan_layered(generator: &dyn Generator, input: &Input) -> Layered {
    let scenario_ids = input.feature.scenario_ids();
    let mut steps: Vec<PlanStep> = Vec::new();
    let mut notes = Notes::default();
    let mut earlier_main: Vec<String> = Vec::new();
    for (index, unit) in input.api.units.iter().enumerate() {
        let mut previous: Option<(String, Vec<String>)> = None;
        let mut drafted: Option<Draft> = None;
        for attempt in 1..=MAX_ATTEMPTS {
            let request = StructuredRequest {
                purpose: format!("layered-{}-{attempt}", index + 1),
                messages: messages(input, index, previous.as_ref().map(|(r, p)| (r.as_str(), p.as_slice()))),
                schema_name: "unit".into(),
                schema: schema(&scenario_ids),
                thinking: false,
            };
            match generator.complete_structured(&request) {
                Err(e) => previous = Some((String::new(), vec![format!("the model call failed: {e}")])),
                Ok(r) if r.finish_reason == "length" => {
                    previous = Some((
                        r.content,
                        vec!["The answer hit the token limit; write fewer or shorter cases.".to_string()],
                    ))
                }
                Ok(r) => match parse(&r.content, &scenario_ids) {
                    Ok(draft) => {
                        drafted = Some(draft);
                        break;
                    }
                    Err(problems) => previous = Some((r.content, problems)),
                },
            }
        }
        let Some(draft) = drafted else {
            notes.failed_units.push(unit.line.clone());
            continue;
        };
        let main = unit.main_files(&input.api.package);
        let mut create = main.clone();
        create.push(unit.test_file(&input.api.package));
        steps.push(PlanStep {
            id: format!("{}-{}", index + 1, kebab(unit.primary())),
            instruction: render_instruction(input.api, index, &draft),
            kind: StepKind::BehaviorChange,
            context: None,
            scope: Scope { modify: vec![], create, read_only: earlier_main.clone() },
            covers: draft.covers.clone(),
        });
        earlier_main.extend(main);
    }
    if !notes.failed_units.is_empty() {
        return Layered { plan: None, notes };
    }
    let plan = Plan {
        id: input.id.to_string(),
        story: Some(input.story.to_string()),
        feature: input.feature_path.map(str::to_string),
        steps,
    };
    notes.missing_literals = missing_literals(input.feature, &plan);
    let covered: BTreeSet<&String> = plan.steps.iter().flat_map(|s| &s.covers).collect();
    notes.uncovered_scenarios = scenario_ids.iter().filter(|id| !covered.contains(id)).cloned().collect();
    Layered { plan: Some(plan), notes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopy_implementer::llm::{LlmError, LlmResponse};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    const STOCK_API: &str = "Package no.experis.stock.\n\
record StockItem(String sku, int onHand); StockItem receive(int quantity).\n\
interface StockRepository: Optional<StockItem> findBySku(String sku). class InMemoryStockRepository implements it.\n\
class StockService, constructor StockService(StockRepository): int available(String sku).\n";

    const FEATURE: &str = "Feature: Stock\n  Scenario: Received stock is available\n    When 10 units of \"SKU-1\" are received\n    Then 10 units of \"SKU-1\" are available\n  Scenario: Reserving an unknown SKU is rejected\n    When 1 unit of \"SKU-9\" is reserved\n    Then the reservation is rejected with the message \"Unknown SKU\"\n";

    #[test]
    fn the_api_gives_the_package_and_one_unit_per_line_with_the_types_it_declares() {
        let api = parse_api(STOCK_API).unwrap();
        assert_eq!(api.package, "no.experis.stock");
        let types: Vec<Vec<&str>> = api.units.iter().map(|u| u.types.iter().map(String::as_str).collect()).collect();
        assert_eq!(types, vec![vec!["StockItem"], vec!["StockRepository", "InMemoryStockRepository"], vec!["StockService"]]);
        assert!(parse_api("record X().").is_err(), "no package line");
        assert!(parse_api("Package a.b.\nsomething without a type").is_err());
    }

    #[test]
    fn a_unit_creates_a_file_for_each_type_and_the_test_of_the_last_one() {
        let api = parse_api(STOCK_API).unwrap();
        let repo = &api.units[1];
        assert_eq!(
            repo.main_files(&api.package),
            ["src/main/java/no/experis/stock/StockRepository.java", "src/main/java/no/experis/stock/InMemoryStockRepository.java"]
        );
        assert_eq!(repo.test_file(&api.package), "src/test/java/no/experis/stock/InMemoryStockRepositoryTest.java");
    }

    fn draft(rules: &str, cases: &[(&str, &str)], covers: &[&str]) -> Draft {
        Draft {
            rules: rules.into(),
            cases: cases.iter().map(|(c, e)| Case { call: (*c).into(), expected: (*e).into() }).collect(),
            covers: covers.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn the_instruction_is_written_from_the_fields_with_the_api_so_far_and_numbered_cases() {
        let api = parse_api(STOCK_API).unwrap();
        let d = draft("receive adds to what is there.", &[("new StockItem(\"SKU-1\", 10).receive(5).onHand()", "is 15"), ("receive(0)", "throws IllegalArgumentException")], &[]);
        let text = render_instruction(&api, 1, &d);
        assert!(text.starts_with("Create the following in the package no.experis.stock: interface StockRepository"), "{text}");
        assert!(text.contains("\nRules: receive adds to what is there.\n"), "{text}");
        let api_line = text.lines().find(|l| l.starts_with("API the tests use:")).unwrap();
        assert!(api_line.contains("record StockItem") && api_line.contains("interface StockRepository") && !api_line.contains("StockService"), "{api_line}");
        assert!(text.contains("Add InMemoryStockRepositoryTest. It MUST contain one test for each item"), "{text}");
        assert!(text.contains("\n1. new StockItem(\"SKU-1\", 10).receive(5).onHand(): is 15\n2. receive(0): throws IllegalArgumentException"), "{text}");
    }

    #[test]
    fn the_literals_of_a_feature_are_its_quoted_strings_and_numbers() {
        let feature = Feature::parse(FEATURE).unwrap();
        let found = literals(&feature);
        for expected in ["SKU-1", "SKU-9", "Unknown SKU", "10", "1"] {
            assert!(found.contains(expected), "{expected}: {found:?}");
        }
    }

    fn script(replies: Vec<Result<LlmResponse, LlmError>>) -> (Script, Arc<Mutex<Vec<StructuredRequest>>>) {
        let requests = Arc::new(Mutex::new(Vec::new()));
        (Script { replies: Mutex::new(replies.into()), requests: requests.clone() }, requests)
    }

    struct Script {
        replies: Mutex<VecDeque<Result<LlmResponse, LlmError>>>,
        requests: Arc<Mutex<Vec<StructuredRequest>>>,
    }

    impl Generator for Script {
        fn complete_structured(&self, request: &StructuredRequest) -> Result<LlmResponse, LlmError> {
            self.requests.lock().unwrap().push(request.clone());
            self.replies.lock().unwrap().pop_front().expect("the script ran out of replies")
        }
    }

    fn reply(content: &str) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse { call_id: 1, content: content.into(), finish_reason: "stop".into(), prompt_tokens: 1, completion_tokens: 1 })
    }

    fn answer(rules: &str, cases: &[(&str, &str)], covers: &[&str]) -> Result<LlmResponse, LlmError> {
        let cases: Vec<Value> = cases.iter().map(|(c, e)| json!({"call": c, "expected": e})).collect();
        reply(&json!({"rules": rules, "cases": cases, "covers": covers}).to_string())
    }

    #[test]
    fn a_plan_has_one_step_per_unit_with_computed_files_read_only_earlier_files_and_the_covers_the_model_named() {
        let api = parse_api(STOCK_API).unwrap();
        let feature = Feature::parse(FEATURE).unwrap();
        let (generator, requests) = script(vec![
            answer("r1", &[("a", "b")], &["received-stock-is-available"]),
            reply("not json"),
            answer("r2", &[("c", "d")], &[]),
            answer("r3", &[("e", "f")], &["reserving-an-unknown-sku-is-rejected"]),
        ]);
        let result = plan_layered(&generator, &Input { id: "stock", story: "The story.", feature_path: Some("tasks/stock/feature.feature"), feature: &feature, api: &api });
        let plan = result.plan.expect("a plan");
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].id, "1-stock-item");
        assert_eq!(plan.steps[0].scope.create, ["src/main/java/no/experis/stock/StockItem.java", "src/test/java/no/experis/stock/StockItemTest.java"]);
        assert_eq!(plan.steps[1].scope.read_only, ["src/main/java/no/experis/stock/StockItem.java"]);
        assert_eq!(plan.steps[2].scope.read_only.len(), 3);
        assert_eq!(plan.steps[0].covers, ["received-stock-is-available"]);
        assert!(plan.steps[1].instruction.contains("Rules: r2"));
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 4, "the invalid answer cost one retry");
        let retry = &requests[2].messages[1].content;
        assert!(retry.contains("not json") && retry.contains("the answer is not valid"), "{retry}");
        assert!(requests[0].messages[1].content.contains("Unit 1 of 3: record StockItem"), "{}", requests[0].messages[1].content);
        assert_eq!(requests[0].schema["properties"]["covers"]["items"]["enum"][0], "received-stock-is-available");
    }

    #[test]
    fn the_notes_name_the_literals_no_step_carries_and_the_scenarios_no_step_covers() {
        let api = parse_api("Package a.b.\nclass A: void f().\n").unwrap();
        let feature = Feature::parse(FEATURE).unwrap();
        let (generator, _) = script(vec![answer("Rules say SKU-1 and 10.", &[("f()", "is 10")], &["received-stock-is-available"])]);
        let result = plan_layered(&generator, &Input { id: "x", story: "s", feature_path: None, feature: &feature, api: &api });
        assert_eq!(result.notes.uncovered_scenarios, ["reserving-an-unknown-sku-is-rejected"]);
        assert!(result.notes.missing_literals.contains(&"Unknown SKU".to_string()) && result.notes.missing_literals.contains(&"SKU-9".to_string()), "{:?}", result.notes);
        assert!(!result.notes.missing_literals.contains(&"SKU-1".to_string()));
    }

    #[test]
    fn a_unit_that_never_gets_a_usable_answer_leaves_no_plan() {
        let api = parse_api("Package a.b.\nclass A: void f().\n").unwrap();
        let feature = Feature::parse(FEATURE).unwrap();
        let (generator, requests) = script(vec![reply("x"), reply("y")]);
        let result = plan_layered(&generator, &Input { id: "x", story: "s", feature_path: None, feature: &feature, api: &api });
        assert!(result.plan.is_none());
        assert_eq!(result.notes.failed_units, ["class A: void f()."]);
        assert_eq!(requests.lock().unwrap().len(), 2, "one retry and no more");
    }
}
