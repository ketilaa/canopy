use canopy_core::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("ANTHROPIC_API_KEY environment variable is not set. Export it before running canopy.")]
    MissingApiKey,
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("Failed to parse JSON from LLM response: {0}")]
    JsonParse(String),
    #[error("Failed to parse YAML from LLM response: {source}\nRaw LLM output:\n{raw}")]
    YamlParse {
        #[source]
        source: serde_yaml::Error,
        raw: String,
    },
    #[error("Unexpected LLM response shape: {0}")]
    UnexpectedShape(String),
}

pub struct LlmClient {
    api_key: String,
    model: String,
    debug: bool,
    provider: LlmProvider,
    base_url: String,
}

impl LlmClient {
    pub fn default_local(debug: bool) -> Self {
        Self {
            api_key: String::new(),
            model: "qwen2.5:32b".to_string(),
            debug,
            provider: LlmProvider::Ollama,
            base_url: "http://localhost:11434".to_string(),
        }
    }

    pub fn from_agent_config(cfg: &AgentLlmConfig, debug: bool) -> Self {
        let base_url = cfg.base_url.clone().unwrap_or_else(|| match cfg.provider {
            LlmProvider::Anthropic => "https://api.anthropic.com".to_string(),
            LlmProvider::Ollama => "http://localhost:11434".to_string(),
        });
        let api_key = match cfg.provider {
            LlmProvider::Anthropic => std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            LlmProvider::Ollama => String::new(),
        };
        Self {
            api_key,
            model: cfg.model.clone(),
            debug,
            provider: cfg.provider.clone(),
            base_url,
        }
    }

    pub fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        self.complete_with_max_tokens(prompt, 4096)
    }

    /// Use for code generation where output can be significantly larger than planning artifacts.
    pub fn complete_large(&self, prompt: &str) -> Result<String, LlmError> {
        self.complete_with_max_tokens(prompt, 8192)
    }

    fn complete_with_max_tokens(&self, prompt: &str, max_tokens: u32) -> Result<String, LlmError> {
        if self.debug {
            eprintln!("\n╔══ LLM INPUT ═══════════════════════════════════════════╗");
            eprintln!("{prompt}");
            eprintln!("╚════════════════════════════════════════════════════════╝\n");
        }

        let (text, json) = match self.provider {
            LlmProvider::Anthropic => self.call_anthropic(prompt, max_tokens)?,
            LlmProvider::Ollama => self.call_openai_compatible(prompt)?,
        };

        if self.debug {
            let model = json["model"].as_str().unwrap_or(&self.model);
            let input_tokens = json["usage"]["input_tokens"]
                .as_u64()
                .or_else(|| json["usage"]["prompt_tokens"].as_u64())
                .unwrap_or(0);
            let output_tokens = json["usage"]["output_tokens"]
                .as_u64()
                .or_else(|| json["usage"]["completion_tokens"].as_u64())
                .unwrap_or(0);
            eprintln!("╔══ LLM OUTPUT ══════════════════════════════════════════╗");
            eprintln!("  model:         {model}");
            eprintln!("  input tokens:  {input_tokens}");
            eprintln!("  output tokens: {output_tokens}");
            eprintln!("──────────────────────────────────────────────────────────");
            eprintln!("{text}");
            eprintln!("╚════════════════════════════════════════════════════════╝\n");
        }

        Ok(text)
    }

    fn call_anthropic(&self, prompt: &str, max_tokens: u32) -> Result<(String, serde_json::Value), LlmError> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": [{"role": "user", "content": prompt}]
        });
        let url = format!("{}/v1/messages", self.base_url);
        let response = ureq::post(&url)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| LlmError::Http(e.to_string()))?;
        let json: serde_json::Value = response
            .into_json()
            .map_err(|e| LlmError::JsonParse(e.to_string()))?;
        let text = json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| LlmError::UnexpectedShape(
                format!("expected content[0].text, got: {json}")
            ))?
            .to_string();
        Ok((text, json))
    }

    fn call_openai_compatible(&self, prompt: &str) -> Result<(String, serde_json::Value), LlmError> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}]
        });
        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = ureq::post(&url)
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| LlmError::Http(e.to_string()))?;
        let json: serde_json::Value = response
            .into_json()
            .map_err(|e| LlmError::JsonParse(e.to_string()))?;
        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| LlmError::UnexpectedShape(
                format!("expected choices[0].message.content, got: {json}")
            ))?
            .to_string();
        Ok((text, json))
    }
}

fn stories_from_intent_prompt(
    intent: &str,
    context: &str,
    existing_stories: &UserStories,
    roles: &RolesRegistry,
) -> String {
    let existing_ids: Vec<&str> = existing_stories.stories.iter().map(|s| s.id.as_str()).collect();
    let existing_ids_str = if existing_ids.is_empty() {
        "none".to_string()
    } else {
        existing_ids.join(", ")
    };
    let existing_roles = if roles.roles.is_empty() {
        "none yet".to_string()
    } else {
        roles.roles.iter()
            .map(|r| match r.description() {
                Some(d) => format!("{} — {}", r.name(), d),
                None => r.name().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        r#"You are an experienced product strategist decomposing a behavioral requirement into user stories.

Business context:
{context}

Behavioral intent:
{intent}

Existing story IDs (do not reuse these): {existing_ids_str}
Known roles: {existing_roles}

Derive the minimal set of user stories that fully cover this intent. Rules:
- Assign each story a short stable ID: use a lowercase domain-area prefix and zero-padded number (e.g. account-001, document-003)
- Choose the prefix from the domain area the story belongs to, not from the intent wording
- The next ID number must be higher than any existing ID with the same prefix
- Reuse a known role if it fits; introduce a new role only when genuinely needed
- Use DDD and domain language in the "want" field — prefer domain verbs (register, activate, publish,
  assign, approve, close) over CRUD verbs (add, create, update, delete)
- "want" must describe a capability, not a location or component — do not name services, bounded
  contexts, or architectural components (avoid: "in the catalog", "via the API", "in the registry")
- "so_that" must state a single concrete business or user benefit — one idea, no "and", no chained thoughts
- A creation story includes all actor-provided attributes — mandatory and optional. Split into an update story only when the intent explicitly describes editing an existing record.
- One intent action = one story. Do not decompose a single action into sub-steps.
- "depends_on" lists IDs of stories (existing or new in this batch) that must exist first
- Reason explicitly about dependencies within this batch: if story B requires story A to exist
  first (because it operates on something A creates), then B must list A in depends_on
- A story that creates a resource has no depends_on; a story that reads, updates, or deletes
  that resource depends on the story that creates it
- Do not duplicate existing stories
- status must be "draft"

Return ONLY valid YAML. No explanation. No code fences. No markdown.

stories:
  - id: <area-NNN>
    as_a: <role>
    want: <capability>
    so_that: <concrete benefit>
    depends_on: []
    status: draft"#,
        context = context,
        intent = intent,
        existing_ids_str = existing_ids_str,
        existing_roles = existing_roles,
    )
}

pub fn generate_stories_from_intent(
    client: &LlmClient,
    intent: &str,
    context: &str,
    existing_stories: &UserStories,
    roles: &RolesRegistry,
) -> Result<UserStories, LlmError> {
    let raw = client.complete_large(&stories_from_intent_prompt(
        intent, context, existing_stories, roles,
    ))?;
    serde_yaml::from_str::<UserStories>(&raw)
        .map_err(|source| LlmError::YamlParse { source, raw })
}

fn domain_extraction_prompt(stories: &[UserStory]) -> String {
    let stories_text = stories
        .iter()
        .map(|s| format!("- {}", s.want))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"You are identifying domain vocabulary from a set of story want-statements.

Wants (what the actor directly operates on):
{stories_text}

Extract only domain objects that are directly created, read, updated, or deleted by these actions.
Do NOT extract actors, beneficiaries, or concepts only implied by purpose or benefit.
Use DDD vocabulary.

Entities: the core business objects — Aggregates, Entities, or Value Objects in the domain model.
  Use PascalCase singular nouns (Account, Document, Booking, Address, Payment).
  Include only real-world domain concepts — things that exist in the business domain.
  Never include: service names (ProductRegistry, CatalogService), infrastructure (Database, EventBus),
  UI concepts (Form, Page), or technical constructs. If it ends in "Service", "Registry",
  "Repository", "Manager", or "Handler" it is not a domain entity.
  Prefer domain language over CRUD language: "Order" not "OrderRecord", "Product" not "ProductItem".

Events: things that happened to a specific entity, named in past tense.
  Naming rule — strictly enforced: every event name MUST start with the name of the entity it belongs to.

  Two kinds of events only:
  1. Lifecycle events — created, updated, deleted:
       InvoiceCreated, InvoiceUpdated, InvoiceDeleted
     Any field-level change is just {{Entity}}Updated — do NOT create a separate event per field.
  2. Business operation events — meaningful state transitions or domain actions:
       AccountActivated, AccountDeactivated, DocumentPublished, AppointmentScheduled, AppointmentCancelled
     Only include these when the story describes a named business operation,
     not when it describes editing or populating data.

  Extract only events directly implied by an operation described in the want statements.
  One event per operation described — do not add anticipatory events.
  If a story describes registering/creating: extract only the Created event.
  If a story describes updating/editing: extract only the Updated event.
  Never add Updated just because Created is present, or vice versa.

Return ONLY valid YAML — no prose, no code fences:

entities:
  - <EntityName>
events:
  - <EventName>
"#,
        stories_text = stories_text,
    )
}

pub fn extract_domain_from_stories(
    client: &LlmClient,
    stories: &[UserStory],
) -> Result<DomainRegistry, LlmError> {
    if stories.is_empty() {
        return Ok(DomainRegistry::default());
    }
    let raw = client.complete(&domain_extraction_prompt(stories))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_yaml::from_str::<DomainRegistry>(stripped)
        .map_err(|source| LlmError::YamlParse { source, raw: stripped.to_string() })
}

fn domain_bootstrap_prompt(idea: &Idea) -> String {
    format!(
        r#"You are identifying core domain entities implied by a software idea.

Idea: {description}

List the key business entities this system will manage.
Rules:
- PascalCase singular nouns only (User, Account, Document)
- Real-world domain concepts only — things the business deals with
- Never include services, infrastructure, UI components, or technical constructs
- Maximum 10 entities

Return ONLY a JSON array of strings. No explanation. No code fences.
["Entity1", "Entity2"]"#,
        description = idea.description
    )
}

pub fn suggest_domain_entities(client: &LlmClient, idea: &Idea) -> Result<Vec<String>, LlmError> {
    let raw = client.complete(&domain_bootstrap_prompt(idea))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<Vec<String>>(stripped)
        .map_err(|e| LlmError::JsonParse(format!("{e}. Raw was: {raw}")))
}

fn roles_bootstrap_prompt(idea: &Idea) -> String {
    format!(
        r#"You are identifying user roles implied by a software idea.

Idea: {description}

List the key roles of people who will interact with this system.
Rules:
- Lowercase noun phrases only (administrator, store manager, warehouse operator)
- Human actors only — not systems, services, or technical components
- Specific named roles that reflect actual domain responsibilities — never generic terms like "user" or "end user"
- Maximum 6 roles

Examples of good roles: administrator, case manager, field operator, analyst, reviewer
Return ONLY a JSON array of strings. No explanation. No code fences.
["role one", "role two"]"#,
        description = idea.description
    )
}

pub fn suggest_roles(client: &LlmClient, idea: &Idea) -> Result<Vec<String>, LlmError> {
    let raw = client.complete(&roles_bootstrap_prompt(idea))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<Vec<String>>(stripped)
        .map_err(|e| LlmError::JsonParse(format!("{e}. Raw was: {raw}")))
}

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
   - Event infrastructure: if the story involves publishing or subscribing to events,
     what event broker/bus is used? Propose it if not yet decided.

Naming rules — strictly enforced:
- Service, frontend, and infrastructure component names: kebab-case only (user-service, booking-service, admin-portal, client-portal, redpanda, postgresql)
  Never use PascalCase or camelCase for component names.
  Never append "Service", "DB", or "Database" as a suffix to service names.
- Domain event names: PascalCase past tense, prefixed with the entity name (InvoiceCreated, AppointmentScheduled, AccountActivated)
  Never use kebab-case for event names.

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

Field format: name (camelCase), type (uuid | string | integer | decimal | boolean | datetime),
description (one sentence).

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
  mandatory:
    - name: "<camelCase>"
      type: "<type>"
      description: "<one sentence>"
  optional:
    - name: "<camelCase>"
      type: "<type>"
      description: "<one sentence>"
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

fn contract_prompt(
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
        r#"You are an API contract designer generating an OpenAPI Specification (OAS) fragment for a user story.

Story: As a {as_a}, I want {want}, so that {so_that}

Architecture Decisions:
{adrs_summary}

Services:
{services_summary}

Behavioral Specification:
{spec_yaml}

Generate a minimal OAS 3.0 YAML contract covering the API endpoints required to implement the BDD scenarios above.

Rules:
- Include only paths directly required by the scenarios
- Use RESTful conventions: POST to create, GET to read, PUT/PATCH to update, DELETE to remove
- Request bodies must include the mandatory and optional fields from entity_schema (if present)
- Response schemas must include the system-generated fields set at creation
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

pub fn generate_story_contract(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    services: &ServicesRegistry,
    adrs: &[Adr],
) -> Result<String, LlmError> {
    let raw = client.complete_large(&contract_prompt(story, spec, services, adrs))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();
    Ok(stripped)
}

/// Quote bare YAML scalar values that contain colons — a common LLM mistake that
/// causes serde_yaml to treat them as nested mappings.
/// Remove duplicate keys within each YAML mapping block.
/// When the LLM emits `operation: create\noperation: modify`, keep the last occurrence.
/// Fix missing indentation inside YAML list items.
/// Some models emit:
///   - id: "1"
///   service: foo      ← should be indented 2 spaces
/// This function detects fields that should be part of the current list item and
/// re-indents them.
fn fix_yaml_list_indentation(yaml: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_item = false;
    let mut item_indent = 0usize;

    for line in yaml.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        if trimmed.starts_with("- ") || trimmed == "-" {
            in_item = true;
            item_indent = indent;
            out.push(line.to_string());
        } else if in_item && !trimmed.is_empty() && indent == item_indent && !trimmed.starts_with('#') {
            // At the same column as the `- ` marker but without one — belongs to the item
            out.push(format!("{}  {}", " ".repeat(item_indent), trimmed));
        } else {
            // Leaving the list item if we hit a non-empty, non-indented line that isn't a new item
            if !trimmed.is_empty() && indent <= item_indent && !trimmed.starts_with('-') {
                in_item = false;
            }
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

fn dedup_yaml_keys(yaml: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    // Track (indent_len, key) pairs seen since the last list-item marker.
    let mut seen: Vec<(usize, &str)> = Vec::new();

    for line in yaml.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        // A new list item resets the seen set for this indent level and deeper.
        if trimmed.starts_with("- ") || trimmed == "-" {
            seen.retain(|(d, _)| *d < indent);
        }

        // Detect a plain mapping key: `key: ...` (no leading `-`).
        if !trimmed.starts_with('-') {
            if let Some(colon) = trimmed.find(": ").or_else(|| trimmed.ends_with(':').then_some(trimmed.len() - 1)) {
                let key = &trimmed[..colon];
                if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                    // If already seen at this indent, skip this line (keep the last occurrence
                    // by retroactively removing the earlier one).
                    if let Some(pos) = seen.iter().position(|(d, k)| *d == indent && *k == key) {
                        seen.remove(pos);
                        // Remove the earlier line from out.
                        if let Some(prev) = out.iter().rposition(|l: &&str| {
                            let t = l.trim_start();
                            let id = l.len() - t.len();
                            id == indent && (t.starts_with(&format!("{key}: ")) || t == &format!("{key}:"))
                        }) {
                            out.remove(prev);
                        }
                    }
                    seen.push((indent, key));
                }
            }
        }
        out.push(line);
    }
    out.join("\n")
}

fn fix_yaml_colon_in_scalars(yaml: &str) -> String {
    yaml.lines().map(|line| {
        // Match lines of the form: <indent><key>: <value> where value is unquoted and contains ':'
        if let Some(colon_pos) = line.find(": ") {
            let (key_part, rest) = line.split_at(colon_pos + 2);
            let value = rest.trim_end();
            // type: [string] — LLM uses bracket notation for array types but YAML parses
            // it as an inline sequence. Quote any unquoted bracket-enclosed type annotation.
            if value.starts_with('[') && value.ends_with(']')
                && !value.starts_with("[\n")
                && !value[1..value.len()-1].contains(", ")
            {
                return format!("{}\"{}\"", key_part, value);
            }
            // Only fix plain (unquoted) scalars that contain a colon
            if !value.is_empty()
                && !value.starts_with('"')
                && !value.starts_with('\'')
                && !value.starts_with('{')
                && !value.starts_with('[')
                && !value.starts_with('|')
                && !value.starts_with('>')
                && value.contains(':')
            {
                let escaped = value.replace('"', "\\\"");
                return format!("{}\"{}\"", key_part, escaped);
            }
        }
        line.to_string()
    }).collect::<Vec<_>>().join("\n")
}

fn is_jvm_technology(tech: &str) -> bool {
    let t = tech.to_lowercase();
    t.contains("spring") || t.contains("java") || t.contains("kotlin")
        || t.contains("maven") || t.contains("gradle") || t.contains("micronaut")
        || t.contains("quarkus")
}

fn infer_working_dir(technology: &str) -> &'static str {
    let t = technology.to_lowercase();
    if t.contains("angular") || t.contains("react") || t.contains("vue")
        || t.contains("next") || t.contains("vite") || t.contains("svelte")
        || t.contains("nuxt")
    {
        "frontend"
    } else {
        "services"
    }
}

pub fn services_need_jvm(services: &ServicesRegistry) -> bool {
    services
        .services
        .iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
        .any(|s| s.technology.as_deref().map(is_jvm_technology).unwrap_or(false))
}

// ── Tech-stack skills ────────────────────────────────────────────────────────
// Each skill is a rules block injected into the per-service plan prompt.
// JVM skills receive dynamic package/path context; others are static strings.
// Add new skills here; the matcher in `skill_for_technology` selects the right one.

fn spring_boot_skill(pkg: &str, pkg_path: &str, service_name: &str) -> String {
    format!(
        "Tech stack rules — Spring Boot 3 (Jakarta EE):\n\
         - Build file:    services/{service_name}/pom.xml  ← this exact path, never just pom.xml\n\
         - Base package:  {pkg}  ← use this exact string in every `package` declaration\n\
         - Source root:   services/{service_name}/src/main/java/{pkg_path}/\n\
         - Test root:     services/{service_name}/src/test/java/{pkg_path}/\n\
         - Example paths: services/{service_name}/src/main/java/{pkg_path}/domain/Product.java\n\
                          services/{service_name}/src/main/java/{pkg_path}/controller/ProductController.java\n\
                          services/{service_name}/src/test/java/{pkg_path}/ProductControllerIT.java\n\
         - Sub-packages:  {pkg}.domain   {pkg}.repository\n\
                          {pkg}.dto      {pkg}.service   {pkg}.controller\n\
         - Every .java file's package declaration MUST be exactly '{pkg}' or one of its sub-packages\n\
         - If an existing file has any other package declaration, correct it to match\n\
         - @SpringBootApplication class MUST be at services/{service_name}/src/main/java/{pkg_path}/*Application.java\n\
           (directly in the base package, never inside a sub-package like service/ or controller/)\n\
         - Namespace: jakarta.* everywhere — NEVER import javax.*\n\
         - One public type per .java file; file name MUST match the class name exactly\n\
         - pom.xml MUST include: spring-boot-starter-data-jpa, spring-boot-starter-validation,\n\
           postgresql (runtime scope), lombok\n\
         - Validation: jakarta.validation.constraints.* (@NotBlank, @NotNull, @Positive, etc.)\n\
         - Integration tests: @SpringBootTest + @AutoConfigureMockMvc",
        pkg = pkg,
        pkg_path = pkg_path,
        service_name = service_name,
    )
}

const SKILL_REACT_VITE: &str = "\
Tech stack rules — React + TypeScript (Vite scaffold):
- File paths in steps are relative to the PROJECT ROOT — always include the full prefix,
  e.g. frontend/admin-portal/src/api/ProductApi.ts
- ALL .ts and .tsx files MUST live under the service's src/ directory
- Minimal layout for a story: one API client, one form component, one App.tsx update — nothing more
- API client: <prefix>/src/api/<Entity>Api.ts — typed fetch(), request and response interfaces inline
- Form component: <prefix>/src/components/<Entity>Form.tsx — controlled inputs, validation, error display
- Wire up: modify <prefix>/src/App.tsx to render the form component
- Import paths inside source files are relative to the file's position — never use ../..
- STRICT SCOPE — do NOT add: custom hooks, page components, route files, store/redux slices,
  utility/validator modules, CSS files, or any abstraction not required by the story.
  The form component handles its own state and calls the API client directly.";

const SKILL_ANGULAR: &str = "\
Tech stack rules — Angular:
- File paths in steps are relative to the PROJECT ROOT — always include the full prefix
- Source root is src/app/ inside the service directory
- Feature folder per domain concept: module, component, service, and model in one folder
- Services: @Injectable({ providedIn: 'root' }) unless feature-scoped
- HTTP: inject HttpClient — never call fetch() directly
- Prefer reactive forms (FormBuilder) over template-driven forms for non-trivial inputs
- Typed HTTP responses: use generics on HttpClient methods";

const SKILL_NODE_EXPRESS: &str = "\
Tech stack rules — Node.js / Express (TypeScript):
- File paths in steps are relative to the PROJECT ROOT — always include the full prefix
- Source root is src/ inside the service directory
- Layout: src/routes/ for Express routers, src/services/ for business logic,
  src/models/ for type interfaces, src/middleware/ for cross-cutting concerns
- Use async/await throughout — no raw .then() chains in route handlers
- Validate input at the route boundary (e.g. zod or joi schema)
- Central error-handling middleware in src/middleware/errorHandler.ts";

/// Build the skill block for the given technology, injecting dynamic package context for JVM.
/// Returns an empty string if no built-in skill matches (LLM gets no extra rules).
pub fn skill_for_technology(tech: &str, pkg: &str, pkg_path: &str, service_name: &str) -> String {
    let t = tech.to_lowercase();
    if t.contains("spring") || t.contains("quarkus") || t.contains("micronaut")
        || (t.contains("java") && !t.contains("javascript"))
        || t.contains("kotlin")
    {
        spring_boot_skill(pkg, pkg_path, service_name)
    } else if t.contains("react") || t.contains("vite") {
        SKILL_REACT_VITE.to_string()
    } else if t.contains("angular") {
        SKILL_ANGULAR.to_string()
    } else if t.contains("node") || t.contains("express") || t.contains("nest") {
        SKILL_NODE_EXPRESS.to_string()
    } else {
        String::new()
    }
}

fn plan_prompt_for_service(
    service: &ServiceEntry,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    adrs: &[Adr],
    existing_files: &[String],
    service_packages: &std::collections::HashMap<String, String>,
) -> String {
    let tech = service.technology.as_deref().unwrap_or("unknown");
    let is_front = service.component_type.as_deref() == Some("frontend");

    let (pkg, pkg_path) = if is_front {
        (String::new(), String::new())
    } else if let Some(detected) = service_packages.get(&service.name) {
        (detected.clone(), detected.replace('.', "/"))
    } else {
        // Scaffold not found — fall back to Spring Initializr convention (hyphen → underscore)
        let p = service.name.replace('-', "_");
        eprintln!("Warning: no scaffolded package detected for '{}'; using fallback '{}'", service.name, p);
        (p.clone(), p.replace('.', "/"))
    };

    let skill = skill_for_technology(tech, &pkg, &pkg_path, &service.name);

    // For frontend the skill already states the path convention; show the prefix explicitly here too.
    let location_line = if is_front {
        format!("Service directory prefix: frontend/{}/  (all file paths in steps start with this)",
            service.name)
    } else {
        String::new() // covered in full detail by the Spring Boot skill
    };

    let schema_yaml = spec.entity_schema.as_ref()
        .map(|s| serde_yaml::to_string(s).unwrap_or_default())
        .unwrap_or_default();
    let scenarios_yaml = serde_yaml::to_string(&spec.scenarios).unwrap_or_default();

    let adrs_summary: String = adrs.iter()
        .map(|a| format!("  - {}: {}", a.title, a.decision))
        .collect::<Vec<_>>()
        .join("\n");

    // Only show files that belong to this service
    let service_prefix = if is_front {
        format!("frontend/{}/", service.name)
    } else {
        format!("services/{}/", service.name)
    };
    let service_existing: Vec<&str> = existing_files.iter()
        .filter(|f| f.starts_with(&service_prefix))
        .map(|f| f.as_str())
        .collect();

    let existing_note = if service_existing.is_empty() {
        String::new()
    } else {
        format!(
            "\nExisting files — use operation: modify for these:\n{}",
            service_existing.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join("\n")
        )
    };

    let skill_section = if skill.is_empty() {
        String::new()
    } else {
        format!("\n{skill}\n")
    };

    format!(
        "Generate implementation steps for service '{sname}' as part of story '{story_id}'.\n\
         \n\
         Story: As a {as_a}, I want {want}, so that {so_that}.\n\
         \n\
         Service: {sname}  Technology: {tech}\n\
         {location_line}\n\
         {skill_section}\n\
         Entity schema:\n{schema_yaml}\n\
         BDD scenarios:\n{scenarios_yaml}\n\
         OAS Contract:\n{contract_yaml}\n\
         Architecture decisions:\n{adrs_summary}\n\
         {existing_note}\n\
         Return ONLY valid YAML — no prose, no code fences.\n\
         List ONLY files that belong to service '{sname}'.\n\
         Every field inside a list item MUST be indented by exactly 2 spaces.\n\
         \n\
         steps:\n\
         - id: \"1\"\n\
           service: {sname}\n\
           file: <path/relative/to/project/root>\n\
           operation: create\n\
           description: <specific description of what this file contains>\n\
         \n\
         Rules:\n\
         - `operation` is create for new files, modify for files in the existing list above\n\
         - One step per file — no duplicates\n\
         - Order: build config → domain → data layer → service → API → tests\n\
         - description must name the specific classes, fields, and annotations\n\
         - ALL string values must be quoted with double quotes — every id, service, file, operation, and description\n\
         - Never use block scalars (>- or |) — always use a single quoted string on one line\n\
         - STRICT SCOPE: only include files that are directly required to implement this story.\n\
           Do NOT include: README, HELP.md, .gitignore, CSS files, config files (tsconfig, vite.config),\n\
           scaffolding artifacts, or any file that does not contain logic for this story.\n\
           If in doubt, leave it out.\n",
        sname = service.name,
        story_id = story.id,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        tech = tech,
        location_line = location_line,
        skill_section = skill_section,
        schema_yaml = schema_yaml,
        scenarios_yaml = scenarios_yaml,
        contract_yaml = contract_yaml,
        adrs_summary = adrs_summary,
        existing_note = existing_note,
    )
}

/// Merge orphaned continuation lines back into the preceding quoted scalar.
///
/// The model occasionally breaks a long quoted description across two lines:
///   description: "fields: name, categories,"
///     images, price
/// The second line is not a valid YAML key, causing a parse error.
/// We detect it (non-empty, no colon, not a list item, follows a closing `"`) and
/// re-attach it before the closing quote of the previous line.
fn fix_broken_quoted_continuations(yaml: &str) -> String {
    let lines: Vec<&str> = yaml.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        // Peek ahead: if next line looks like an orphaned continuation, absorb it.
        if i + 1 < lines.len() && line.ends_with('"') && line.contains(": \"") {
            let next = lines[i + 1].trim();
            let is_continuation = !next.is_empty()
                && !next.starts_with('-')
                && !next.contains(": ")
                && !next.ends_with(':');
            if is_continuation {
                // Insert continuation text before the closing quote.
                let merged = format!("{} {}", &line[..line.len() - 1], next);
                result.push(merged);
                i += 2;
                continue;
            }
        }
        result.push(line.to_string());
        i += 1;
    }
    result.join("\n")
}

fn parse_plan_steps(raw: &str) -> Result<Vec<ImplementationStep>, LlmError> {
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let fixed = dedup_yaml_keys(&fix_yaml_colon_in_scalars(&fix_yaml_list_indentation(&fix_broken_quoted_continuations(stripped))));
    #[derive(serde::Deserialize)]
    struct PlanResponse { steps: Vec<ImplementationStep> }
    let parsed: PlanResponse = serde_yaml::from_str(&fixed)
        .map_err(|source| LlmError::YamlParse { source, raw: fixed })?;
    Ok(parsed.steps)
}

fn layer_weight(file: &str) -> u8 {
    let f = file.to_lowercase();
    if f.ends_with("pom.xml") || f.ends_with("package.json") || f.ends_with("build.gradle") { 0 }
    else if f.contains("/domain/") || f.contains("entity") { 1 }
    else if f.contains("/repository/") { 2 }
    else if f.contains("/dto/") || f.contains("request") || f.contains("response") { 3 }
    else if f.contains("/service/") { 4 }
    else if f.contains("/controller/") || f.contains("/api/") { 5 }
    else if f.contains("test") || f.contains("spec") { 6 }
    else { 3 }
}

pub fn generate_story_plan(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    services: &ServicesRegistry,
    adrs: &[Adr],
    existing_files: &[String],
    service_packages: &std::collections::HashMap<String, String>,
) -> Result<StoryPlan, LlmError> {
    let active: Vec<&ServiceEntry> = services.services.iter()
        .filter(|s| s.component_type.as_deref() != Some("infrastructure"))
        .collect();

    let mut all_steps: Vec<ImplementationStep> = Vec::new();
    for service in &active {
        let prompt = plan_prompt_for_service(
            service, story, spec, contract_yaml, adrs, existing_files, service_packages,
        );
        let raw = client.complete_large(&prompt)?;
        let mut steps = parse_plan_steps(&raw)?;
        for step in &mut steps {
            step.service = service.name.clone();
            // Normalise operation: any value that isn't exactly "modify" becomes "create"
            if step.operation.to_lowercase() != "modify" {
                step.operation = "create".to_string();
            } else {
                step.operation = "modify".to_string();
            }
        }
        all_steps.extend(steps);
    }

    // Sort: backend services first, frontend last; within each group by architectural layer
    let is_frontend_service = |name: &str| {
        services.services.iter()
            .find(|s| s.name == name)
            .and_then(|s| s.component_type.as_deref())
            .map(|t| t == "frontend")
            .unwrap_or(false)
    };
    all_steps.sort_by_key(|s| {
        let tier = if is_frontend_service(&s.service) { 1u8 } else { 0u8 };
        (tier, layer_weight(&s.file))
    });

    for (i, step) in all_steps.iter_mut().enumerate() {
        step.id = (i + 1).to_string();
    }

    Ok(StoryPlan { story_id: story.id.clone(), steps: all_steps })
}

fn step_prompt(
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
) -> String {
    let schema_yaml = spec.entity_schema.as_ref()
        .map(|s| serde_yaml::to_string(s).unwrap_or_default())
        .unwrap_or_default();

    // The plan LLM sometimes prefixes service names with their directory (e.g. "frontend/admin-portal").
    // Strip any leading path component before looking up in the registry.
    let service_name = step.service.rsplit('/').next().unwrap_or(&step.service);
    let service_entry = services.services.iter()
        .find(|s| s.name == service_name || s.name == step.service);
    let technology = service_entry.and_then(|s| s.technology.as_deref()).unwrap_or("unknown");
    // Detect frontend by registry entry OR by file extension (belt-and-suspenders).
    let is_frontend = service_entry
        .and_then(|s| s.component_type.as_deref())
        .map(|t| t == "frontend")
        .unwrap_or(false)
        || step.file.ends_with(".ts")
        || step.file.ends_with(".tsx");

    let pkg = service_packages.get(service_name)
        .cloned()
        .unwrap_or_else(|| service_name.replace('-', "_"));
    let pkg_path = pkg.replace('.', "/");

    let tech_rules = if is_frontend {
        format!(
            "Technology rules (React + TypeScript, Vite scaffold):\n\
             - Source root is src/ — ALL .ts/.tsx files live under src/\n\
             - Layout: src/api/<Entity>Api.ts, src/components/<Entity>Form.tsx, src/App.tsx\n\
             - Import paths are always relative to the file's location inside src/\n\
               e.g. App.tsx imports: import ProductForm from './components/ProductForm'\n\
               e.g. App.tsx imports: import {{ registerProduct }} from './api/ProductApi'\n\
             - Never use '../../' — all project files are siblings or children inside src/\n\
             - This file's location: {file_path}\n\
             - Complete TypeScript — no 'any' types unless unavoidable\n\
             - Use fetch() for HTTP calls — no external HTTP libraries\n\
             - Idiomatic React with hooks (useState, useCallback)\n\
             - Form validation: enforce required fields client-side before submission\n\
             - Show success message after successful operation\n\
             - Show field-level error messages from 400 responses\n\
             - Do not import files that do not exist yet",
            file_path = step.file
        )
    } else {
        format!(
            "Technology rules (Spring Boot, Java, Maven):\n\
             - Base package: {pkg}  ← use this exact string in every package declaration\n\
             - Source root:  src/main/java/{pkg_path}/\n\
             - Test root:    src/test/java/{pkg_path}/\n\
             - Sub-packages: {pkg}.domain  {pkg}.repository  {pkg}.service  {pkg}.controller\n\
             - Every file's package declaration must be exactly '{pkg}' or a sub-package of it\n\
             - If the current file contains a different package declaration, correct it to match the above\n\
             - Complete, compilable Java — no stubs, no TODO placeholders — include all imports\n\
             - NAMESPACE: Spring Boot 3+ uses jakarta.* — NEVER use javax.*\n\
               Use jakarta.validation.*, jakarta.persistence.*, jakarta.servlet.*, jakarta.annotation.*\n\
             - REST endpoints must match OAS contract paths and HTTP methods exactly\n\
             - 201 Created with Location header on successful creation\n\
             - 400 Bad Request with {{message: String, fields: List<String>}} on validation failure\n\
             - Use Hibernate Validator annotations (@NotBlank, @NotNull, @Min) on request DTOs\n\
             - Publish domain events via ApplicationEventPublisher\n\
             - For pom.xml modifications: preserve all existing content, only add missing dependencies\n\
             - Required Spring Boot dependencies if missing: spring-boot-starter-data-jpa, \
               spring-boot-starter-validation, h2 (test scope), lombok"
        )
    };

    let current_section = match current_content {
        Some(content) => format!(
            "\nCurrent file content (modify operation — preserve what stays, change what the description requires):\n\
             ```\n{content}\n```\n"
        ),
        None => String::new(),
    };

    let roots_section = match roots_context {
        Some(ctx) if !ctx.is_empty() => format!(
            "\nRelated code already in the project (use these exact class names and package paths):\n{ctx}\n"
        ),
        _ => String::new(),
    };

    format!(
        "Generate the complete content of file '{file}'.\n\
         \n\
         Operation: {operation}\n\
         Description: {description}\n\
         \n\
         Story: As a {as_a}, I want {want}, so that {so_that}.\n\
         Service: {service} ({technology})\n\
         \n\
         Entity schema:\n\
         {schema_yaml}\n\
         OAS Contract:\n\
         {contract_yaml}\n\
         {current_section}\
         {roots_section}\n\
         {tech_rules}\n\
         \n\
         Return ONLY the raw file content — no JSON wrapper, no markdown, no code fences, no explanation.",
        file = step.file,
        operation = step.operation,
        description = step.description,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        service = step.service,
        technology = technology,
        schema_yaml = schema_yaml,
        contract_yaml = contract_yaml,
        current_section = current_section,
        roots_section = roots_section,
        tech_rules = tech_rules,
    )
}

pub fn execute_implementation_step(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
) -> Result<String, LlmError> {
    let prompt = step_prompt(story, spec, contract_yaml, step, current_content, roots_context, service_packages, services);
    let raw = client.complete_large(&prompt)?;
    let trimmed = raw.trim();
    let after_open = trimmed
        .trim_start_matches("```java")
        .trim_start_matches("```typescript")
        .trim_start_matches("```tsx")
        .trim_start_matches("```ts")
        .trim_start_matches("```xml")
        .trim_start_matches("```yaml")
        .trim_start_matches("```properties")
        .trim_start_matches("```")
        .trim_start();
    let content = if let Some(pos) = after_open.rfind("\n```") {
        &after_open[..pos]
    } else {
        after_open.trim_end_matches("```").trim_end()
    };
    Ok(content.to_string())
}

fn fix_prompt(file_path: &str, content: &str, errors: &str, existing_files: &[String]) -> String {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let lang = match ext {
        "java" => "Java",
        "ts" | "tsx" => "TypeScript",
        "xml" => "XML",
        _ => "source",
    };
    let extra_rules = if ext == "java" {
        "\n- A Java source file contains exactly one top-level type declaration\n\
         - Nothing may appear after the final closing brace of the top-level class/interface/enum/record\n\
         - Remove any stray import statements, package declarations, or class bodies that appear after that brace\n\
         - The file must begin with the package declaration"
    } else if file_path.ends_with("pom.xml") {
        "\n- Add the required <dependency> blocks inside <dependencies>\n\
         - Use the correct groupId/artifactId/version for each missing package\n\
         - For javax.validation use jakarta.validation-api or spring-boot-starter-validation\n\
         - For javax.persistence use jakarta.persistence-api or spring-boot-starter-data-jpa\n\
         - Do not remove any existing dependencies\n\
         - Keep the XML well-formed"
    } else {
        ""
    };
    let files_section = if !existing_files.is_empty() {
        format!(
            "\nExisting files in the project (use for correct import paths):\n{}\n",
            existing_files.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join("\n")
        )
    } else {
        String::new()
    };
    format!(
        "Fix the {lang} file below so that all listed errors are resolved.\n\
         \n\
         File: {file_path}\n\
         \n\
         Errors:\n\
         {errors}\n\
         {files_section}\n\
         Current content:\n\
         {content}\n\
         \n\
         Rules:\n\
         - Return ONLY the corrected file content — no prose, no markdown fences, no explanations\
         {extra_rules}\n\
         - Preserve all correct logic; only fix what the errors report\n\
         - Do not add TODO comments or placeholder stubs\n\
         - Only import from modules that exist in the project files listed above"
    )
}

pub fn fix_file(
    client: &LlmClient,
    file_path: &str,
    content: &str,
    errors: &str,
    existing_files: &[String],
) -> Result<String, LlmError> {
    let raw = client.complete_large(&fix_prompt(file_path, content, errors, existing_files))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```java")
        .trim_start_matches("```typescript")
        .trim_start_matches("```xml")
        .trim_start_matches("```")
        .trim();
    if let Some(end) = stripped.rfind("\n```") {
        Ok(stripped[..end].trim_end().to_string())
    } else {
        Ok(stripped.to_string())
    }
}

pub fn generate_scaffold_from_services(services: &ServicesRegistry, group_id: &str) -> ScaffoldPlan {
    let mut commands = Vec::new();
    for service in &services.services {
        if service.component_type.as_deref() == Some("infrastructure") {
            eprintln!(
                "  (skipping '{}': infrastructure component — managed via docker-compose or similar)",
                service.name
            );
            continue;
        }
        if let Some(ref tech) = service.technology {
            let working_dir = service
                .component_type
                .as_deref()
                .map(|ct| if ct == "frontend" { "frontend" } else { "services" })
                .unwrap_or_else(|| infer_working_dir(tech));
            match technology_to_command(&service.name, tech, group_id, working_dir) {
                Some(cmd) => commands.push(cmd),
                None => eprintln!(
                    "  (skipping '{}': no scaffold template for '{}')",
                    service.name, tech
                ),
            }
        } else {
            eprintln!(
                "  (skipping '{}': no technology decided — run `canopy spec` to resolve tech stack ADRs)",
                service.name
            );
        }
    }
    ScaffoldPlan { generated_at: String::new(), commands }
}

fn technology_to_command(
    name: &str,
    technology: &str,
    group_id: &str,
    working_dir: &str,
) -> Option<ScaffoldCommand> {
    let t = technology.to_lowercase();
    let artifact_id = name.to_lowercase().replace(' ', "-");

    let (command, creates) = if t.contains("next.js") || t.contains("nextjs") {
        (
            format!("npx create-next-app@latest {name} --typescript --tailwind --app --no-git"),
            format!("{name}/"),
        )
    } else if t.contains("angular") {
        (
            format!("npx @angular/cli@latest new {name} --directory={name} --style=css --routing --skip-git --no-interactive"),
            format!("{name}/"),
        )
    } else if t.contains("vite")
        || t.contains("react")
        || t.contains("vue")
        || t.contains("svelte")
        || t.contains("solid")
        || t.contains("preact")
        || t.contains("lit")
    {
        let template = vite_template_for(&t);
        (
            format!("printf 'n\\n' | npm create vite@latest {name} -- --template {template}"),
            format!("{name}/"),
        )
    } else if t.contains("spring boot") || t.contains("spring-boot") {
        let (lang, proj_type) = if t.contains("kotlin") {
            ("kotlin", "gradle-project")
        } else {
            ("java", "maven-project")
        };
        (
            format!(
                "mkdir -p {artifact_id} && curl -G https://start.spring.io/starter.tgz \\\n  -d dependencies=web,actuator -d language={lang} -d type={proj_type} \\\n  -d bootVersion=4.1.0 \\\n  -d groupId={group_id} -d artifactId={artifact_id} -d name={artifact_id} \\\n  | tar -xzvf - -C {artifact_id}"
            ),
            format!("{artifact_id}/"),
        )
    } else if t.contains("node") || t.contains("express") || t.contains("fastify")
        || t.contains("koa") || t.contains("hapi")
    {
        (
            format!("mkdir -p {name} && cd {name} && npm init -y && npm install express && touch index.js"),
            format!("{name}/"),
        )
    } else if t.contains("python") || t.contains("django") || t.contains("flask") || t.contains("fastapi") {
        (
            format!("mkdir -p {name} && touch {name}/main.py {name}/requirements.txt"),
            format!("{name}/"),
        )
    } else if t.contains("rust") || t.contains("axum") || t.contains("actix") || t.contains("rocket") {
        (
            format!("cargo new {name}"),
            format!("{name}/"),
        )
    } else if t.contains(".net") || t.contains("dotnet") || t.contains("asp.net") || t.contains("c#") {
        (
            format!("dotnet new webapi -n {name}"),
            format!("{name}/"),
        )
    } else if t.contains("spring") || t.contains("java") || t.contains("maven") {
        (
            format!("mvn archetype:generate -DgroupId={group_id} -DartifactId={artifact_id} -DarchetypeArtifactId=maven-archetype-quickstart -DarchetypeVersion=1.4 -DinteractiveMode=false"),
            format!("{artifact_id}/"),
        )
    } else if t.contains("kotlin") || t.contains("gradle") {
        (
            format!("gradle init --type kotlin-application --dsl kotlin --no-incubating"),
            format!("{name}/"),
        )
    } else {
        return None;
    };

    Some(ScaffoldCommand {
        label: format!("{name} ({technology})"),
        command,
        working_dir: working_dir.to_string(),
        creates,
    })
}

fn vite_template_for(tech_lower: &str) -> &'static str {
    // Always use TypeScript variants — avoids the variant-selection prompt in Vite 8
    // when a plain JS template is specified (which interprets 'n' as cancel).
    if tech_lower.contains("react") && tech_lower.contains("swc") {
        "react-swc-ts"
    } else if tech_lower.contains("react") {
        "react-ts"
    } else if tech_lower.contains("vue") {
        "vue-ts"
    } else if tech_lower.contains("svelte") {
        "svelte-ts"
    } else if tech_lower.contains("solid") {
        "solid-ts"
    } else if tech_lower.contains("preact") {
        "preact-ts"
    } else if tech_lower.contains("lit") {
        "lit-ts"
    } else {
        "vanilla-ts"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_yaml_list_indentation_repairs_missing_indent() {
        let input = "steps:\n- id: \"1\"\nservice: product-service\nfile: foo.java\noperation: create\ndescription: Do something.\n- id: \"2\"\nservice: admin-portal\nfile: bar.tsx\noperation: create\ndescription: Do another thing.";
        let result = fix_yaml_list_indentation(input);
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result).expect("should parse");
        let steps = parsed["steps"].as_sequence().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["service"].as_str().unwrap(), "product-service");
        assert_eq!(steps[1]["service"].as_str().unwrap(), "admin-portal");
    }

    #[test]
    fn dedup_yaml_keys_removes_duplicate_operation() {
        let input = "steps:\n- id: \"8\"\n  service: product-service\n  file: foo.java\n  operation: create\n  operation: modify\n  description: Do something.";
        let result = dedup_yaml_keys(input);
        // Only one `operation:` line should survive, and it should be the last one
        let count = result.lines().filter(|l| l.trim_start().starts_with("operation:")).count();
        assert_eq!(count, 1);
        assert!(result.contains("operation: modify"));
    }

    #[test]
    fn dedup_yaml_keys_leaves_unique_keys_intact() {
        let input = "steps:\n- id: \"1\"\n  service: svc\n  file: a.java\n  operation: create\n  description: Create something.";
        let result = dedup_yaml_keys(input);
        assert_eq!(result, input);
    }
}
