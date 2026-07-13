use crate::client::{LlmClient, LlmError};
use crate::prompts::yaml_util::{parse_lenient_sequence, strip_code_fence};
use crate::repair::fix_yaml_colon_in_scalars;
use canopy_core::*;

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
    format!(
        r#"You are an experienced software architect.

A team is about to write a BDD specification for this user story:
  As a {as_a}, I want {want}, so that {so_that}

Existing Architecture Decisions:
{adrs_summary}

Known Services and Responsibilities:
{services_summary}

SKIP a question entirely if its answer is already captured above. Check precisely before proposing:
- Service ownership: skip if the specific service that should own THIS story's domain is already in Known Services.
- UI/frontend: skip if a frontend that serves THIS actor's interaction for THIS capability is already in Known Services.
  Do NOT skip just because some other frontend exists for a different actor or purpose.
- Tech stack: skip if the specific service already has a decided technology in Known Services.
- Database infrastructure: skip if the specific data-owning service already has a database in Known Services.
  Do NOT skip just because some other service already has a database.
- Event broker infrastructure: skip if an event broker entry already exists in Known Services.
- Domain event ADR: skip only if a domain-event ADR for THIS story's entity and operation already
  exists in Existing Architecture Decisions above — never skip just because the story's own
  wording never uses the word "event."
Propose ONLY questions where the decision is genuinely absent from the above context.
If all decisions are already made, return an empty proposals list.

Include ALL of:
1. Structural questions — service ownership, data responsibility, integration contracts, API boundaries.
   - Domain event ADR — MANDATORY whenever the Architecture Style ADR above is event-driven and
     this story's action creates, updates, or deletes an aggregate. Classify the story's own
     action (want: "{want}") as exactly one of creation | update | deletion | other, then propose
     exactly ONE domain event ADR naming exactly one event: "<Entity>Created" or
     "<Entity>Registered" for creation, "<Entity>Updated" for update, "<Entity>Deleted" for
     deletion.
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
- Domain event names: PascalCase past tense, prefixed with the entity name (InvoiceCreated, AppointmentScheduled, AccountActivated)
  Never use kebab-case for event names.
- Domain event ADR decisions: when a Topic Naming Convention ADR exists in Existing Architecture Decisions,
  derive the topic name from it and format the decision as "<EventName> on topic <topic-name>".
  Example: "ProductCreated on topic product-events"
  The topic name is the aggregate name in kebab-case with an "-events" suffix (product → product-events).
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
    )
}

pub fn identify_architectural_questions(
    client: &LlmClient,
    story: &UserStory,
    existing_adrs: &[Adr],
    services: &ServicesRegistry,
) -> Result<ProposedAdrs, LlmError> {
    let raw = client.complete_large(&architectural_questions_prompt(story, existing_adrs, services))?;
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

Business policy checklist — walk through EACH item below, ONE AT A TIME:
1. Uniqueness — must any field, or combination of fields, be unique across all existing records
   of this entity?
2. Defaults — does any optional field have an implied default value when the actor omits it?
3. Retention — is there a rule for how long this entity persists, or when it expires or archives?
4. Authorization — does creating or modifying this entity require a specific role or permission
   beyond the actor already being authenticated?
5. Idempotency — if the actor submits the same request twice, must that be rejected as a
   duplicate, or is it safely repeatable?
6. Consistency — does creating this entity depend on, or affect, the state of any other entity?

For EACH item, classify it as exactly one of:
- resolved — add an entry to resolved_policies stating the rule as a concrete constraint.
- not applicable — do not output anything for it.
- unresolved — add a concrete question to open_questions. NEVER silently pick an interpretation.

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
resolved_policies:
  - area: "<uniqueness | defaults | retention | authorization | idempotency | consistency>"
    resolution: "<the policy stated as a concrete rule>"
out_of_scope:
  - "<explicitly excluded concern>"
open_questions:
  - "<unresolved question if any>"
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
struct SchemaAndPolicyDraft {
    #[serde(default)]
    entity_schema: Option<EntitySchema>,
    #[serde(default)]
    resolved_policies: Vec<ResolvedPolicy>,
    #[serde(default)]
    out_of_scope: Vec<String>,
    #[serde(default)]
    open_questions: Vec<String>,
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

    let scenarios = if let Some(schema) = &draft.entity_schema {
        let matrix = scenario_coverage_matrix(schema, &draft.resolved_policies);
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
        resolved_policies: draft.resolved_policies,
        out_of_scope: draft.out_of_scope,
        open_questions: draft.open_questions,
    })
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
