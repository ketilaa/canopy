//! Stage 0 of the behavior-first planning pipeline (docs/design/behavior-first-planning.md):
//! Specification Completeness. Validates a story's specification is complete enough to safely
//! decompose into behaviors, before any behavior extraction begins — no architecture vocabulary
//! enters at this stage, only whether the specification itself is internally complete.

use crate::client::{LlmClient, LlmError};
use crate::prompts::yaml_util::{parse_lenient_sequence, strip_code_fence};
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
    items.iter().enumerate().map(|(i, item)| format!("{}. {item}", i + 1)).collect::<Vec<_>>().join("\n")
}

/// Renders a checklist section, or nothing at all when there are no items. Live-verified bug
/// this fixes: rendering an empty checklist as a "None." placeholder still sometimes produced a
/// spurious gap referencing "None." as if it were a real item — the model didn't reliably treat
/// the placeholder as "nothing to check." Omitting the section entirely removes the ambiguity
/// instead of trusting the model to interpret a placeholder correctly.
fn checklist_section(heading: &str, instruction: &str, items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }
    format!("{heading}\n{instruction}\n{}\n\n", numbered(items))
}

fn specification_completeness_prompt(story: &UserStory, spec: &IntentSpec, adrs: &[Adr]) -> String {
    let constraint_items = spec.entity_schema.as_ref()
        .map(constraint_checklist)
        .unwrap_or_default();
    let scenario_items = scenario_checklist(&spec.scenarios);
    let open_question_items: Vec<String> = spec.open_questions.to_vec();

    let checklist1 = checklist_section(
        "## Checklist 1 — Constraint coverage",
        "For EACH item, does at least one BDD scenario test that exact constraint being violated?",
        &constraint_items,
    );
    let checklist2 = checklist_section(
        "## Checklist 2 — Scenario outcome clarity",
        "For EACH item, does the `then` state an observable, checkable outcome? (\"the system \
         handles it correctly\" is NOT observable; \"an error message ... is shown\" IS observable.)",
        &scenario_items,
    );
    let checklist3 = checklist_section(
        "## Checklist 3 — Open question resolution",
        "For EACH item, is it resolved by an existing ADR or scenario above?",
        &open_question_items,
    );

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

Below are checklists, each covering a different concern. Walk through EVERY item in EVERY
checklist shown ONE AT A TIME, in order — do not skip any, and do not reason about the
specification as a whole. Answer yes or no for each item individually before moving to the
next. Only emit a gap for an item you answered "no" to. A checklist that isn't shown below has
no items to check — do not emit any gap for it.

{checklist1}{checklist2}{checklist3}Emit a `missing_scenario` gap for each "no" in Checklist 1, an `ambiguous_outcome` gap for each
"no" in Checklist 2, and an `unresolved_question` gap for each "no" in Checklist 3. Do not invent
gaps outside these three categories, and do not emit anything for an item you answered "yes" to.

Return ONLY valid YAML — no prose, no code fences:

gaps:
  - kind: missing_scenario | ambiguous_outcome | unresolved_question
    description: "<specific, concrete — name the exact field, scenario id, or question>"
"#,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        adrs_summary = adrs_summary,
        checklist1 = checklist1,
        checklist2 = checklist2,
        checklist3 = checklist3,
    )
}

pub fn identify_specification_gaps(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    adrs: &[Adr],
) -> Result<SpecificationCompleteness, LlmError> {
    let raw = client.complete_large(&specification_completeness_prompt(story, spec, adrs))?;
    let stripped = strip_code_fence(&raw);
    let gaps = parse_lenient_sequence::<CompletenessGap>(&stripped, "gaps")?;
    Ok(SpecificationCompleteness { gaps })
}

// ── Stage 1: Behavior Extraction ────────────────────────────────────────────────────────────
//
// Same lesson as Stage 0, applied one level further: anything mechanically derivable from
// already-structured data (entity-schema constraints, system-generated fields, domain-event
// ADRs) is computed directly in Rust, not asked of the model. Only behaviors that require
// genuine narrative interpretation — what a scenario's flow actually does — go through an LLM
// call, and even that call is checklist-driven per scenario rather than a holistic read of the
// whole specification, for the same reason Stage 0's checklist rewrite went from 4/9 to 9/9.

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn subject_for_field(entity: &str, field: &str) -> String {
    format!("{entity}{}", capitalize(field))
}

/// One behavior per (field, constraint) pair — the exact same enumeration Stage 0's
/// `constraint_checklist` walks, just emitting a statement instead of a coverage question.
/// A mandatory field with no `min_length` still gets an explicit "must be provided" behavior —
/// required-ness is a constraint even when the schema doesn't spell out a length bound for it.
/// True for a schema field whose `type` names a collection (`[string]`, `array`, ...) — `field`
/// doesn't otherwise carry this signal anywhere accessible to statement wording, so `max_length`
/// on such a field means "each element's length," not "the collection's own length," and the
/// generated statement must say so explicitly rather than reading as if it measured the
/// collection (live-verified ambiguity: "Categories longer than 100 characters is rejected."
/// read as if the whole list, not each category string, were being measured).
fn is_collection_field(field: &FieldDef) -> bool {
    field.field_type.trim_start().starts_with('[') || field.field_type.eq_ignore_ascii_case("array")
}

fn mechanical_validation_behaviors(schema: &EntitySchema, next_id: &mut impl FnMut() -> String) -> Vec<Behavior> {
    let mut out = Vec::new();
    let mandatory = schema.mandatory.iter().map(|f| (f, true));
    let optional = schema.optional.iter().map(|f| (f, false));
    for (field, is_mandatory) in mandatory.chain(optional) {
        let subject = subject_for_field(&schema.entity, &field.name);
        let is_collection = is_collection_field(field);
        let mut saw_min_length = false;
        if let Some(v) = &field.validation {
            if let Some(n) = v.max_length {
                let statement = if is_collection {
                    format!("Each item in {} longer than {n} characters is rejected.", field.name)
                } else {
                    format!("{} longer than {n} characters is rejected.", capitalize(&field.name))
                };
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.max_length", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement, derivation: BehaviorDerivation::Mechanical,
                });
            }
            if let Some(n) = v.min_length {
                saw_min_length = true;
                let statement = if is_collection {
                    format!("Each item in {} shorter than {n} characters is rejected.", field.name)
                } else {
                    format!("{} shorter than {n} characters is rejected.", capitalize(&field.name))
                };
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.min_length", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement, derivation: BehaviorDerivation::Mechanical,
                });
            }
            if let Some(n) = v.min {
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.min", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement: format!("{} below {n} is rejected.", capitalize(&field.name)),
                    derivation: BehaviorDerivation::Mechanical,
                });
            }
            if let Some(n) = v.max {
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.max", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement: format!("{} above {n} is rejected.", capitalize(&field.name)),
                    derivation: BehaviorDerivation::Mechanical,
                });
            }
            if v.pattern.is_some() {
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.pattern", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement: format!("{} violating the required pattern is rejected.", capitalize(&field.name)),
                    derivation: BehaviorDerivation::Mechanical,
                });
            }
            if let Some(n) = v.max_items {
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.max_items", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement: format!("More than {n} {} is rejected.", field.name),
                    derivation: BehaviorDerivation::Mechanical,
                });
            }
        }
        if is_mandatory && !saw_min_length {
            out.push(Behavior {
                id: next_id(), source: BehaviorSource::EntitySchema,
                source_ref: format!("{}.{}.required", schema.entity, field.name),
                scope: BehaviorScope::Unit, subject, kind: BehaviorKind::Validation,
                statement: format!("Missing {} is rejected.", field.name),
                derivation: BehaviorDerivation::Mechanical,
            });
        }
    }
    out
}

/// One behavior per system-generated field — the aggregate's own factory assigns it.
fn mechanical_construction_behaviors(schema: &EntitySchema, next_id: &mut impl FnMut() -> String) -> Vec<Behavior> {
    schema.system_generated.iter().map(|field| Behavior {
        id: next_id(),
        source: BehaviorSource::EntitySchema,
        source_ref: format!("{}.{}.system_generated", schema.entity, field.name),
        scope: BehaviorScope::Unit,
        subject: schema.entity.clone(),
        kind: BehaviorKind::Construction,
        statement: format!("{} construction assigns {}.", schema.entity, field.name),
        derivation: BehaviorDerivation::Mechanical,
    }).collect()
}

/// Parses the fixed "<EventName> on topic <topic-name>" shape `spec.rs`'s own ADR-proposal
/// prompt enforces for domain-event ADRs (see canopy-llm/src/prompts/spec.rs) — not a guess,
/// a documented, generated convention.
fn parse_event_adr(decision: &str) -> Option<(String, String)> {
    let mut parts = decision.splitn(2, " on topic ");
    let event_name = parts.next()?.trim();
    let topic = parts.next()?.trim();
    if event_name.is_empty() || topic.is_empty() { return None; }
    Some((event_name.to_string(), topic.to_string()))
}

/// Every domain event ADR implies the same fixed payload shape, per the event-orientation
/// architecture skill's own rule (`canopy-llm/src/skills/architecture.rs`): eventId (own
/// identity) + occurredAt + <entity>Id (aggregate reference), published on its topic. Computed
/// directly from the ADR's decision text — no LLM needed to restate a convention this fixed.
/// Only ADRs for events belonging to THIS story's own entity are included (event name prefixed
/// with the entity name is itself an enforced naming convention — see `spec.rs`).
fn mechanical_event_behaviors(entity: &str, adrs: &[Adr], next_id: &mut impl FnMut() -> String) -> Vec<Behavior> {
    let mut out = Vec::new();
    for adr in adrs {
        let Some((event_name, topic)) = parse_event_adr(&adr.decision) else { continue };
        if !event_name.starts_with(entity) { continue; }
        let aggregate_ref = format!("{entity}Id");
        for field in ["eventId", "occurredAt"] {
            out.push(Behavior {
                id: next_id(), source: BehaviorSource::Adr, source_ref: adr.title.clone(),
                scope: BehaviorScope::Unit, subject: event_name.clone(), kind: BehaviorKind::EventShape,
                statement: format!("{event_name} contains {field}."),
                derivation: BehaviorDerivation::Mechanical,
            });
        }
        out.push(Behavior {
            id: next_id(), source: BehaviorSource::Adr, source_ref: adr.title.clone(),
            scope: BehaviorScope::Unit, subject: event_name.clone(), kind: BehaviorKind::EventShape,
            statement: format!("{event_name} contains {aggregate_ref}."),
            derivation: BehaviorDerivation::Mechanical,
        });
        out.push(Behavior {
            id: next_id(), source: BehaviorSource::Adr, source_ref: adr.title.clone(),
            scope: BehaviorScope::Unit, subject: "EventPublisher".to_string(), kind: BehaviorKind::Publication,
            statement: format!("{event_name} is published on {topic}."),
            derivation: BehaviorDerivation::Mechanical,
        });
    }
    out
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawBehavior {
    source_ref: String,
    scope: BehaviorScope,
    subject: String,
    kind: BehaviorKind,
    statement: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawBlocked {
    source_ref: String,
    reason: String,
}

fn scenario_behavior_prompt(story: &UserStory, spec: &IntentSpec, openapi_yaml: &str) -> String {
    let scenario_list = numbered(&scenario_checklist(&spec.scenarios));
    let open_questions = if spec.open_questions.is_empty() {
        "None recorded — nothing blocks any scenario below on this account.".to_string()
    } else {
        numbered(&spec.open_questions)
    };
    let openapi_section = if openapi_yaml.trim().is_empty() {
        "None yet.".to_string()
    } else {
        openapi_yaml.to_string()
    };

    format!(
        r#"You are extracting atomic, independently-testable behaviors from BDD scenarios. You
are NOT designing architecture and NOT deciding file structure — only what must be true,
observably.

Story: As a {as_a}, I want {want}, so that {so_that}

Already covered separately — do NOT extract a behavior for any of these, even if a scenario's
`then` clause mentions them:
- Field validation constraints (max_length, min_length, required, etc.) — derived directly from
  the entity schema.
- Aggregate construction of system-generated fields (id, createdAt, modifiedAt, ...) — derived
  directly from the entity schema.
- Domain event payload shape (eventId, occurredAt, aggregate reference) and topic publication —
  derived directly from architecture decisions.

Open questions not yet resolved (context for the blocking rule below):
{open_questions}

OpenAPI spec (context for http-request/http-response behaviors):
{openapi_section}

For EACH scenario below, ONE AT A TIME:

Step 1 — classify it as SUCCESS (the story's action completes as intended) or FAILURE (the
action is rejected).

Step 2 — walk the checklist for that classification. Answer yes or no for EVERY item before
moving to the next scenario. Extract a behavior for every "yes" — including ones the scenario's
`then` clause never states in so many words, if the classification implies it.

If SUCCESS:
  - Is something persisted? (persistence)
  - Is an event published? (orchestration)
  - What HTTP status/response is returned? (http-request / http-response)

If FAILURE:
  - Is an error response returned, and with what status/message? (error-translation)
  - Is persistence PREVENTED — does nothing get saved? A failure scenario implies this even when
    it isn't stated out loud; extract it explicitly. (persistence)
  - Is event publication PREVENTED — does nothing get published? Same rule: implied, not stated,
    still extract it. (orchestration)

{scenario_list}

Rules:
- If a behavior's exact shape depends on an open question above, do NOT invent an
  interpretation — add an entry to `blocked` instead of `behaviors`, referencing the scenario
  and the specific question.
- kind must be exactly one of: persistence | orchestration | http-request | http-response |
  error-translation. Do NOT use validation, construction, event-shape, or publication — those
  are already covered.
- A behavior describing the OVERALL outcome of a scenario's flow (e.g. "X is persisted", "Y is
  published", "X is NOT persisted") is almost always scope=integration — it spans multiple
  components, not one. Use a subject naming the operation itself (e.g. "WidgetRegistration"),
  not a specific file or class. Only use scope=unit if the behavior is genuinely verifiable by
  testing one component alone.
- `scope` is ALWAYS exactly `unit` or `integration` — never the same value as `kind`. WRONG:
  `scope: http-response, kind: http-response`. CORRECT: `scope: integration, kind: http-response`.
- Duplicate or near-duplicate behaviors across similar scenarios are fine — do not merge or
  skip them. Each preserves traceability to its own scenario; consolidation happens later, in
  clustering, not here.

Return ONLY valid YAML — no prose, no code fences:

behaviors:
  - source_ref: "<scenario id>"
    scope: unit | integration
    subject: "<subject>"
    kind: persistence | orchestration | http-request | http-response | error-translation
    statement: "<atomic, observable behavior statement>"

blocked:
  - source_ref: "<scenario id>"
    reason: "<which open question blocks this, and why>"
"#,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        open_questions = open_questions,
        openapi_section = openapi_section,
        scenario_list = scenario_list,
    )
}

/// Stage 1 entry point. Combines mechanical behaviors (entity schema, domain-event ADRs) with
/// LLM-extracted, checklist-driven scenario behaviors into one traceable list, plus a gaps
/// artifact for anything blocked on an unresolved open question.
pub fn extract_behaviors(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    adrs: &[Adr],
    openapi_yaml: &str,
) -> Result<(BehaviorList, BehaviorGaps), LlmError> {
    let mut counter = 0usize;
    let mut next_id = move || { counter += 1; format!("{}-b{:03}", story.id, counter) };

    let mut behaviors = Vec::new();
    if let Some(schema) = &spec.entity_schema {
        behaviors.extend(mechanical_validation_behaviors(schema, &mut next_id));
        behaviors.extend(mechanical_construction_behaviors(schema, &mut next_id));
        behaviors.extend(mechanical_event_behaviors(&schema.entity, adrs, &mut next_id));
    }

    let raw = client.complete_large(&scenario_behavior_prompt(story, spec, openapi_yaml))?;
    let stripped = strip_code_fence(&raw);
    let raw_behaviors = parse_lenient_sequence::<RawBehavior>(&stripped, "behaviors")?;
    let raw_blocked = parse_lenient_sequence::<RawBlocked>(&stripped, "blocked")?;

    for rb in raw_behaviors {
        behaviors.push(Behavior {
            id: next_id(),
            source: BehaviorSource::Scenario,
            source_ref: rb.source_ref,
            scope: rb.scope,
            subject: rb.subject,
            kind: rb.kind,
            statement: rb.statement,
            derivation: BehaviorDerivation::Inferred,
        });
    }

    let blocked = raw_blocked.into_iter().map(|b| BlockedBehaviorCandidate {
        source: BehaviorSource::Scenario,
        source_ref: b.source_ref,
        reason: b.reason,
    }).collect();

    Ok((BehaviorList { behaviors }, BehaviorGaps { blocked }))
}

/// Stage 1's own mechanical audit — same shape as Stage 0/2's. Live-verified need: a scenario
/// whose LLM-generated behaviors all failed per-item YAML validation (a `scope` field mistakenly
/// set to the behavior's own `kind` value) produced zero surviving behaviors and wasn't recorded
/// as blocked either — silently invisible without this check. The invariant: every scenario in
/// `spec.scenarios` must have produced at least one surviving behavior OR appear in
/// `gaps.blocked`; anything with neither is a real coverage loss, not a legitimate outcome.
pub fn audit_behavior_coverage(spec: &IntentSpec, behaviors: &BehaviorList, gaps: &BehaviorGaps) -> BehaviorAudit {
    let mut findings = Vec::new();
    for scenario in &spec.scenarios {
        let has_behavior = behaviors.behaviors.iter().any(|b| b.source_ref == scenario.id);
        let is_blocked = gaps.blocked.iter().any(|b| b.source_ref == scenario.id);
        if !has_behavior && !is_blocked {
            findings.push(BehaviorAuditFinding {
                description: format!(
                    "Scenario '{}' produced no surviving behaviors and isn't recorded as blocked either — likely lost during generation or parsing, not a legitimate empty outcome.",
                    scenario.id
                ),
            });
        }
    }
    BehaviorAudit { findings }
}
