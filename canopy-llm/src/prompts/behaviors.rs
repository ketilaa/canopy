//! Stage 0 of the behavior-first planning pipeline (docs/design/behavior-first-planning.md):
//! Specification Completeness. Validates a story's specification is complete enough to safely
//! decompose into behaviors, before any behavior extraction begins — no architecture vocabulary
//! enters at this stage, only whether the specification itself is internally complete.

use crate::client::{LlmClient, LlmError};
use crate::prompts::yaml_util::{parse_lenient_sequence, strip_code_fence};
use canopy_core::*;

/// Best-effort mechanical signal for whether some scenario already addresses a field + numeric
/// threshold: does any scenario's own text (its `then` clause plus its explicit `constraints`
/// field) mention both the field name and the threshold number? Deliberately crude — a textual
/// proxy, not proof — but it turns Checklist 1's question from "recall whether coverage exists"
/// (holistic recall, the exact shape of judgment live-verified to fail: the model reported gaps
/// against scenarios that explicitly named the constraint being asked about) into "verify this
/// specific candidate, or say why it's wrong" (a narrower, easier task per item) — the same
/// mechanical-baseline-plus-review shape already proven for Stage 3's clustering and Stage 4's
/// dependency inference.
fn mechanical_candidate<'a>(field_name: &str, threshold: &str, scenarios: &'a [Scenario]) -> Option<&'a str> {
    let field_lower = field_name.to_lowercase();
    scenarios.iter()
        .find(|s| {
            let haystack = format!("{} {}", s.then.join(" "), s.constraints.join(" ")).to_lowercase();
            haystack.contains(&field_lower) && haystack.contains(threshold)
        })
        .map(|s| s.id.as_str())
}

fn constraint_item(field_name: &str, label: &str, threshold: &str, scenarios: &[Scenario]) -> String {
    // No scenarios exist at all in this case (as opposed to "no matching candidate found among
    // existing scenarios") — prompt-review finding: referencing "scenario above" or "a DIFFERENT
    // scenario above" would dangle, since no scenario section is rendered when spec.scenarios is
    // empty (see `has_scenarios` in specification_completeness_prompt). Fall back to the plain
    // question with nothing to point at.
    if scenarios.is_empty() {
        return format!("Field '{field_name}': {label} — does at least one BDD scenario test this constraint being violated?");
    }
    match mechanical_candidate(field_name, threshold, scenarios) {
        Some(id) => format!(
            "Field '{field_name}': {label} — candidate: scenario {id} mentions '{field_name}' and \
             '{threshold}'. Does {id} genuinely test a value violating this constraint? If not, \
             does a DIFFERENT scenario above cover it?"
        ),
        None => format!(
            "Field '{field_name}': {label} — no scenario mentions both '{field_name}' and \
             '{threshold}'. Does any scenario above still cover this constraint under different \
             wording? If not, this is a genuine gap."
        ),
    }
}

/// Mechanically enumerates every (field, constraint) pair from the entity schema — one line per
/// constraint, in a fixed order. Live-verified: a single holistic "compare schema and scenarios"
/// prompt correctly found 4 of 5 real gaps against product-001's schema but silently missed one
/// field's max_length despite catching the identical constraint shape on three other fields —
/// not a conceptual failure, a coverage failure. Forcing an explicit per-constraint checklist
/// turns "notice everything that's missing" (holistic, omission-prone) into "answer yes/no for
/// each of these N already-enumerated items" (a much narrower task per item).
fn constraint_checklist(schema: &EntitySchema, scenarios: &[Scenario]) -> Vec<String> {
    let mut items = Vec::new();
    for field in schema.mandatory.iter().chain(schema.optional.iter()) {
        let Some(v) = &field.validation else { continue };
        if let Some(n) = v.max_length {
            items.push(constraint_item(&field.name, &format!("max_length={n}"), &n.to_string(), scenarios));
        }
        // min_length=0 is vacuous — nothing can be shorter than 0 characters, so there is no
        // possible violation to test. Omitting the item entirely removes the false-positive risk
        // at the source instead of asking a question about a constraint that cannot be violated.
        if let Some(n) = v.min_length {
            if n > 0 {
                items.push(constraint_item(&field.name, &format!("min_length={n}"), &n.to_string(), scenarios));
            }
        }
        if let Some(n) = v.min {
            items.push(constraint_item(&field.name, &format!("min={n}"), &n.to_string(), scenarios));
        }
        if let Some(n) = v.max {
            items.push(constraint_item(&field.name, &format!("max={n}"), &n.to_string(), scenarios));
        }
        if let Some(p) = &v.pattern {
            items.push(format!("Field '{}': pattern={p} — scenario testing a value that violates this pattern?", field.name));
        }
        if let Some(n) = v.max_items {
            items.push(constraint_item(&field.name, &format!("max_items={n}"), &n.to_string(), scenarios));
        }
    }
    items
}

/// One entry per scenario id, pointing back at `scenario_reference_listing` below rather than
/// restating its `then` clause — prompt-review finding: an earlier version repeated the full
/// `then` text here too, duplicating the same scenario content twice within one prompt call
/// (once bare in the reference listing, once again moments later in this checklist), which is
/// exactly the token/attention dilution the proximity and duplicate-injection house rules exist
/// to prevent. The enumeration itself (walk every id, one at a time) is what this checklist
/// needs — not a second copy of the text already visible just above.
fn scenario_checklist(scenarios: &[Scenario]) -> Vec<String> {
    scenarios.iter()
        .map(|s| format!("{} (see scenario listing above)", s.id))
        .collect()
}

/// Single canonical scenario listing, shown once before both Checklist 1 and Checklist 2 — each
/// scenario's `then` outcome and its own explicit `constraints` field. Live-verified bug this
/// fixes: Checklist 1 asks "does at least one scenario test this exact constraint," but no
/// scenario text existed anywhere earlier in the prompt for it to cross-reference — the model had
/// nothing to check against at the point it needed to answer, and flagged every constraint as
/// missing even when a scenario explicitly stating that exact constraint already existed.
fn scenario_reference_listing(scenarios: &[Scenario]) -> String {
    scenarios.iter().enumerate().map(|(i, s)| {
        let constraints = if s.constraints.is_empty() {
            String::new()
        } else {
            format!(" | constraints: {}", s.constraints.join("; "))
        };
        format!("{}. {}: then: {}{}", i + 1, s.id, s.then.join("; "), constraints)
    }).collect::<Vec<_>>().join("\n")
}

fn numbered(items: &[String]) -> String {
    items.iter().enumerate().map(|(i, item)| format!("{}. {item}", i + 1)).collect::<Vec<_>>().join("\n")
}

/// One entry per `out_of_scope` item, each pointing back at the scenario listing already shown
/// once above (see `scenario_reference_listing`) rather than restating scenario content here —
/// same proximity/no-duplicate-injection reasoning as `scenario_checklist`. One item per
/// (excluded concern, scenario) PAIR — nested enumeration, not one holistic "scan every scenario"
/// question per excluded concern — the same discipline `exhaustive-enumeration-over-holistic-
/// review` already established for Checklists 1-3 (prompt-review finding: an earlier version
/// asked one question per excluded concern that scanned the whole scenario list at once, exactly
/// the omission-prone shape `mechanical_candidate`'s own doc comment above already documents as
/// live-verified to fail). Items stay bare (name the pair, no restated question) so the actual
/// yes/no question lives only in the shared instruction — matching `scenario_checklist`/
/// `open_question_items`'s shape, not `constraint_item`'s (which restates a *different* question
/// per item because each one names a distinct mechanically-found candidate; there's no equivalent
/// per-pair candidate here). Naturally empty when there are no scenarios or no excluded concerns
/// — nothing to pair, not a special-cased guard (see docs/design/product-010-story-readiness-
/// failure-diagnosis.md for the confirmed failure case this checklist targets: an excluded
/// concern contradicted by an accepted scenario, undetected because no check compared the two).
fn scope_contradiction_checklist(out_of_scope: &[String], scenarios: &[Scenario]) -> Vec<String> {
    let mut items = Vec::new();
    for concern in out_of_scope {
        for scenario in scenarios {
            items.push(format!("Excluded concern \"{concern}\" vs. scenario {}", scenario.id));
        }
    }
    items
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
        .map(|schema| constraint_checklist(schema, &spec.scenarios))
        .unwrap_or_default();
    let scenario_items = scenario_checklist(&spec.scenarios);
    let open_question_items: Vec<String> = spec.open_questions.to_vec();

    // Shown BEFORE both checklists, not after — see `scenario_reference_listing`'s own doc
    // comment for the live-verified bug this ordering fixes. `has_scenarios` also gates
    // Checklist 1's instruction wording below: it must not say "listed above" when nothing was
    // actually listed (prompt-review finding — a spec can have an entity_schema, and therefore
    // constraint_items, while scenarios is momentarily empty).
    let has_scenarios = !spec.scenarios.is_empty();
    let scenario_reference = if has_scenarios {
        format!(
            "Scenarios (reference for both checklists below):\n{}\n\n",
            scenario_reference_listing(&spec.scenarios),
        )
    } else {
        String::new()
    };

    // Each item already states its own mechanically pre-computed candidate (or lack of one) and
    // asks a bounded verify-or-name-an-alternative question — this instruction just frames what
    // "yes"/"no" mean for that per-item question, it doesn't ask the model to judge coverage from
    // an unaided read of the scenario list the way the item text alone used to.
    let checklist1_instruction = if has_scenarios {
        "For EACH item, answer its own question about the given candidate (or lack of one). \
         Answer \"yes\" if the constraint is genuinely covered by some scenario above, \"no\" if not."
    } else {
        "For EACH item, does at least one BDD scenario test that exact constraint being violated?"
    };
    let checklist1 = checklist_section(
        "## Checklist 1 — Constraint coverage",
        checklist1_instruction,
        &constraint_items,
    );
    let checklist2 = checklist_section(
        "## Checklist 2 — Scenario outcome clarity",
        "For EACH item, does that scenario's `then` clause (in the listing above) state an \
         observable, checkable outcome? (\"the system handles it correctly\" is NOT observable; \
         \"an error message ... is shown\" IS observable.)",
        &scenario_items,
    );
    let checklist3 = checklist_section(
        "## Checklist 3 — Open question resolution",
        "For EACH item, is it resolved by an existing ADR or scenario above?",
        &open_question_items,
    );
    let scope_items = scope_contradiction_checklist(&spec.out_of_scope, &spec.scenarios);
    let checklist4 = checklist_section(
        "## Checklist 4 — Scope contradiction",
        "For EACH item, does the referenced scenario (in the listing above) avoid presupposing \
         or requiring the referenced excluded concern? Answer \"yes\" if it stays clear of it, \
         \"no\" if it presupposes or requires it — the story would then be excluding and \
         depending on the same concern.",
        &scope_items,
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

{scenario_reference}{checklist1}{checklist2}{checklist3}{checklist4}Emit a `missing_scenario` gap for each "no" in Checklist 1, an `ambiguous_outcome` gap for each
"no" in Checklist 2, an `unresolved_question` gap for each "no" in Checklist 3, and a
`scope_contradiction` gap for each "no" in Checklist 4. Do not invent gaps outside these four
categories, and do not emit anything for an item you answered "yes" to.

Return ONLY valid YAML — no prose, no code fences:

gaps:
  - kind: missing_scenario | ambiguous_outcome | unresolved_question | scope_contradiction
    description: "<specific, concrete — name the exact field, scenario id, question, or excluded item>"
"#,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        adrs_summary = adrs_summary,
        scenario_reference = scenario_reference,
        checklist1 = checklist1,
        checklist2 = checklist2,
        checklist3 = checklist3,
        checklist4 = checklist4,
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
                    entity: Some(schema.entity.clone()), member: Some(field.name.clone()),
                    mandatory: Some(is_mandatory),
                });
            }
            if let Some(n) = v.min_length {
                // min_length=0 is vacuous — nothing can be shorter than 0 characters, so there is
                // no violation for this behavior to describe. Live-verified: emitting one anyway
                // ("Website shorter than 0 characters is rejected") produced a nonsensical,
                // untestable requirement, correctly flagged by Stage 3's own cluster review as
                // not making sense. `saw_min_length` must only be set when a real behavior was
                // actually pushed — prompt-review caught a real bug here: setting it unconditionally
                // suppressed the mandatory-field "Missing X is rejected" fallback below even when
                // n==0 skipped pushing anything, silently losing the field's only required-ness
                // behavior entirely for a mandatory field that (incorrectly) carries min_length: 0.
                if n > 0 {
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
                        entity: Some(schema.entity.clone()), member: Some(field.name.clone()),
                        mandatory: Some(is_mandatory),
                    });
                }
            }
            if let Some(n) = v.min {
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.min", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement: format!("{} below {n} is rejected.", capitalize(&field.name)),
                    derivation: BehaviorDerivation::Mechanical,
                    entity: Some(schema.entity.clone()), member: Some(field.name.clone()),
                    mandatory: Some(is_mandatory),
                });
            }
            if let Some(n) = v.max {
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.max", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement: format!("{} above {n} is rejected.", capitalize(&field.name)),
                    derivation: BehaviorDerivation::Mechanical,
                    entity: Some(schema.entity.clone()), member: Some(field.name.clone()),
                    mandatory: Some(is_mandatory),
                });
            }
            if v.pattern.is_some() {
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.pattern", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement: format!("{} violating the required pattern is rejected.", capitalize(&field.name)),
                    derivation: BehaviorDerivation::Mechanical,
                    entity: Some(schema.entity.clone()), member: Some(field.name.clone()),
                    mandatory: Some(is_mandatory),
                });
            }
            if let Some(n) = v.max_items {
                out.push(Behavior {
                    id: next_id(), source: BehaviorSource::EntitySchema,
                    source_ref: format!("{}.{}.max_items", schema.entity, field.name),
                    scope: BehaviorScope::Unit, subject: subject.clone(), kind: BehaviorKind::Validation,
                    statement: format!("More than {n} {} is rejected.", field.name),
                    derivation: BehaviorDerivation::Mechanical,
                    entity: Some(schema.entity.clone()), member: Some(field.name.clone()),
                    mandatory: Some(is_mandatory),
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
                entity: Some(schema.entity.clone()), member: Some(field.name.clone()),
                mandatory: Some(is_mandatory),
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
        entity: Some(schema.entity.clone()),
        // Not `Some(field.name)` — every system-generated field's construction behavior shares
        // one cluster/contract for the whole entity (grouped by subject=entity, kind=Construction),
        // so no single field represents the contract; see the `member` field's own doc comment.
        member: None,
        // Mandatory/optional isn't a meaningful concept for a whole-entity construction behavior.
        mandatory: None,
    }).collect()
}

/// Parses the fixed "<EventName> on topic <topic-name>" shape `spec.rs`'s own ADR-proposal
/// prompt enforces for domain-event ADRs (see canopy-llm/src/prompts/spec.rs) — not a guess,
/// a documented, generated convention.
pub fn parse_event_adr(decision: &str) -> Option<(String, String)> {
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
                // `entity` is the function's own parameter, not parsed back out of `event_name` —
                // the naming convention (event name prefixed with entity name) already made this
                // safe to parse, but there's no reason to when the true value is already in scope.
                entity: Some(entity.to_string()), member: None, mandatory: None,
            });
        }
        out.push(Behavior {
            id: next_id(), source: BehaviorSource::Adr, source_ref: adr.title.clone(),
            scope: BehaviorScope::Unit, subject: event_name.clone(), kind: BehaviorKind::EventShape,
            statement: format!("{event_name} contains {aggregate_ref}."),
            derivation: BehaviorDerivation::Mechanical,
            entity: Some(entity.to_string()), member: None, mandatory: None,
        });
        out.push(Behavior {
            id: next_id(), source: BehaviorSource::Adr, source_ref: adr.title.clone(),
            scope: BehaviorScope::Unit, subject: "EventPublisher".to_string(), kind: BehaviorKind::Publication,
            statement: format!("{event_name} is published on {topic}."),
            derivation: BehaviorDerivation::Mechanical,
            entity: Some(entity.to_string()), member: None, mandatory: None,
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
            // The model is only asked for a single `subject` string (see `scenario_behavior_prompt`
            // above), not an entity/field split — leaving these `None` rather than parsing `subject`
            // avoids reintroducing the compound-name ambiguity this field exists to avoid.
            entity: None, member: None, mandatory: None,
        });
    }

    let blocked = raw_blocked.into_iter().map(|b| BlockedBehaviorCandidate {
        source: BehaviorSource::Scenario,
        source_ref: b.source_ref,
        reason: b.reason,
    }).collect();

    Ok((BehaviorList { behaviors }, BehaviorGaps { blocked }))
}

/// Second invariant `audit_behavior_coverage` checks (see below): every ADR that names a domain
/// event for this story's own entity must have produced at least one `EventShape` behavior —
/// independent of whether the ADR's decision text follows the "<EventName> on topic <topic>"
/// shape `mechanical_event_behaviors` requires to also derive a `Publication` behavior. Matching
/// is deliberately looser than `mechanical_event_behaviors`'s own parse: it only needs the
/// pre-topic event name (same `" on topic "` split), not a successfully-parsed topic, so this can
/// flag the exact case where the stricter mechanical derivation silently produced nothing —
/// e.g. an ADR whose decision text predates a project's Topic Naming Convention ADR, or one
/// hand-edited after the fact. Contains-no-"-"/"-no-space" mirrors `spec.rs`'s own
/// `find_existing_domain_event_for_story` matching, to avoid a false positive against an
/// unrelated kebab-case decision (e.g. "widget-service") that happens to start with the entity
/// name but isn't an event at all.
fn adr_event_coverage_findings(spec: &IntentSpec, adrs: &[Adr], behaviors: &BehaviorList) -> Vec<BehaviorAuditFinding> {
    let Some(schema) = &spec.entity_schema else { return Vec::new() };
    let entity_lower = schema.entity.to_lowercase();
    let mut findings = Vec::new();
    for adr in adrs {
        if !adr.title.to_lowercase().contains("event") { continue; }
        let event_name = adr.decision.split(" on topic ").next().unwrap_or(&adr.decision).trim();
        if event_name.is_empty() || event_name.contains('-') || event_name.contains(' ') { continue; }
        if !event_name.to_lowercase().starts_with(&entity_lower) { continue; }
        let has_behavior = behaviors.behaviors.iter()
            .any(|b| b.kind == BehaviorKind::EventShape && b.subject == event_name);
        if !has_behavior {
            findings.push(BehaviorAuditFinding {
                description: format!(
                    "ADR '{}' names domain event '{event_name}' for this story's entity, but no EventShape behavior was produced for it — check whether its decision text follows the \"<EventName> on topic <topic>\" convention `mechanical_event_behaviors` requires.",
                    adr.title
                ),
            });
        }
    }
    findings
}

/// Stage 1's own mechanical audit — same shape as Stage 0/2's. Live-verified need: a scenario
/// whose LLM-generated behaviors all failed per-item YAML validation (a `scope` field mistakenly
/// set to the behavior's own `kind` value) produced zero surviving behaviors and wasn't recorded
/// as blocked either — silently invisible without this check. Checks two invariants: every
/// scenario in `spec.scenarios` must have produced at least one surviving behavior OR appear in
/// `gaps.blocked` (anything with neither is a real coverage loss, not a legitimate outcome), and
/// every domain-event ADR for this story's entity must have produced at least one `EventShape`
/// behavior (see `adr_event_coverage_findings` above).
pub fn audit_behavior_coverage(spec: &IntentSpec, adrs: &[Adr], behaviors: &BehaviorList, gaps: &BehaviorGaps) -> BehaviorAudit {
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
    findings.extend(adr_event_coverage_findings(spec, adrs, behaviors));
    BehaviorAudit { findings }
}

#[cfg(test)]
mod adr_event_coverage_tests {
    use super::*;

    fn spec_for(entity: &str) -> IntentSpec {
        IntentSpec {
            intent_ref: "story-001".to_string(),
            entity_schema: Some(EntitySchema {
                entity: entity.to_string(),
                system_generated: vec![],
                mandatory: vec![],
                optional: vec![],
            }),
            scenarios: vec![],
            resolved_policies: vec![],
            out_of_scope: vec![],
            open_questions: vec![],
        }
    }

    fn adr(title: &str, decision: &str) -> Adr {
        Adr {
            title: title.to_string(),
            decision: decision.to_string(),
            reason: String::new(),
            alternatives: vec![],
        }
    }

    fn event_shape_behavior(subject: &str) -> Behavior {
        Behavior {
            id: "story-001-b099".to_string(),
            source: BehaviorSource::Adr,
            source_ref: "some-adr".to_string(),
            scope: BehaviorScope::Unit,
            subject: subject.to_string(),
            kind: BehaviorKind::EventShape,
            statement: format!("{subject} contains eventId."),
            derivation: BehaviorDerivation::Mechanical,
            entity: None, member: None, mandatory: None,
        }
    }

    #[test]
    fn flags_a_domain_event_adr_with_no_topic_and_no_produced_behavior() {
        let spec = spec_for("Manufacturer");
        let adrs = vec![adr("Domain Event for Manufacturer Registration", "ManufacturerRegistered")];
        let behaviors = BehaviorList { behaviors: vec![] };
        let findings = adr_event_coverage_findings(&spec, &adrs, &behaviors);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].description.contains("ManufacturerRegistered"));
    }

    #[test]
    fn does_not_flag_when_a_matching_event_shape_behavior_exists() {
        let spec = spec_for("Manufacturer");
        let adrs = vec![adr("Domain Event for Manufacturer Registration", "ManufacturerRegistered on topic manufacturer-events")];
        let behaviors = BehaviorList { behaviors: vec![event_shape_behavior("ManufacturerRegistered")] };
        let findings = adr_event_coverage_findings(&spec, &adrs, &behaviors);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_a_non_event_adr_even_if_it_starts_with_the_entity_name() {
        let spec = spec_for("Manufacturer");
        let adrs = vec![adr("Service Ownership for Manufacturer Registration", "manufacturer-service")];
        let behaviors = BehaviorList { behaviors: vec![] };
        let findings = adr_event_coverage_findings(&spec, &adrs, &behaviors);
        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_an_event_adr_for_a_different_entity() {
        let spec = spec_for("Manufacturer");
        let adrs = vec![adr("Domain Event for Product Registration", "ProductCreated on topic product-events")];
        let behaviors = BehaviorList { behaviors: vec![] };
        let findings = adr_event_coverage_findings(&spec, &adrs, &behaviors);
        assert!(findings.is_empty());
    }
}

#[cfg(test)]
mod scope_contradiction_checklist_tests {
    use super::*;

    fn scenario(id: &str) -> Scenario {
        Scenario {
            id: id.to_string(),
            name: String::new(),
            given: vec![],
            when: String::new(),
            then: vec![],
            constraints: vec![],
        }
    }

    #[test]
    fn one_item_per_concern_scenario_pair() {
        let out_of_scope = vec!["Customer authentication and authorization".to_string()];
        let scenarios = vec![scenario("story-001-01"), scenario("story-001-02")];
        let items = scope_contradiction_checklist(&out_of_scope, &scenarios);
        assert_eq!(items.len(), 2);
        assert!(items[0].contains("Customer authentication and authorization"));
        assert!(items[0].contains("story-001-01"));
        assert!(items[1].contains("story-001-02"));
    }

    #[test]
    fn nests_fully_across_multiple_concerns_and_scenarios() {
        let out_of_scope = vec!["A".to_string(), "B".to_string()];
        let scenarios = vec![scenario("s1"), scenario("s2"), scenario("s3")];
        let items = scope_contradiction_checklist(&out_of_scope, &scenarios);
        assert_eq!(items.len(), 6);
    }

    #[test]
    fn no_items_when_out_of_scope_is_empty() {
        let items = scope_contradiction_checklist(&[], &[scenario("s1")]);
        assert!(items.is_empty());
    }

    #[test]
    fn no_items_when_there_are_no_scenarios_to_compare_against() {
        let out_of_scope = vec!["Customer authentication and authorization".to_string()];
        let items = scope_contradiction_checklist(&out_of_scope, &[]);
        assert!(items.is_empty());
    }
}
