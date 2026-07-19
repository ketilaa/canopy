use crate::client::{LlmClient, LlmError};
use crate::prompts::yaml_util::{parse_lenient_sequence, strip_code_fence};
use crate::repair::fix_yaml_colon_in_scalars;
use canopy_core::*;

#[derive(PartialEq, Eq, Debug)]
enum EventOperation {
    Creation,
    Update,
    Deletion,
    Other,
}

/// Classifies a story's `want` by whole-word match against fixed verb inflection lists — never a
/// substring/prefix check, which a prior version of this function used and which misfired on
/// ordinary words that happen to contain a short verb root (e.g. "add" inside "address", "edit"
/// inside "credit", "chang" inside "exchange" — each would have misclassified an unrelated story).
fn classify_operation_from_want(words: &[String]) -> EventOperation {
    const CREATION: &[&str] = &[
        "register", "registers", "registered", "registering",
        "create", "creates", "created", "creating",
        "add", "adds", "added", "adding",
        "onboard", "onboards", "onboarded", "onboarding",
        "submit", "submits", "submitted", "submitting",
        "publish", "publishes", "published", "publishing",
    ];
    const UPDATE: &[&str] = &[
        "update", "updates", "updated", "updating",
        "edit", "edits", "edited", "editing",
        "modify", "modifies", "modified", "modifying",
        "change", "changes", "changed", "changing",
        "revise", "revises", "revised", "revising",
    ];
    const DELETION: &[&str] = &[
        "delete", "deletes", "deleted", "deleting",
        "remove", "removes", "removed", "removing",
        "deactivate", "deactivates", "deactivated", "deactivating",
        "cancel", "cancels", "cancelled", "canceled", "cancelling", "canceling",
        "archive", "archives", "archived", "archiving",
    ];
    if words.iter().any(|w| CREATION.contains(&w.as_str())) {
        EventOperation::Creation
    } else if words.iter().any(|w| UPDATE.contains(&w.as_str())) {
        EventOperation::Update
    } else if words.iter().any(|w| DELETION.contains(&w.as_str())) {
        EventOperation::Deletion
    } else {
        EventOperation::Other
    }
}

/// Classifies an event name by its exact canonical suffix — the naming rules elsewhere in this
/// same prompt only ever allow an event to end in one of these four words, so this is an exact
/// check against a closed set, not a heuristic guess the way a verb-substring match would be.
fn classify_operation_from_event_name(event_name: &str) -> EventOperation {
    if event_name.ends_with("Created") || event_name.ends_with("Registered") {
        EventOperation::Creation
    } else if event_name.ends_with("Updated") {
        EventOperation::Update
    } else if event_name.ends_with("Deleted") {
        EventOperation::Deletion
    } else {
        EventOperation::Other
    }
}

/// Mechanically pre-computes whether a domain-event ADR for THIS story's entity AND operation
/// already exists, replacing a holistic "scan the ADR list and judge" instruction with a stated
/// fact the model can't misjudge — the same "enumeration over holistic review" shift already
/// proven across this pipeline. Live-verified need: `architectural_questions_prompt`'s previous
/// "skip only if a domain-event ADR for THIS story's entity and operation already exists...
/// check precisely" instruction reproduced a duplicate domain-event ADR in 2 of 3 runs in a
/// reproducibility sweep, even with the existing ADR shown verbatim in context.
///
/// Matching requires ALL of: the story's own `want` text names a known domain entity as a whole
/// word (not a substring inside an unrelated word, e.g. "Order" inside "reorder" — though a real
/// English word that happens to equal an entity name, e.g. "order" appearing as its own word in
/// an unrelated idiom, is a deeper semantic ambiguity this can't resolve; accepted as a residual,
/// low-likelihood limitation); an ADR whose title mentions "event"; whose decision text (the part
/// before " on topic ", if present) starts with that entity's name case-insensitively; contains
/// no "-" or " " (excludes a false-positive match against a kebab-case service-ownership decision
/// like "widget-service", which also starts with the entity name but is not an event); AND whose
/// own operation classification (via its exact `Created`/`Registered`/`Updated`/`Deleted` suffix)
/// matches the story's own classified operation, AND that classification is not `Other` on
/// either side — this last condition is what stops an existing `WidgetRegistered` ADR from
/// suppressing a genuinely-needed `WidgetUpdated` proposal for a later, different story about the
/// same entity, and stops two operations this function can't confidently classify from wrongly
/// comparing equal to each other just because both fell through to `Other`. `likely_entity` is
/// only found when the story's own `want` text names a known domain entity — on a story whose
/// entity has no vocabulary yet, or one whose `want` mentions more than one known entity
/// (ambiguous — picks the first match), this returns `None` and the model keeps full discretion,
/// same as before this fix.
fn find_existing_domain_event_for_story<'a>(
    story: &UserStory,
    existing_adrs: &'a [Adr],
    domain: &DomainRegistry,
) -> Option<(&'a str, &'a str)> {
    let want_words: Vec<String> = story.want.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect();
    let likely_entity = domain.entities.iter()
        .find(|e| want_words.contains(&e.name().to_lowercase()))?;
    let entity_lower = likely_entity.name().to_lowercase();
    let story_operation = classify_operation_from_want(&want_words);
    if story_operation == EventOperation::Other {
        return None;
    }
    existing_adrs.iter().find(|adr| {
        let event_part = adr.decision.split(" on topic ").next().unwrap_or(&adr.decision).trim();
        adr.title.to_lowercase().contains("event")
            && event_part.to_lowercase().starts_with(&entity_lower)
            && !event_part.contains('-')
            && !event_part.contains(' ')
            && classify_operation_from_event_name(event_part) == story_operation
    }).map(|adr| (adr.decision.as_str(), adr.title.as_str()))
}

/// Category 1's domain-event ADR sub-item (added 2026-07-13) replaces a single generic "event
/// design" bullet with an explicit creation|update|deletion classification. Live-verified need:
/// two structurally identical creation stories (register a product, register a manufacturer),
/// same event-driven architecture and broker already decided — one got a domain-event ADR
/// proposed, the other didn't, with no signal anywhere for why the model treated them
/// differently. The spec-generation step later invented the event anyway
/// (`ManufacturerRegistered`), proving the concept was inferable from the story all along; it
/// just wasn't surfaced when ADRs get proposed. Same shape of fix as Stage 0's original
/// checklist rewrite: replace implicit pattern recognition with an explicit per-story
/// classification the model can't skip.
fn architectural_questions_prompt(
    story: &UserStory,
    existing_adrs: &[Adr],
    services: &ServicesRegistry,
    domain: &DomainRegistry,
) -> String {
    let adrs_summary = if existing_adrs.is_empty() {
        "None yet.".to_string()
    } else {
        existing_adrs
            .iter()
            .map(|a| format!("- {}: {}", a.title, a.decision))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let services_summary = if services.services.is_empty() {
        "None yet.".to_string()
    } else {
        services
            .services
            .iter()
            .map(|s| {
                let tech = s.technology.as_deref().unwrap_or("unknown");
                let resp = s.responsibilities.join(", ");
                format!("- {} [{}]: {}", s.name, tech, resp)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let domain_event_status = match find_existing_domain_event_for_story(story, existing_adrs, domain) {
        Some((decision, title)) => format!(
            "Existing ADR: \"{decision}\" ({title}).\nNEVER propose another domain-event ADR for \
             this story."
        ),
        None => format!(
            "No domain-event ADR exists yet.\nIf the Architecture Style ADR above is \
             event-driven and this story's action creates, updates, or deletes an aggregate, \
             ALWAYS propose exactly one: classify the action (want: \"{want}\") as exactly one of \
             creation | update | deletion | other, then name it per the Naming rules below.",
            want = story.want,
        ),
    };
    format!(
        r#"You are an experienced software architect.

A team is about to write a BDD specification for this user story:
  As a {as_a}, I want {want}, so that {so_that}

Existing Architecture Decisions:
{adrs_summary}

Known Services and Responsibilities:
{services_summary}

Domain Event Status: {domain_event_status}

SKIP a question entirely if its answer is already captured above. Check precisely before proposing:
- Service ownership: skip if the specific service that should own THIS story's domain is already in Known Services.
- UI/frontend: skip if a frontend that serves THIS actor's interaction for THIS capability is already in Known Services.
  Do NOT skip just because some other frontend exists for a different actor or purpose.
- Tech stack: skip if the specific service already has a decided technology in Known Services.
- Database infrastructure: skip if the specific data-owning service already has a database in Known Services.
  Do NOT skip just because some other service already has a database.
- Event broker infrastructure: skip if an event broker entry already exists in Known Services.
Propose ONLY questions where the decision is genuinely absent from the above context.
If all decisions are already made, return an empty proposals list.

Include ALL of:
1. Structural questions — service ownership, data responsibility, integration contracts, API boundaries.
   - Domain event ADR — follow Domain Event Status above exactly.
2. UI questions — if the story has a human actor performing an action, there must be a frontend component
   through which they act. Ask what UI delivers this capability and propose it as a new service if not yet decided.
   - Set service to the kebab-case frontend component name (e.g. admin-portal) for BOTH the component proposal AND its tech stack proposal.
   - Immediately after proposing a new frontend component, propose its tech stack as the very next question.
3. Tech stack questions — for every new backend service introduced in category 1, what technology should it use?
   This is MANDATORY — never omit a tech stack proposal for a newly introduced backend service.
   Set service to the service name and component_type to "service".
   Suggest the most pragmatic and common choice, but a human will decide before accepting.
4. Infrastructure questions — if not yet decided:
   - Persistent storage: what database does each service use to store its data?
     Propose one per service that owns data. Suggest the most appropriate type (relational, document, etc.)
   - Event infrastructure: if the Architecture Style ADR says "event-driven", an event broker is
     MANDATORY infrastructure. Propose it unconditionally if no event broker is in Known Services —
     do not wait for the story to mention events explicitly.

Naming rules — strictly enforced:
- Service, frontend, and infrastructure component names: kebab-case only (user-service, booking-service, admin-portal, client-portal, redpanda, postgresql)
  Never use PascalCase or camelCase for component names.
  Never append "Service", "DB", or "Database" as a suffix to service names.
- Domain event names: PascalCase past tense, prefixed with the entity name (<Entity>Created, <Entity>Updated, <Entity>Deleted)
  Never use kebab-case for event names.
- Domain event ADR decisions: when a Topic Naming Convention ADR exists in Existing Architecture Decisions,
  derive the topic name from it and format the decision as "<EventName> on topic <topic-name>".
  Example: "<Entity>Created on topic <entity>-events"
  The topic name is the entity name in kebab-case with an "-events" suffix.
  If no Topic Naming Convention ADR exists, name the event only (no topic).

For tech stack proposals:
- Set technology to the canonical technology name (e.g. "Spring Boot", "Angular", "React",
  "Vue", "Next.js", "Node.js", "PostgreSQL", "MongoDB", "Redpanda", "Kafka")
- Set component_type to:
  - "frontend"        — browser UI
  - "service"         — backend application service
  - "infrastructure"  — shared infrastructure (database, event broker, cache)

Infrastructure components are shared — propose them once, not once per service.

Return ONLY valid YAML — no prose, no code fences:

proposals:
  - question: "<the architectural question>"
    title: "<short ADR title>"
    decision: "<recommended decision>"
    reason: "<why this is the right decision>"
    alternatives:
      - "<alternative considered>"
    service: "<kebab-case-name or null>"
    service_responsibilities:
      - "<responsibility>"
    technology: "<technology name or null>"
    component_type: "<frontend | service | infrastructure | null>"
"#,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        adrs_summary = adrs_summary,
        services_summary = services_summary,
        domain_event_status = domain_event_status,
    )
}

pub fn identify_architectural_questions(
    client: &LlmClient,
    story: &UserStory,
    existing_adrs: &[Adr],
    services: &ServicesRegistry,
    domain: &DomainRegistry,
) -> Result<ProposedAdrs, LlmError> {
    let raw = client.complete_large(&architectural_questions_prompt(story, existing_adrs, services, domain))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    // Live-verified need: one malformed proposal (an explicit `null` in a Vec<String> field)
    // previously failed `serde_yaml`'s atomic Vec<T> deserialization for the WHOLE batch, losing
    // 5 other correctly-formed proposals along with it — parse item-by-item instead, same as
    // Stage 0-4's own LLM-output parsing.
    let proposals = parse_lenient_sequence::<ProposedAdr>(stripped, "proposals")?;
    Ok(ProposedAdrs { proposals })
}

#[cfg(test)]
mod domain_event_status_tests {
    use super::find_existing_domain_event_for_story;
    use canopy_core::{Adr, DomainEntity, DomainRegistry, UserStory};

    fn story(want: &str) -> UserStory {
        UserStory {
            id: "widget-001".to_string(),
            as_a: "actor".to_string(),
            want: want.to_string(),
            so_that: "reason".to_string(),
            depends_on: vec![],
            status: Default::default(),
        }
    }

    fn adr(title: &str, decision: &str) -> Adr {
        Adr {
            title: title.to_string(),
            decision: decision.to_string(),
            reason: "reason".to_string(),
            alternatives: vec![],
        }
    }

    fn domain_with(entities: &[&str]) -> DomainRegistry {
        DomainRegistry {
            entities: entities.iter().map(|e| DomainEntity::Simple(e.to_string())).collect(),
            events: vec![],
        }
    }

    #[test]
    fn finds_an_existing_domain_event_with_a_topic() {
        let adrs = vec![adr("Domain Event for Widget Registration", "WidgetRegistered on topic widget-events")];
        let domain = domain_with(&["Widget"]);
        let found = find_existing_domain_event_for_story(&story("register a widget"), &adrs, &domain);
        assert_eq!(found, Some(("WidgetRegistered on topic widget-events", "Domain Event for Widget Registration")));
    }

    #[test]
    fn finds_an_existing_domain_event_with_no_topic() {
        // Live-verified need: the actual dogfooding project's domain-event ADR had no topic at
        // all (no Topic Naming Convention ADR existed), which `parse_event_adr` alone can't
        // detect since it requires " on topic " to match — this function must not depend on
        // that split succeeding.
        let adrs = vec![adr("Domain Event for Widget Registration", "WidgetRegistered")];
        let domain = domain_with(&["Widget"]);
        let found = find_existing_domain_event_for_story(&story("register a widget"), &adrs, &domain);
        assert_eq!(found, Some(("WidgetRegistered", "Domain Event for Widget Registration")));
    }

    #[test]
    fn does_not_false_positive_on_a_kebab_case_service_decision() {
        // Live-verified need: "Service Ownership for Widget Registration: widget-service" starts
        // with the entity name too (case-insensitively) but is not a domain event.
        let adrs = vec![adr("Service Ownership for Widget Registration", "widget-service")];
        let domain = domain_with(&["Widget"]);
        let found = find_existing_domain_event_for_story(&story("register a widget"), &adrs, &domain);
        assert_eq!(found, None);
    }

    #[test]
    fn returns_none_when_no_domain_event_adr_exists_yet() {
        let adrs = vec![adr("Service Ownership for Widget Registration", "widget-service")];
        let domain = domain_with(&["Widget"]);
        let found = find_existing_domain_event_for_story(&story("register a widget"), &adrs, &domain);
        assert_eq!(found, None);
    }

    #[test]
    fn returns_none_when_the_story_names_no_known_entity() {
        let adrs = vec![adr("Domain Event for Widget Registration", "WidgetRegistered")];
        let domain = domain_with(&["Widget"]);
        let found = find_existing_domain_event_for_story(&story("register a gadget"), &adrs, &domain);
        assert_eq!(found, None);
    }

    #[test]
    fn returns_none_when_domain_vocabulary_is_empty() {
        let adrs = vec![adr("Domain Event for Widget Registration", "WidgetRegistered")];
        let domain = domain_with(&[]);
        let found = find_existing_domain_event_for_story(&story("register a widget"), &adrs, &domain);
        assert_eq!(found, None);
    }

    #[test]
    fn a_creation_event_adr_does_not_suppress_a_later_update_story_for_the_same_entity() {
        // Regression the reviewer caught: matching on entity alone (ignoring operation) would
        // wrongly suppress a genuinely-needed WidgetUpdated ADR just because WidgetRegistered
        // already exists for the same entity.
        let adrs = vec![adr("Domain Event for Widget Registration", "WidgetRegistered")];
        let domain = domain_with(&["Widget"]);
        let found = find_existing_domain_event_for_story(&story("update a widget"), &adrs, &domain);
        assert_eq!(found, None);
    }

    #[test]
    fn matches_case_insensitively_against_a_lowercase_edited_domain_entity() {
        // domain_registry.yaml is documented as human-edit-freely, so entity casing isn't
        // guaranteed to stay PascalCase.
        let adrs = vec![adr("Domain Event for Widget Registration", "WidgetRegistered")];
        let domain = domain_with(&["widget"]);
        let found = find_existing_domain_event_for_story(&story("register a widget"), &adrs, &domain);
        assert_eq!(found, Some(("WidgetRegistered", "Domain Event for Widget Registration")));
    }

    #[test]
    fn does_not_match_an_entity_name_as_a_substring_inside_an_unrelated_word() {
        // "Order" must not match inside "reorder" — only a whole-word occurrence of the entity
        // name in the story's want counts. (A real English word that happens to equal an entity
        // name, e.g. "order" appearing as its own word in an unrelated idiom, is a deeper
        // semantic ambiguity word-boundary tokenization can't resolve — accepted as a residual,
        // low-likelihood limitation rather than something this fix claims to solve.)
        let adrs = vec![adr("Domain Event for Order Registration", "OrderRegistered")];
        let domain = domain_with(&["Order"]);
        let found = find_existing_domain_event_for_story(
            &story("reorder low-stock warehouse items"), &adrs, &domain,
        );
        assert_eq!(found, None);
    }

    #[test]
    fn does_not_misclassify_an_update_story_as_creation_via_a_verb_substring() {
        // Regression: an earlier version classified operations via raw substring match, so
        // "update the widget's shipping address" was misclassified Creation because it contains
        // "add" (as a substring of "address") before ever reaching the "updat" check.
        let adrs = vec![adr("Domain Event for Widget Registration", "WidgetRegistered")];
        let domain = domain_with(&["Widget"]);
        let found = find_existing_domain_event_for_story(
            &story("update the widget's shipping address"), &adrs, &domain,
        );
        assert_eq!(found, None);
    }

    #[test]
    fn does_not_match_two_unclassifiable_operations_via_an_other_equals_other_blind_spot() {
        // Regression: comparing EventOperation::Other == EventOperation::Other would wrongly
        // treat two operations neither classifier recognizes as "the same operation" — a story
        // whose want matches none of the known verbs must never assert a match against an
        // existing ADR, even one whose own event name is equally unclassifiable ("WidgetSynced"
        // ends in neither Created/Registered/Updated/Deleted).
        let adrs = vec![adr("Domain Event for Widget Registration", "WidgetSynced")];
        let domain = domain_with(&["Widget"]);
        let found = find_existing_domain_event_for_story(
            &story("resize the widget thumbnail"), &adrs, &domain,
        );
        assert_eq!(found, None);
    }
}

fn context_sections(adrs: &[Adr], services: &ServicesRegistry, domain: &DomainRegistry) -> (String, String, String, String) {
    let adrs_summary = if adrs.is_empty() {
        "None yet.".to_string()
    } else {
        adrs.iter()
            .map(|a| format!("- {}: {}", a.title, a.decision))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let services_summary = if services.services.is_empty() {
        "None yet.".to_string()
    } else {
        services
            .services
            .iter()
            .map(|s| format!("- {}: {}", s.name, s.responsibilities.join(", ")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let entities = if domain.entities.is_empty() {
        "none yet".to_string()
    } else {
        domain.entities.iter()
            .map(|e| match e.description() {
                Some(d) => format!("{} — {}", e.name(), d),
                None => e.name().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let events = if domain.events.is_empty() {
        "none yet".to_string()
    } else {
        domain.events.iter()
            .map(|e| match e.description() {
                Some(d) => format!("{} — {}", e.name(), d),
                None => e.name().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    (adrs_summary, services_summary, entities, events)
}

/// Call 1 of 2 (added 2026-07-13, replacing the previous single-call design): defines the entity
/// schema and resolves business policy — no scenario-writing here. Live-verified motivation: with
/// entity_schema and scenarios generated in one holistic call, Stage 0 was repeatedly finding the
/// same shape of omission (missing constraint-violation scenarios) after the fact, story after
/// story — a downstream safety net catching a gap scenario generation itself kept leaving behind.
/// Splitting the schema out lets a mechanical step (`scenario_coverage_matrix`) enumerate exactly
/// what Call 2 needs to cover, the same "coverage first, narrative second" shift that already
/// fixed Stage 0's own checklist and Stage 1's behavior extraction.
fn entity_schema_prompt(
    story: &UserStory,
    adrs: &[Adr],
    services: &ServicesRegistry,
    domain: &DomainRegistry,
) -> String {
    let (adrs_summary, services_summary, entities, events) = context_sections(adrs, services, domain);
    format!(
        r#"You are a BDD expert defining the domain model and business policy for a user story.

Story ID: {story_id}
User Story: As a {as_a}, I want {want}, so that {so_that}

Architecture Decisions in Effect:
{adrs_summary}

Services and Responsibilities:
{services_summary}

Domain Entities: {entities}
Domain Events: {events}

Creation story detection:
This is a CREATION STORY if the want contains a creation verb (register, create, add, onboard,
submit, publish) OR if the domain events include a {{Entity}}Created event for an entity in
the want statement.

If this is a creation story, you MUST output an entity_schema section.
If this is NOT a creation story, omit entity_schema entirely.

entity_schema rules — identify the primary entity being created and define its fields:

system_generated — fields the system sets automatically; the actor never provides these:
  - Always include: id (uuid), createdAt (datetime)
  - Always include: modifiedAt (datetime) — null at creation, updated on every write
  - Include business-operation timestamps for any domain event that implies a later state
    transition on this entity (e.g. AccountActivated → activatedAt datetime,
    DocumentPublished → publishedAt datetime). Set these to null at creation.
  - Do not include actor-provided fields here.

mandatory — fields the actor MUST provide when registering the entity:
  - These are the minimum data required for the entity to exist in the domain.
  - Infer from domain context, industry norms, and common sense for this entity type.
  - Do not include system_generated fields here.

optional — fields the actor MAY provide; nullable or defaulted:
  - These enrich the entity but are not required for it to exist.

Field format: name (camelCase), type (uuid | string | integer | decimal | boolean | datetime |
"[string]" | "[uuid]"), description (one sentence), validation (see rules below).

Validation rules — mandatory and optional fields MUST include a validation block.
system_generated fields do NOT need a validation block.
  string:    max_length (required — pick a domain-appropriate ceiling, e.g. 200 for a name,
                         2000 for a description, 500 for a URL)
             min_length (required — 1 for mandatory fields, 0 for optional)
  integer:   min (required), max (required)
  decimal:   min (required), max (required)
  boolean:   omit validation block
  uuid:      omit validation block
  datetime:  omit validation block
  "[string]": max_items (required), max_length (required — max length per individual item)
  "[uuid]":  max_items (required)

Business policy checklist — output EXACTLY the 6 items below in policy_checklist, in this exact
order, using this exact area name for each. Do not skip an area. Do not add an extra item.
1. area "uniqueness" — must any field, or combination of fields, be unique across all existing
   records of this entity?
2. area "defaults" — does any optional field have an implied default value when the actor omits it?
3. area "retention" — is there a rule for how long this entity persists, or when it expires or
   archives?
4. area "authorization" — does creating or modifying this entity require a specific role or
   permission beyond the actor already being authenticated?
5. area "idempotency" — if the actor submits the same request twice, must that be rejected as a
   duplicate, or is it safely repeatable?
6. area "consistency" — does creating this entity depend on, or affect, the state of any other
   entity?

Policy Discovery finds decisions that already exist — it never makes new ones. For EACH item,
set classification to exactly one of these three values — NEVER any other value:
- "resolved" — ONLY if the story, an Architecture Decision above, or the Domain Entities/Events
  above EXPLICITLY states this rule. detail states the rule; evidence quotes or names the exact
  source (e.g. "ADR: Domain Event for Widget Registration", "story: as_a widget administrator",
  or "domain vocabulary: Widget — the entity this story registers").
- "not_applicable" — ONLY if the domain model above structurally rules this out (e.g. no other
  entities exist at all, so consistency cannot apply). detail states the structural reason;
  evidence names the source, same as for "resolved" — this is not a free pass from grounding.
- "unresolved" — the default whenever no explicit evidence exists either way. detail states the
  question for a human to answer; omit evidence.
NEVER invent a plausible-sounding rule to avoid "unresolved" — a business rule with no textual
support above is a guess, not a resolution. When uncertain between resolved and unresolved,
ALWAYS choose unresolved.

Return ONLY valid YAML — no prose, no code fences.
YAML string rules — you MUST follow these to avoid parse errors:
- Any string value containing a colon (:) MUST be enclosed in double quotes
- Any list item ending with a question mark (?) MUST be enclosed in double quotes
- type values are strings: string, integer, decimal, uuid, datetime, boolean, "[string]", "[uuid]" — always quote bracket forms: type: "[string]" not type: [string]

entity_schema:                    # omit entirely if not a creation story
  entity: "<PascalCase entity name>"
  system_generated:
    - name: "<camelCase>"
      type: "<type>"
      description: "<one sentence>"
      # no validation block for system_generated fields
  mandatory:
    - name: "<camelCase>"
      type: "<type>"
      description: "<one sentence>"
      validation:
        max_length: <N>       # string: required; [string]: required (per-item ceiling)
        min_length: <N>       # string: required (1 for mandatory)
        min: <N>              # integer/decimal: required
        max: <N>              # integer/decimal: required
        max_items: <N>        # [string]/[uuid]: required
  optional:
    - name: "<camelCase>"
      type: "<type>"
      description: "<one sentence>"
      validation:
        max_length: <N>       # string: required; [string]: required (per-item ceiling)
        min_length: 0         # string: 0 for optional fields
        min: <N>              # integer/decimal: required
        max: <N>              # integer/decimal: required
        max_items: <N>        # [string]/[uuid]: required
policy_checklist:
  - area: "uniqueness"
    classification: "resolved | not_applicable | unresolved"
    detail: "<concrete rule, structural reason, or concrete question>"
    evidence: "<exact source quoted/named — omit only if unresolved>"
  - area: "defaults"
    classification: "resolved | not_applicable | unresolved"
    detail: "<concrete rule, structural reason, or concrete question>"
    evidence: "<exact source quoted/named — omit only if unresolved>"
  - area: "retention"
    classification: "resolved | not_applicable | unresolved"
    detail: "<concrete rule, structural reason, or concrete question>"
    evidence: "<exact source quoted/named — omit only if unresolved>"
  - area: "authorization"
    classification: "resolved | not_applicable | unresolved"
    detail: "<concrete rule, structural reason, or concrete question>"
    evidence: "<exact source quoted/named — omit only if unresolved>"
  - area: "idempotency"
    classification: "resolved | not_applicable | unresolved"
    detail: "<concrete rule, structural reason, or concrete question>"
    evidence: "<exact source quoted/named — omit only if unresolved>"
  - area: "consistency"
    classification: "resolved | not_applicable | unresolved"
    detail: "<concrete rule, structural reason, or concrete question>"
    evidence: "<exact source quoted/named — omit only if unresolved>"
out_of_scope:
  - "<explicitly excluded concern>"
"#,
        story_id = story.id,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        adrs_summary = adrs_summary,
        services_summary = services_summary,
        entities = entities,
        events = events,
    )
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PolicyChecklistItem {
    area: String,
    classification: String,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    evidence: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SchemaAndPolicyDraft {
    #[serde(default)]
    entity_schema: Option<EntitySchema>,
    #[serde(default)]
    policy_checklist: Vec<PolicyChecklistItem>,
    #[serde(default)]
    out_of_scope: Vec<String>,
}

/// Mechanically buckets Call 1's forced per-item policy classification into the two downstream
/// `IntentSpec` fields — a deterministic transform of the model's own explicit output, not a
/// correction of it (the audit/compensation distinction in CLAUDE.md's Prompt House Style).
/// Live-verified need #1: even with a per-item resolved/not_applicable/unresolved classification
/// already asked for, a run put 3 of 6 items in a 4th, unsanctioned bucket (`out_of_scope`)
/// instead — that case has a `detail` the model actually wrote, so relaying it into
/// open_questions unchanged is legitimate bucketing, not compensation.
///
/// Live-verified need #2: a later run showed "resolved" being used for plausible-sounding
/// business rules with no grounding anywhere upstream (a role name, a default value, a retention
/// policy, none stated in the story/ADRs/domain vocabulary) — the model was answering business
/// questions instead of surfacing them, exactly the failure mode Entity/Event Continuity and
/// Decision Points elsewhere in this pipeline exist to prevent. `entity_schema_prompt` now
/// requires an `evidence` field alongside `detail` for "resolved" — this function enforces that
/// requirement is actually met rather than trusting the classification alone: "resolved" with a
/// `detail` but no `evidence` has nothing grounding it, so it fails loudly and asks for a re-run
/// rather than accepting an ungrounded resolution.
///
/// A "resolved" or "not_applicable" item missing `detail` or `evidence` is ungrounded either
/// way — the prompt now demands the same grounding bar for both, precisely so a model looking
/// for the path of least resistance can't dodge the "resolved" audit by mislabeling a fabricated
/// item as "not_applicable" instead. An unrecognized-classification item with NO `detail` at all
/// is the same shape too: there is no model-authored text to relay. Rather than fabricate a
/// question the model never asked (compensation — the exact thing this house rule forbids), fail
/// loudly, the same shape as `check_entity_continuity`/`check_event_continuity`: nothing is
/// saved, and the caller re-runs `canopy spec`.
fn bucket_policy_checklist(items: Vec<PolicyChecklistItem>) -> Result<(Vec<ResolvedPolicy>, Vec<String>), LlmError> {
    let mut resolved = Vec::new();
    let mut open_questions = Vec::new();
    for item in items {
        match item.classification.trim().to_lowercase().as_str() {
            "resolved" => match (item.detail, item.evidence) {
                (Some(resolution), Some(evidence)) => {
                    if is_unsupported_absence_claim(&resolution) {
                        return Err(LlmError::UnexpectedShape(format!(
                            "policy checklist item '{}' was classified 'resolved' but its own \
                             resolution text reports an absence rather than stating a rule (\"{}\") \
                             — treating silence as permission is not a resolution. Re-run \
                             `canopy spec`.",
                            item.area, resolution
                        )));
                    }
                    resolved.push(ResolvedPolicy { area: item.area, resolution, evidence })
                }
                _ => return Err(LlmError::UnexpectedShape(format!(
                    "policy checklist item '{}' was classified 'resolved' but no detail and/or \
                     no evidence was provided — a resolution with no grounding is a guess, not a \
                     resolution. Re-run `canopy spec`.",
                    item.area
                ))),
            },
            "not_applicable" | "not applicable" => match (&item.detail, &item.evidence) {
                (Some(detail), Some(_)) => {
                    if is_unsupported_absence_claim(detail) {
                        return Err(LlmError::UnexpectedShape(format!(
                            "policy checklist item '{}' was classified 'not_applicable' but its \
                             own detail text reports an absence rather than stating a structural \
                             reason (\"{}\") — treating silence as permission is not a resolution. \
                             Re-run `canopy spec`.",
                            item.area, detail
                        )));
                    }
                }
                _ => return Err(LlmError::UnexpectedShape(format!(
                    "policy checklist item '{}' was classified 'not_applicable' but no \
                     detail and/or no evidence was provided — a structural exemption with no \
                     grounding is a guess, not a resolution. Re-run `canopy spec`.",
                    item.area
                ))),
            },
            _ => match item.detail {
                Some(detail) => open_questions.push(detail),
                None => return Err(LlmError::UnexpectedShape(format!(
                    "policy checklist item '{}' had classification '{}' (not one of resolved | \
                     not_applicable | unresolved) and no detail — nothing to surface as an open \
                     question. Re-run `canopy spec`.",
                    item.area, item.classification
                ))),
            },
        }
    }
    Ok((resolved, open_questions))
}

/// A `resolved`/`not_applicable` item can pass the grounding check above (both `detail` and
/// `evidence` present) while still being unsupported: `detail` reports that the input is *silent*
/// on the question, and that silence is then treated as if it settled the question — live-verified
/// (product-010's real `authorization` resolution: `detail` = "the story does not explicitly
/// mention any authorization requirements for browsing a catalog", `evidence` = a genuine verbatim
/// quote of the story). Presence of `evidence` doesn't help here — the evidence can be completely
/// real and the resolution still invalid, because absence of a stated requirement is not
/// confirmation none exists (docs/design/mechanism-b-implementation-evaluation.md).
///
/// Deliberately narrow: matches self-referential absence-of-mention framing ("the input doesn't
/// say X"), not general negation of a domain rule ("X is not required"). The latter is completely
/// ordinary, legitimate phrasing for a real, grounded resolution (e.g. "authorization not required
/// — catalog browsing is intentionally public") and must not be rejected — scoping to the specific
/// self-referential shape is what keeps this precise instead of just flagging the word "not".
fn is_unsupported_absence_claim(text: &str) -> bool {
    const MARKERS: &[&str] = &[
        "does not mention", "does not explicitly mention",
        "does not state", "does not explicitly state",
        "does not specify", "no mention of", "not mentioned",
        "not specified in", "not stated in",
        "nothing in the story", "nothing indicates",
    ];
    let lower = text.to_lowercase();
    MARKERS.iter().any(|m| lower.contains(m))
}

/// Mechanically enumerates every scenario the spec needs, BEFORE any scenario is written — the
/// same "exhaustive enumeration over holistic review" principle that fixed Stage 0's own
/// constraint checklist, applied one stage earlier so scenario generation has an explicit
/// inventory to fill in rather than an implicit one to invent. One item per field constraint
/// (mirroring Stage 0's own field×constraint enumeration exactly, so what this produces and what
/// Stage 0 later checks are the same list by construction), one for missing-mandatory-fields, two
/// happy-path items, and one per resolved business policy.
fn scenario_coverage_matrix(schema: &EntitySchema, resolved_policies: &[ResolvedPolicy]) -> Vec<String> {
    let mut items = Vec::new();

    let system_generated_fields = schema.system_generated.iter()
        .map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
    items.push(format!(
        "Success: the actor submits only the mandatory fields — the {} is registered, its \
         system-generated fields ({}) each hold the exact creation-time value the schema \
         defines for them — non-null where the schema gives a real value, explicitly null where \
         the schema marks null-at-creation — and any domain event is published.",
        schema.entity, system_generated_fields
    ));
    if !schema.optional.is_empty() {
        items.push(format!(
            "Success: the actor submits the mandatory fields AND the optional fields — the {} is \
             registered with the optional values stored as provided.",
            schema.entity
        ));
    }

    for field in schema.mandatory.iter().chain(schema.optional.iter()) {
        let Some(v) = &field.validation else { continue };
        if let Some(n) = v.max_length {
            items.push(format!("Failure: '{}' longer than {n} characters is rejected.", field.name));
        }
        if let Some(n) = v.min_length {
            if n > 0 {
                items.push(format!("Failure: '{}' shorter than {n} characters is rejected.", field.name));
            }
        }
        if let Some(n) = v.min {
            items.push(format!("Failure: '{}' below {n} is rejected.", field.name));
        }
        if let Some(n) = v.max {
            items.push(format!("Failure: '{}' above {n} is rejected.", field.name));
        }
        if v.pattern.is_some() {
            items.push(format!("Failure: '{}' violating the required pattern is rejected.", field.name));
        }
        if let Some(n) = v.max_items {
            items.push(format!("Failure: more than {n} items for '{}' is rejected.", field.name));
        }
    }

    if !schema.mandatory.is_empty() {
        items.push("Failure: the actor submits with mandatory fields missing entirely — the \
                     registration is rejected, naming which fields are required.".to_string());
    }

    for policy in resolved_policies {
        items.push(format!(
            "Resolved policy ({}): {}. Write the scenario this policy implies (a rejection case, \
             if the policy states a constraint that can be violated).",
            policy.area, policy.resolution
        ));
    }

    items
}

/// Call 2 of 2: writes exactly one scenario per already-enumerated coverage item — a checklist
/// traversal, not a discovery exercise, the same shape as Stage 1's SUCCESS/FAILURE checklist
/// that has consistently been the strongest-performing prompt in this pipeline.
fn scenario_generation_prompt(
    story: &UserStory,
    schema: &EntitySchema,
    coverage_matrix: &[String],
    adrs: &[Adr],
) -> String {
    let coverage_list = coverage_matrix.iter().enumerate()
        .map(|(i, item)| format!("{}. {item}", i + 1))
        .collect::<Vec<_>>().join("\n");
    let adrs_summary = if adrs.is_empty() {
        "None yet.".to_string()
    } else {
        adrs.iter()
            .map(|a| format!("- {}: {}", a.title, a.decision))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mandatory_fields = schema.mandatory.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");
    let optional_fields = if schema.optional.is_empty() {
        "none".to_string()
    } else {
        schema.optional.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")
    };
    let system_generated_fields = schema.system_generated.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ");

    format!(
        r#"You are a BDD expert writing acceptance criteria from an ALREADY-DETERMINED coverage
requirement list — you are NOT deciding what needs to be tested, only writing the Given/When/Then
for each requirement already listed below.

Story ID: {story_id}
User Story: As a {as_a}, I want {want}, so that {so_that}

Entity: {entity}
Mandatory fields: {mandatory_fields}
Optional fields: {optional_fields}
System-generated fields: {system_generated_fields}

Architecture Decisions in Effect:
{adrs_summary}

Coverage requirements — for EACH item below, ONE AT A TIME, write exactly one BDD scenario
satisfying it. Do not skip any. Do not add scenarios for requirements not listed here:

{coverage_list}

Scenario rules:
- Scenarios describe OBSERVABLE BEHAVIOR from the user's perspective — never internal API calls,
  HTTP verbs, JSON payloads, or implementation details
- Given describes actor context and relevant domain state only — never system availability,
  portal health, service status, or infrastructure operations.
  Good: "The actor is authenticated and no {entity} with this name exists."
  Bad:  "The admin portal is operating normally."
- When describes what the actor does — one action per scenario. For a success requirement, MUST
  explicitly name the mandatory fields the actor submits.
- Then describes what the actor observes or what changed in the domain — never service internals,
  technology names, or infrastructure operations. For a success requirement, MUST reference the
  system-generated fields set at creation.
  Good: "The {entity} is registered and a {entity}Created event is published."
  Bad:  "WidgetService stores the data and the message broker receives the event."
- constraints: state the exact rule this scenario tests (empty list if the requirement is a
  happy-path success case with no constraint being violated)
- Scenario IDs must follow the pattern: {story_id}-01, {story_id}-02, etc., in the same order as
  the coverage requirements above.

Return ONLY valid YAML — no prose, no code fences.
YAML string rules — you MUST follow these to avoid parse errors:
- Any string value containing a colon (:) MUST be enclosed in double quotes
- Any list item ending with a question mark (?) MUST be enclosed in double quotes

scenarios:
  - id: "{story_id}-01"
    name: "<scenario name>"
    given:
      - "<world state precondition>"
    when: "<user or system action>"
    then:
      - "<observable outcome>"
    constraints:
      - "<constraint or empty list>"
"#,
        story_id = story.id,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        entity = schema.entity,
        mandatory_fields = mandatory_fields,
        optional_fields = optional_fields,
        system_generated_fields = system_generated_fields,
        adrs_summary = adrs_summary,
        coverage_list = coverage_list,
    )
}

/// Fallback for non-creation stories only — no entity_schema exists to build a coverage matrix
/// from, so this keeps the previous holistic "write BDD scenarios for this story" approach.
/// Creation stories (the common case for this pipeline so far) always use the coverage-matrix
/// path above instead.
fn fallback_scenario_prompt(story: &UserStory, adrs: &[Adr]) -> String {
    let adrs_summary = if adrs.is_empty() {
        "None yet.".to_string()
    } else {
        adrs.iter()
            .map(|a| format!("- {}: {}", a.title, a.decision))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        r#"You are a BDD expert writing acceptance criteria for a user story.

Story ID: {story_id}
User Story: As a {as_a}, I want {want}, so that {so_that}

Architecture Decisions in Effect:
{adrs_summary}

Write BDD scenarios (Given/When/Then) as acceptance criteria. Rules:
- Scenarios describe OBSERVABLE BEHAVIOR from the user's perspective — never internal API calls,
  HTTP verbs, JSON payloads, or implementation details
- Given describes actor context and relevant domain state only — never system availability,
  portal health, service status, or infrastructure operations.
  Good: "The actor is authenticated and the relevant domain state exists."
  Bad:  "The admin portal is operating normally."
- When describes what the actor does — one action per scenario
- Then describes what the actor observes or what changed in the domain — never service internals,
  technology names, or infrastructure operations.
  Good: "The actor sees the updated result."
  Bad:  "WidgetService updates the record and the message broker receives the event."
- Scenario IDs must follow the pattern: {story_id}-01, {story_id}-02, etc.

Return ONLY valid YAML — no prose, no code fences.

scenarios:
  - id: "{story_id}-01"
    name: "<scenario name>"
    given:
      - "<world state precondition>"
    when: "<user or system action>"
    then:
      - "<observable outcome>"
    constraints:
      - "<constraint or empty list>"
"#,
        story_id = story.id,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        adrs_summary = adrs_summary,
    )
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ScenarioBatch {
    #[serde(default)]
    scenarios: Vec<Scenario>,
}

pub fn generate_story_spec(
    client: &LlmClient,
    story: &UserStory,
    adrs: &[Adr],
    services: &ServicesRegistry,
    domain: &DomainRegistry,
) -> Result<IntentSpec, LlmError> {
    let raw = client.complete_large(&entity_schema_prompt(story, adrs, services, domain))?;
    let stripped = strip_code_fence(&raw);
    let fixed = fix_yaml_colon_in_scalars(&stripped);
    let draft: SchemaAndPolicyDraft = serde_yaml::from_str(&fixed)
        .map_err(|source| LlmError::YamlParse { source, raw: fixed })?;
    let (resolved_policies, open_questions) = bucket_policy_checklist(draft.policy_checklist)?;

    let scenarios = if let Some(schema) = &draft.entity_schema {
        let matrix = scenario_coverage_matrix(schema, &resolved_policies);
        let raw2 = client.complete_large(&scenario_generation_prompt(story, schema, &matrix, adrs))?;
        let stripped2 = strip_code_fence(&raw2);
        let fixed2 = fix_yaml_colon_in_scalars(&stripped2);
        let batch: ScenarioBatch = serde_yaml::from_str(&fixed2)
            .map_err(|source| LlmError::YamlParse { source, raw: fixed2 })?;
        batch.scenarios
    } else {
        let raw2 = client.complete_large(&fallback_scenario_prompt(story, adrs))?;
        let stripped2 = strip_code_fence(&raw2);
        let fixed2 = fix_yaml_colon_in_scalars(&stripped2);
        let batch: ScenarioBatch = serde_yaml::from_str(&fixed2)
            .map_err(|source| LlmError::YamlParse { source, raw: fixed2 })?;
        batch.scenarios
    };

    Ok(IntentSpec {
        intent_ref: story.id.clone(),
        entity_schema: draft.entity_schema,
        scenarios,
        resolved_policies,
        out_of_scope: draft.out_of_scope,
        open_questions,
    })
}

#[cfg(test)]
mod policy_checklist_tests {
    use super::{bucket_policy_checklist, PolicyChecklistItem};

    fn item(area: &str, classification: &str, detail: Option<&str>, evidence: Option<&str>) -> PolicyChecklistItem {
        PolicyChecklistItem {
            area: area.to_string(),
            classification: classification.to_string(),
            detail: detail.map(str::to_string),
            evidence: evidence.map(str::to_string),
        }
    }

    #[test]
    fn resolved_item_with_evidence_becomes_a_resolved_policy() {
        let (resolved, open) = bucket_policy_checklist(vec![
            item("uniqueness", "resolved", Some("name must be unique"), Some("ADR: Service Ownership")),
        ]).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].area, "uniqueness");
        assert_eq!(resolved[0].resolution, "name must be unique");
        assert_eq!(resolved[0].evidence, "ADR: Service Ownership");
        assert!(open.is_empty());
    }

    #[test]
    fn resolved_with_a_real_absence_claim_fails_loudly_even_with_genuine_evidence() {
        // The real, confirmed product-010 failure: `detail` reports that the story is silent on
        // authorization, `evidence` is a genuine verbatim quote of the real story — the existing
        // presence check alone would accept this. Isolating that from this new check: evidence is
        // present and real, so a rejection here can only be caused by the absence-claim check.
        let err = bucket_policy_checklist(vec![
            item(
                "authorization",
                "resolved",
                Some("The story does not explicitly mention any authorization requirements for browsing a catalog."),
                Some("User Story: As a customer, I want browse the published catalog, so that can see available products"),
            ),
        ]).unwrap_err();
        assert!(err.to_string().contains("authorization"));
        assert!(err.to_string().contains("treating silence as permission"));
    }

    #[test]
    fn not_applicable_with_a_real_absence_claim_fails_loudly() {
        let err = bucket_policy_checklist(vec![
            item("authorization", "not_applicable", Some("The spec does not mention any authorization rule."), Some("story text")),
        ]).unwrap_err();
        assert!(err.to_string().contains("authorization"));
    }

    #[test]
    fn a_grounded_not_required_resolution_is_not_blocked() {
        // General negation of a domain rule ("not required") must stay legitimate — only the
        // self-referential "the input doesn't say X" framing is targeted, not the word "not".
        let (resolved, _) = bucket_policy_checklist(vec![
            item(
                "authorization",
                "resolved",
                Some("Authorization not required — catalog browsing is intentionally public."),
                Some("ADR: Catalog Browsing Service Ownership"),
            ),
        ]).unwrap();
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn existing_grounded_not_applicable_fixture_is_not_blocked() {
        // "no other entities exist" cites a genuine structural fact, not an absence-of-mention
        // report — must keep passing under the new check exactly as it did before it existed.
        let (resolved, open) = bucket_policy_checklist(vec![
            item("consistency", "not_applicable", Some("no other entities exist"), Some("domain vocabulary: none")),
        ]).unwrap();
        assert!(resolved.is_empty());
        assert!(open.is_empty());
    }

    #[test]
    fn not_applicable_item_with_grounding_produces_no_output() {
        let (resolved, open) = bucket_policy_checklist(vec![
            item("consistency", "not_applicable", Some("no other entities exist"), Some("domain vocabulary: none")),
        ]).unwrap();
        assert!(resolved.is_empty());
        assert!(open.is_empty());
    }

    #[test]
    fn not_applicable_without_grounding_fails_loudly_instead_of_accepting_a_guess() {
        // Live-verified need: "not_applicable" must be audited exactly as strictly as "resolved"
        // — otherwise a model avoiding the "unresolved" default can dodge the resolved-branch
        // audit by mislabeling a fabricated exemption as "not_applicable" instead.
        let err = bucket_policy_checklist(vec![
            item("consistency", "not_applicable", None, None),
        ]).unwrap_err();
        assert!(err.to_string().contains("consistency"));
    }

    #[test]
    fn not_applicable_with_detail_but_no_evidence_fails_loudly() {
        let err = bucket_policy_checklist(vec![
            item("consistency", "not_applicable", Some("no other entities exist"), None),
        ]).unwrap_err();
        assert!(err.to_string().contains("consistency"));
    }

    #[test]
    fn not_applicable_with_evidence_but_no_detail_fails_loudly() {
        let err = bucket_policy_checklist(vec![
            item("consistency", "not_applicable", None, Some("domain vocabulary: none")),
        ]).unwrap_err();
        assert!(err.to_string().contains("consistency"));
    }

    #[test]
    fn unresolved_item_becomes_an_open_question() {
        let (resolved, open) = bucket_policy_checklist(vec![
            item("retention", "unresolved", Some("How long should records be kept?"), None),
        ]).unwrap();
        assert!(resolved.is_empty());
        assert_eq!(open, vec!["How long should records be kept?".to_string()]);
    }

    #[test]
    fn resolved_without_detail_fails_loudly_instead_of_fabricating_a_question() {
        let err = bucket_policy_checklist(vec![
            item("authorization", "resolved", None, Some("story: as_a widget administrator")),
        ]).unwrap_err();
        assert!(err.to_string().contains("authorization"));
    }

    #[test]
    fn resolved_without_evidence_fails_loudly_instead_of_accepting_a_guess() {
        // Live-verified need: the model classified several policy areas "resolved" with a
        // plausible-sounding rule (a role name, a default value, a retention window) that had no
        // support anywhere in the story/ADRs/domain vocabulary. Requiring evidence forces the
        // model to ground the rule instead of guessing — a "resolved" with no evidence at all is
        // exactly the guess this exists to catch.
        let err = bucket_policy_checklist(vec![
            item("retention", "resolved", Some("records persist indefinitely"), None),
        ]).unwrap_err();
        assert!(err.to_string().contains("retention"));
    }

    #[test]
    fn unrecognized_classification_with_detail_relays_the_models_own_text() {
        // Live-verified regression: a run classified 3 of 6 items into an unsanctioned 4th
        // bucket instead of one of the three the prompt offers. The model's own text is relayed
        // unchanged into open_questions — this is legitimate bucketing, not compensation, since
        // the model actually wrote this detail.
        let (resolved, open) = bucket_policy_checklist(vec![
            item("idempotency", "out_of_scope", Some("duplicate submissions are rare"), None),
        ]).unwrap();
        assert!(resolved.is_empty());
        assert_eq!(open, vec!["duplicate submissions are rare".to_string()]);
    }

    #[test]
    fn unrecognized_classification_without_detail_fails_loudly_instead_of_fabricating() {
        let err = bucket_policy_checklist(vec![
            item("idempotency", "out_of_scope", None, None),
        ]).unwrap_err();
        assert!(err.to_string().contains("idempotency"));
    }

    #[test]
    fn all_six_areas_are_accounted_for() {
        let areas = ["uniqueness", "defaults", "retention", "authorization", "idempotency", "consistency"];
        let items = areas.iter().map(|a| item(a, "unresolved", Some("?"), None)).collect();
        let (resolved, open) = bucket_policy_checklist(items).unwrap();
        assert!(resolved.is_empty());
        assert_eq!(open.len(), 6);
    }
}

fn openapi_prompt(
    story: &UserStory,
    spec: &IntentSpec,
    services: &ServicesRegistry,
    adrs: &[Adr],
) -> String {
    let spec_yaml = serde_yaml::to_string(spec).unwrap_or_default();
    let adrs_summary = if adrs.is_empty() {
        "None yet.".to_string()
    } else {
        adrs.iter()
            .map(|a| format!("- {}: {}", a.title, a.decision))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let services_summary = if services.services.is_empty() {
        "None yet.".to_string()
    } else {
        services.services.iter()
            .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
            .map(|s| {
                let tech = s.technology.as_deref().unwrap_or("unknown");
                format!("- {} [{}]: {}", s.name, tech, s.responsibilities.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        r#"You are an API designer generating an OpenAPI Specification (OAS) fragment for a user story.

Story: As a {as_a}, I want {want}, so that {so_that}

Architecture Decisions:
{adrs_summary}

Services:
{services_summary}

Behavioral Specification:
{spec_yaml}

Generate a minimal OAS 3.0 YAML document covering the API endpoints required to implement the BDD scenarios above.

Rules:
- Include only paths directly required by the scenarios
- Use RESTful conventions: POST to create, GET to read, PUT/PATCH to update, DELETE to remove
- Request bodies must include the mandatory and optional fields from entity_schema (if present)
- Response schemas must include the system-generated fields set at creation
- Map validation constraints from entity_schema onto OAS schema properties:
    max_length → maxLength, min_length → minLength, min → minimum, max → maximum, max_items → maxItems
- Include 400 Bad Request response with {{message, fields}} for validation failures
- Include 201 Created with Location header for successful creation
- Use $ref for reusable schemas

Return ONLY valid YAML. No prose. No code fences. No markdown."#,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        adrs_summary = adrs_summary,
        services_summary = services_summary,
        spec_yaml = spec_yaml,
    )
}

pub fn generate_story_openapi(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    services: &ServicesRegistry,
    adrs: &[Adr],
) -> Result<String, LlmError> {
    let raw = client.complete_large(&openapi_prompt(story, spec, services, adrs))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();
    Ok(stripped)
}
