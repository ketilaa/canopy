//! Stage 0 of the behavior-first planning pipeline (docs/design/behavior-first-planning.md):
//! Specification Completeness. Validates a story's specification is complete enough to safely
//! decompose into behaviors, before any behavior extraction begins — no architecture vocabulary
//! enters at this stage, only whether the specification itself is internally complete.

use crate::client::{LlmClient, LlmError};
use canopy_core::*;

/// Mechanically enumerates every (field, constraint) pair from the entity schema — one line per
/// constraint, in a fixed order. Live-verified: a single holistic "compare schema and scenarios"
/// prompt correctly found 4 of 5 real gaps against product-001's schema but silently missed one
/// field's max_length despite catching the identical constraint shape on three other fields —
/// not a conceptual failure, a coverage failure. Forcing an explicit per-constraint checklist
/// turns "notice everything that's missing" (holistic, omission-prone) into "answer yes/no for
/// each of these N already-enumerated items" (a much narrower task per item).
fn constraint_checklist(schema: &EntitySchema) -> Vec<String> {
    let mut items = Vec::new();
    for field in schema.mandatory.iter().chain(schema.optional.iter()) {
        let Some(v) = &field.validation else { continue };
        if let Some(n) = v.max_length {
            items.push(format!("Field '{}': max_length={n} — scenario testing a value longer than {n} characters?", field.name));
        }
        if let Some(n) = v.min_length {
            items.push(format!("Field '{}': min_length={n} — scenario testing a value shorter than {n} characters (including empty/missing)?", field.name));
        }
        if let Some(n) = v.min {
            items.push(format!("Field '{}': min={n} — scenario testing a value below {n}?", field.name));
        }
        if let Some(n) = v.max {
            items.push(format!("Field '{}': max={n} — scenario testing a value above {n}?", field.name));
        }
        if let Some(p) = &v.pattern {
            items.push(format!("Field '{}': pattern={p} — scenario testing a value that violates this pattern?", field.name));
        }
        if let Some(n) = v.max_items {
            items.push(format!("Field '{}': max_items={n} — scenario testing more than {n} items?", field.name));
        }
    }
    items
}

/// One line per scenario's own `then` clause — so "is this outcome observable" is answered
/// against an already-isolated single scenario, not inferred while reading the whole list.
fn scenario_checklist(scenarios: &[Scenario]) -> Vec<String> {
    scenarios.iter()
        .map(|s| format!("{}: then: {}", s.id, s.then.join("; ")))
        .collect()
}

fn numbered(items: &[String]) -> String {
    if items.is_empty() {
        return "None.".to_string();
    }
    items.iter().enumerate().map(|(i, item)| format!("{}. {item}", i + 1)).collect::<Vec<_>>().join("\n")
}

fn specification_completeness_prompt(story: &UserStory, spec: &IntentSpec, adrs: &[Adr]) -> String {
    let constraint_items = spec.entity_schema.as_ref()
        .map(constraint_checklist)
        .unwrap_or_default();
    let constraint_list = numbered(&constraint_items);
    let scenario_list = numbered(&scenario_checklist(&spec.scenarios));
    let open_questions = numbered(&spec.open_questions.iter().cloned().collect::<Vec<_>>());
    let adrs_summary = if adrs.is_empty() {
        "None yet.".to_string()
    } else {
        adrs.iter()
            .map(|a| format!("- {}: {}", a.title, a.decision))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"You are a business analyst checking a specification for completeness before
implementation begins. You are NOT designing architecture and NOT deciding how anything will be
implemented — only whether the specification itself is complete enough to proceed.

Story: As a {as_a}, I want {want}, so that {so_that}

Architecture decisions (context only — use these ONLY to judge whether an open question in
Checklist 3 is already resolved; do not evaluate the decisions themselves):
{adrs_summary}

Below are three checklists. Walk through EVERY item in EVERY checklist ONE AT A TIME, in order —
do not skip any, and do not reason about the specification as a whole. Answer yes or no for each
item individually before moving to the next. Only emit a gap for an item you answered "no" to.

## Checklist 1 — Constraint coverage
For EACH item, does at least one BDD scenario test that exact constraint being violated?
{constraint_list}

## Checklist 2 — Scenario outcome clarity
For EACH item, does the `then` state an observable, checkable outcome? ("the system handles it
correctly" is NOT observable; "an error message ... is shown" IS observable.)
{scenario_list}

## Checklist 3 — Open question resolution
For EACH item, is it resolved by an existing ADR or scenario above?
{open_questions}

Emit a `missing_scenario` gap for each "no" in Checklist 1, an `ambiguous_outcome` gap for each
"no" in Checklist 2, and an `unresolved_question` gap for each "no" in Checklist 3. Do not invent
gaps outside these three categories, and do not emit anything for an item you answered "yes" to.

Return ONLY valid YAML — no prose, no code fences:

gaps:
  - kind: missing_scenario | ambiguous_outcome | unresolved_question
    description: "<specific, concrete — name the exact field, scenario id, or question>"
    blocking: true | false
"#,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        adrs_summary = adrs_summary,
        constraint_list = constraint_list,
        scenario_list = scenario_list,
        open_questions = open_questions,
    )
}

pub fn identify_specification_gaps(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    adrs: &[Adr],
) -> Result<SpecificationCompleteness, LlmError> {
    let raw = client.complete_large(&specification_completeness_prompt(story, spec, adrs))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_yaml::from_str::<SpecificationCompleteness>(stripped)
        .map_err(|source| LlmError::YamlParse { source, raw: stripped.to_string() })
}
