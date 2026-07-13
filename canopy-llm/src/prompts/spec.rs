use crate::client::{LlmClient, LlmError};
use crate::repair::fix_yaml_colon_in_scalars;
use canopy_core::*;

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
Propose ONLY questions where the decision is genuinely absent from the above context.
If all decisions are already made, return an empty proposals list.

Include ALL of:
1. Structural questions — service ownership, data responsibility, integration contracts, event design, API boundaries
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
    serde_yaml::from_str::<ProposedAdrs>(stripped)
        .map_err(|source| LlmError::YamlParse { source, raw: stripped.to_string() })
}

fn story_spec_prompt(
    story: &UserStory,
    adrs: &[Adr],
    services: &ServicesRegistry,
    domain: &DomainRegistry,
) -> String {
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
    format!(
        r#"You are a BDD expert writing acceptance criteria for a user story.

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

If this is a creation story, you MUST output an entity_schema section before scenarios.
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

Scenario grounding rule — when entity_schema is present, BDD scenarios MUST be grounded in it:
- "when" MUST explicitly name the mandatory fields the actor submits
- "then" MUST reference at least the system-generated fields set at creation
  (e.g. "the system assigns an id and sets createdAt to the current timestamp")
- Also include a scenario for the missing-mandatory-field failure case

Write BDD scenarios (Given/When/Then) as acceptance criteria. Additional rules:
- Scenarios describe OBSERVABLE BEHAVIOR from the user's perspective — never internal API calls,
  HTTP verbs, JSON payloads, or implementation details
- Given describes actor context and relevant domain state only — never system availability,
  portal health, service status, or infrastructure operations.
  Good: "The product manager is authenticated and no product with this name exists."
  Bad:  "The admin portal is operating normally."
- When describes what the actor does — one action per scenario
- Then describes what the actor observes or what changed in the domain — never service internals,
  technology names, or infrastructure operations.
  Good: "The product is registered and a ProductCreated event is published."
  Bad:  "ProductService stores the data and Redpanda receives the event."
- intent_ref must be exactly: {story_id}
- Scenario IDs must follow the pattern: {story_id}-01, {story_id}-02, etc.

Return ONLY valid YAML — no prose, no code fences.
YAML string rules — you MUST follow these to avoid parse errors:
- Any string value containing a colon (:) MUST be enclosed in double quotes
- Any list item ending with a question mark (?) MUST be enclosed in double quotes
- type values are strings: string, integer, decimal, uuid, datetime, boolean, "[string]", "[uuid]" — always quote bracket forms: type: "[string]" not type: [string]

intent_ref: {story_id}
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
scenarios:
  - id: "{story_id}-01"
    name: "<scenario name>"
    given:
      - "<world state precondition>"
    when: "<user or system action — must name mandatory fields for creation stories>"
    then:
      - "<observable outcome — must reference system-assigned fields for creation stories>"
    constraints:
      - "<constraint or empty list>"
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

pub fn generate_story_spec(
    client: &LlmClient,
    story: &UserStory,
    adrs: &[Adr],
    services: &ServicesRegistry,
    domain: &DomainRegistry,
) -> Result<IntentSpec, LlmError> {
    let raw = client.complete_large(&story_spec_prompt(story, adrs, services, domain))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let fixed = fix_yaml_colon_in_scalars(stripped);
    serde_yaml::from_str::<IntentSpec>(&fixed)
        .map_err(|source| LlmError::YamlParse { source, raw: fixed })
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
