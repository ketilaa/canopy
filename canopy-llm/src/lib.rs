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
// Every tech-stack skill implements the same three-section contract:
//   1. file_layout    — where files live and what each directory means
//   2. namespace_rules — allowed/forbidden imports with examples
//   3. layer_order    — the sequence in which files must be generated (dependency order)
//
// `notes` is optional: fix-loop guidance, scope constraints, etc.
//
// To add a new stack: implement a builder function that returns TechStackSkill,
// fill all three required fields, add a match arm in skill_for_technology().

pub struct TechStackSkill {
    pub name: String,
    /// Where files live; directory conventions; one-type-per-file rules.
    pub file_layout: String,
    /// Allowed and forbidden imports/namespaces with concrete examples.
    pub namespace_rules: String,
    /// Ordered list of layers with rationale — the LLM generates in this sequence.
    pub layer_order: String,
    /// Optional extra rules: scope constraints, fix-loop guidance, etc.
    pub notes: Option<String>,
}

impl TechStackSkill {
    pub fn render(&self) -> String {
        let notes_section = match &self.notes {
            Some(n) if !n.is_empty() => format!("\n\nAdditional rules:\n{n}"),
            _ => String::new(),
        };
        format!(
            "Tech stack — {name}:\n\
             \n\
             File layout:\n{layout}\n\
             \n\
             Namespace / import rules:\n{ns}\n\
             \n\
             Layer order — generate files in this sequence:\n{order}{notes}",
            name  = self.name,
            layout = self.file_layout,
            ns    = self.namespace_rules,
            order = self.layer_order,
            notes = notes_section,
        )
    }
}

fn spring_boot_skill(pkg: &str, pkg_path: &str, service_name: &str) -> TechStackSkill {
    TechStackSkill {
        name: "Spring Boot 3 (Jakarta EE)".to_string(),
        file_layout: format!(
            "  Build file:  services/{sn}/pom.xml\n\
             Source root: services/{sn}/src/main/java/{pp}/\n\
             Test root:   services/{sn}/src/test/java/{pp}/\n\
             Layers:      {p}.domain  {p}.repository  {p}.dto  {p}.service  {p}.controller\n\
             One public type per .java file; file name must match the class name exactly.\n\
             @SpringBootApplication lives in {p} directly — never inside a sub-package.",
            sn = service_name, pp = pkg_path, p = pkg
        ),
        namespace_rules: format!(
            "  jakarta.* everywhere — NEVER import javax.* (will not compile under Jakarta EE 9+)\n\
             - jakarta.servlet.http.HttpServletRequest  (NOT javax.servlet.http.HttpServletRequest)\n\
             - jakarta.validation.constraints.*  (@NotBlank, @NotNull, @Positive, ...)\n\
             - jakarta.persistence.*  (@Entity, @Id, @GeneratedValue, @Column, ...)\n\
             - jakarta.annotation.*  (@PostConstruct, ...)\n\
             Every package declaration must be exactly {p} or a sub-package of it.",
            p = pkg
        ),
        layer_order: format!(
            "  1. services/{sn}/pom.xml     — complete Maven POM; must end with </project>\n\
             2. {pp}/domain/         — @Entity classes with @Id and @GeneratedValue\n\
             3. {pp}/repository/     — JpaRepository interfaces\n\
             4. {pp}/dto/            — request/response classes with validation annotations\n\
             5. {pp}/service/        — @Service business logic\n\
             6. {pp}/controller/     — @RestController endpoints matching OAS contract\n\
             7. src/test/**/*IT.java — @SpringBootTest integration tests (end-to-end only)\n\
                Do NOT plan *Test.java files — the TDD loop generates them automatically.\n\
             Reason: each layer imports from the one above; generate strictly in this order.",
            sn = service_name, pp = pkg_path
        ),
        notes: Some(format!(
            "  pom.xml required starters: spring-boot-starter-web, spring-boot-starter-data-jpa,\n\
             spring-boot-starter-validation, h2 (runtime scope), spring-boot-starter-test (test scope).\n\
             (Maven structure and dependency validity rules are in the Maven build skill below.)\n\
             Integration tests: import DTOs from {p}.dto — never define local classes that shadow them.\n\
             Include all java.util.* and annotation imports. Test only OAS-declared endpoints.\n\
             Validation annotation type-safety rules (violations cause UnexpectedTypeException at runtime):\n\
             - @Positive / @Min / @Max / @DecimalMin / @DecimalMax — ONLY on numeric types\n\
               (int, Integer, long, Long, BigDecimal, Double, etc.)\n\
               NEVER on String, List, Set, Collection, or any other non-numeric type.\n\
             - For a non-null, non-empty collection:  @NotNull + @NotEmpty  (NOT @Positive)\n\
             - For a non-blank string:               @NotBlank  (NOT @NotNull alone)\n\
             - For a non-null object reference:      @NotNull",
            p = pkg
        )),
    }
}

fn react_vite_skill() -> TechStackSkill {
    TechStackSkill {
        name: "React + TypeScript (Vite)".to_string(),
        file_layout:
            "  All .ts/.tsx files live under <service-prefix>/src/\n\
             Canonical layout for one story:\n\
             - <prefix>/src/api/<Entity>Api.ts         — typed fetch() client + interfaces\n\
             - <prefix>/src/components/<Entity>Form.tsx — controlled form component\n\
             - <prefix>/src/App.tsx                    — renders the form\n\
             File paths in plan steps are relative to the PROJECT ROOT;\n\
             always include the full prefix (e.g. frontend/admin-portal/src/api/ProductApi.ts)."
            .to_string(),
        namespace_rules:
            "  Imports are relative to the file's position inside src/:\n\
             - App.tsx:          import ProductForm from './components/ProductForm'\n\
             - ProductForm.tsx:  import { registerProduct } from '../api/ProductApi'\n\
             Never use '../../' — all source files are siblings or children within src/.\n\
             HTTP: use fetch() only — no axios, ky, or any other HTTP library.\n\
             Do not import a file that does not exist yet."
            .to_string(),
        layer_order:
            "  1. src/api/<Entity>Api.ts         — request/response interfaces + fetch function\n\
             2. src/components/<Entity>Form.tsx  — imports from api/; no other new deps\n\
             3. src/App.tsx                      — imports and renders the form component\n\
             4. tests (if any)\n\
             Reason: each file imports from the previous; generating out of order causes type mismatches."
            .to_string(),
        notes: Some(
            "  STRICT SCOPE — do NOT add unless the story explicitly requires it:\n\
             custom hooks, page components, route files, Redux/Zustand slices,\n\
             utility modules, CSS files, or any abstraction not named in the acceptance criteria.\n\
             The form component handles its own state and calls the API client directly.\n\
             Fix-loop — TS2322 on a JSX element means this file passes props the component does not accept.\n\
             Check the referenced files for the component's actual Props type.\n\
             React.FC or React.FC<{}> with no type parameter accepts NO props.\n\
             Remove the offending props from the JSX call in THIS file — do NOT modify the component.\n\
             Also remove state variables and handlers that only existed to feed those removed props."
            .to_string()
        ),
    }
}

fn angular_skill() -> TechStackSkill {
    TechStackSkill {
        name: "Angular".to_string(),
        file_layout:
            "  Source root: <service-prefix>/src/app/\n\
             Feature folder per domain concept (one folder per entity/use-case):\n\
             - src/app/<feature>/<feature>.module.ts\n\
             - src/app/<feature>/<feature>.service.ts\n\
             - src/app/<feature>/<feature>.component.ts / .html\n\
             - src/app/<feature>/<feature>.model.ts\n\
             File paths in plan steps are relative to the PROJECT ROOT."
            .to_string(),
        namespace_rules:
            "  Import only from Angular packages and local files:\n\
             - @angular/core        (@Component, @Injectable, @Input, @OnInit, ...)\n\
             - @angular/common/http (HttpClient, HttpClientModule)\n\
             - @angular/forms       (FormBuilder, Validators, ReactiveFormsModule)\n\
             Never call fetch() directly — inject HttpClient and use typed generics:\n\
               this.http.post<ProductResponse>('/products', body)\n\
             Services: @Injectable({ providedIn: 'root' }) unless feature-lazy-loaded."
            .to_string(),
        layer_order:
            "  1. <feature>.model.ts      — TypeScript interfaces (no Angular deps)\n\
             2. <feature>.service.ts     — @Injectable; imports HttpClient and model\n\
             3. <feature>.module.ts      — NgModule; imports HttpClientModule, ReactiveFormsModule\n\
             4. <feature>.component.ts   — @Component; injects service\n\
             5. <feature>.component.html — template; no logic, only bindings\n\
             Reason: component depends on service; service depends on model."
            .to_string(),
        notes: Some(
            "  Prefer reactive forms (FormBuilder) over template-driven for non-trivial inputs.\n\
             Use RxJS operators (map, catchError) in service methods; subscribe in components.\n\
             Unsubscribe in ngOnDestroy or use the async pipe to avoid memory leaks."
            .to_string()
        ),
    }
}

fn node_express_skill() -> TechStackSkill {
    TechStackSkill {
        name: "Node.js / Express (TypeScript)".to_string(),
        file_layout:
            "  Source root: <service-prefix>/src/\n\
             - src/models/        — TypeScript interfaces (pure types, no runtime deps)\n\
             - src/repositories/  — data access layer; all database calls live here; no Express imports\n\
             - src/services/      — business logic; depends on repositories; no Express imports\n\
             - src/routes/        — Express routers; thin request/response handling; validate with zod\n\
             - src/middleware/    — cross-cutting (errorHandler, auth, logging)\n\
             - src/app.ts         — builds and exports the Express app; MUST NOT call app.listen()\n\
             - src/index.ts       — entry point; imports app and calls app.listen()\n\
             File paths in plan steps are relative to the PROJECT ROOT."
            .to_string(),
        namespace_rules:
            "  ES module imports throughout — never use require().\n\
             Validate input at the route boundary with zod:\n\
             - define a zod schema in the route file\n\
             - call schema.parse(req.body); let zod throw propagate to the error handler\n\
             async/await everywhere — no raw .then() chains in route handlers.\n\
             CRITICAL: src/app.ts builds and exports the Express app without calling app.listen().\n\
             src/index.ts is the ONLY file that calls app.listen().\n\
             This separation is required so Supertest can import { app } without starting a server."
            .to_string(),
        layer_order:
            "  1. src/models/        — interfaces only; no deps\n\
             2. src/repositories/  — imports models; all DB calls; no Express deps\n\
             3. src/services/      — imports models and repositories; no Express deps\n\
             4. src/routes/        — imports services; mounts on Express router; validates with zod\n\
             5. src/middleware/errorHandler.ts — depends on nothing; must be created before app.ts\n\
             6. src/app.ts         — assembles the Express app; imports routes and middleware\n\
             7. src/index.ts       — starts the server; imports app; calls app.listen()\n\
             8. tests/             — import app from src/app.ts; use Supertest for route tests\n\
             Reason: services must not import from routes; app.ts must not call listen()."
            .to_string(),
        notes: None,
    }
}

// ── Testing Skills ───────────────────────────────────────────────────────────
//
// Testing skills encode the exact framework choices, annotation patterns, and
// assertion style for each technology. They are injected at three points:
//   1. unit_test_stub_prompt  — drives TDD Red phase test generation
//   2. fix_prompt             — guides the fix loop when repairing test files
//   3. plan_prompt_for_service — tells the planner which test files to include
//
// Adding a new skill: write a const (or fn for dynamic content), add a match arm
// in unit_testing_skill() / integration_testing_skill() / testing_skill_for_file().

const SPRING_BOOT_UNIT_TEST_COMMON: &str = "\
=== Testing Skill: Spring Boot unit tests (JUnit Jupiter + AssertJ + Mockito) ===

Framework stack — all available from spring-boot-starter-test, no extra deps needed:
  JUnit Jupiter 5     org.junit.jupiter.api.{Test,BeforeEach,AfterEach,Nested,DisplayName}
  AssertJ             static import org.assertj.core.api.Assertions.*
  Mockito             static import org.mockito.Mockito.* + org.mockito.ArgumentMatchers.*
  MockMvc             org.springframework.test.web.servlet.{MockMvc,MockMvcRequestBuilders,ResultMatchers}
  Jakarta Validation  jakarta.validation.Validation.buildDefaultValidatorFactory().getValidator()

Static imports — include all relevant ones in every test file:
  import static org.assertj.core.api.Assertions.*;
  import static org.mockito.ArgumentMatchers.*;
  import static org.mockito.Mockito.*;
  import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
  import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

Assertion style — AssertJ everywhere; never bare JUnit assertions:
  assertThat(response.getId()).isNotNull()
  assertThat(response.getName()).isEqualTo(\"Widget\")
  assertThat(violations).isEmpty()
  assertThat(violations).extracting(v -> v.getPropertyPath().toString()).contains(\"name\")
  assertThatThrownBy(() -> service.method(arg)).isInstanceOf(ResponseStatusException.class)

Forbidden — these indicate a mistake, fix them immediately:
  - @SpringBootTest in unit tests → use @WebMvcTest / @DataJpaTest / @ExtendWith(MockitoExtension)
  - org.junit.Test or @RunWith    → JUnit 4; use org.junit.jupiter.api.Test and @ExtendWith
  - assertEquals / assertTrue     → use assertThat() from AssertJ
  - javax.*                       → jakarta.* only (Spring Boot 3 / Jakarta EE 9+)";

const SPRING_BOOT_INTEGRATION_TEST_SKILL: &str = "\
=== Testing Skill: Spring Boot integration tests ===

Guiding principle: prefer focused slice tests; use @SpringBootTest sparingly.

@SpringBootTest — full application context, all beans wired, real HTTP or RANDOM_PORT:
  @SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
  @AutoConfigureMockMvc
  class ProductRegistrationIT {
    @Autowired MockMvc mockMvc;
    // Tests the full stack: controller → service → repository → H2 in one shot.
    // Reserve for end-to-end scenarios that slice tests cannot cover.
  }

Prefer focused slice tests for targeted scenarios (faster, more isolated):
  @WebMvcTest(FooController.class)  → controller + HTTP layer; no JPA, no full context
  @DataJpaTest                      → repository + H2 only; no web or service layer
  @JsonTest                         → Jackson serialization only
  @RestClientTest                   → REST client only

Integration test file naming convention: *IT.java (not *Test.java).
These are the LAST steps in the implementation plan — they exercise the full stack.

Assertions: AssertJ + MockMvc (same as unit tests, see unit test skill above).";

const REACT_VITEST_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: React + TypeScript — Vitest + React Testing Library ===
Skill trigger keyword: vitest\n\
=== Testing Skill: React + TypeScript (Vitest + React Testing Library) ===

Framework stack:
  vitest                      — test runner (replaces Jest in Vite projects)
  @testing-library/react      — render(), screen, waitFor()
  @testing-library/user-event — userEvent.type(), userEvent.click()
  @testing-library/jest-dom   — .toBeInTheDocument(), .toHaveValue(), etc.

Standard imports:
  import { describe, it, expect, vi, beforeEach } from 'vitest'
  import { render, screen, waitFor } from '@testing-library/react'
  import userEvent from '@testing-library/user-event'

Component test pattern:
  describe('ProductForm', () => {
    it('should submit form data when all required fields are filled', async () => {
      const onSubmit = vi.fn()
      render(<ProductForm onSubmit={onSubmit} />)
      await userEvent.type(screen.getByLabelText(/name/i), 'Widget')
      await userEvent.click(screen.getByRole('button', { name: /submit/i }))
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ name: 'Widget' }))
    })
  })

API function test pattern (mock fetch globally):
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true, status: 201,
      headers: { get: (h: string) => h === 'Location' ? '/api/products/uuid' : null },
      json: async () => ({ id: 'uuid', name: 'Widget' })
    }))
  })

Forbidden:
  - jest.fn() → vi.fn()
  - enzyme → use @testing-library/react
  - testing implementation details (state, refs) → test via the DOM only";

const REACT_JEST_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: React + TypeScript — Jest + React Testing Library ===
Skill trigger keyword: jest (react)

Framework stack:
  jest                        — test runner (jest.fn(), jest.mock())
  @testing-library/react      — render(), screen, waitFor()
  @testing-library/user-event — userEvent.type(), userEvent.click()
  @testing-library/jest-dom   — .toBeInTheDocument(), .toHaveValue(), etc.

Standard imports:
  import { describe, it, expect, jest, beforeEach } from '@jest/globals'
  import { render, screen, waitFor } from '@testing-library/react'
  import userEvent from '@testing-library/user-event'

Component test pattern:
  describe('ProductForm', () => {
    it('should submit form data when all required fields are filled', async () => {
      const onSubmit = jest.fn()
      render(<ProductForm onSubmit={onSubmit} />)
      await userEvent.type(screen.getByLabelText(/name/i), 'Widget')
      await userEvent.click(screen.getByRole('button', { name: /submit/i }))
      expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ name: 'Widget' }))
    })
  })

API function test pattern (mock fetch globally):
  beforeEach(() => {
    global.fetch = jest.fn().mockResolvedValue({
      ok: true, status: 201,
      headers: { get: (h: string) => h === 'Location' ? '/api/products/uuid' : null },
      json: async () => ({ id: 'uuid', name: 'Widget' })
    } as Response)
  })

Forbidden:
  - vi.fn() → jest.fn()
  - enzyme → use @testing-library/react
  - testing implementation details (state, refs) → test via the DOM only";

const ANGULAR_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: Angular (TestBed + Jasmine / Jest) ===

Component tests via TestBed:
  beforeEach(() => TestBed.configureTestingModule({
    declarations: [ProductFormComponent],
    imports: [ReactiveFormsModule, HttpClientTestingModule],
    providers: [{ provide: ProductService, useValue: mockProductService }]
  }).compileComponents())
  const fixture = TestBed.createComponent(ProductFormComponent)
  fixture.detectChanges()
  const el: HTMLElement = fixture.nativeElement

Service tests:
  beforeEach(() => TestBed.configureTestingModule({
    imports: [HttpClientTestingModule],
    providers: [ProductService]
  }))
  service = TestBed.inject(ProductService)
  httpMock = TestBed.inject(HttpTestingController)
  afterEach(() => httpMock.verify())

Assertion: jasmine expect() or jest expect() depending on project config.
Prefer Angular Testing Library (@testing-library/angular) for behaviour-driven tests.";

const NODE_VITEST_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: Node.js / Express (Vitest + Supertest) ===

Trigger keyword: vitest (node)

Framework stack:
  vitest    — test runner and mocking (vi.fn(), vi.mock())
  supertest — HTTP integration: request(app).post('/api/...')

Route / integration test pattern:
  import request from 'supertest'
  import { app } from '../app'
  import { describe, it, expect, vi, beforeEach } from 'vitest'

  describe('POST /api/products', () => {
    it('returns 201 with Location header when payload is valid', async () => {
      const res = await request(app).post('/api/products')
        .send({ name: 'Widget', price: 29.99 })
        .set('Content-Type', 'application/json')
      expect(res.status).toBe(201)
      expect(res.headers.location).toMatch(/\\/api\\/products\\//)
    })
    it('returns 400 when mandatory field is missing', async () => {
      const res = await request(app).post('/api/products').send({})
      expect(res.status).toBe(400)
    })
  })

Service unit test pattern (mock repository):
  vi.mock('../repository/ProductRepository')
  import { ProductRepository } from '../repository/ProductRepository'
  const mockRepo = vi.mocked(ProductRepository)
  mockRepo.save.mockResolvedValue({ id: 'uuid', ...payload })";

const NODE_EXPRESS_UNIT_TEST_SKILL: &str = "\
=== Testing Skill: Node.js / Express (Jest + Supertest) ===

Trigger keyword: jest (node)

Framework stack:
  jest      — test runner and mocking (jest.fn(), jest.mock())
  supertest — HTTP integration: request(app).post('/api/...')

Route / integration test pattern:
  import request from 'supertest'
  import { app } from '../app'

  describe('POST /api/products', () => {
    it('returns 201 with Location header when payload is valid', async () => {
      const res = await request(app).post('/api/products')
        .send({ name: 'Widget', price: 29.99 })
        .set('Content-Type', 'application/json')
      expect(res.status).toBe(201)
      expect(res.headers.location).toMatch(/\\/api\\/products\\//)
    })
    it('returns 400 when mandatory field is missing', async () => {
      const res = await request(app).post('/api/products').send({})
      expect(res.status).toBe(400)
    })
  })

Service unit test pattern (mock repository):
  jest.mock('../repository/ProductRepository')
  const mockRepo = jest.mocked(ProductRepository)
  mockRepo.save.mockResolvedValue({ id: 'uuid', ...payload })";

fn spring_boot_unit_test_skill(layer: &str) -> String {
    let layer_pattern = match layer {
        "controller" =>
            "Layer pattern — @WebMvcTest (web slice only, no JPA or service beans):\n\
             \n  @WebMvcTest(FooController.class)\n\
               class FooControllerTest {\n\
                 @Autowired MockMvc mockMvc;\n\
                 @Autowired ObjectMapper objectMapper;\n\
                 @MockBean FooService fooService;\n\
             \n    @Test\n\
                 void should_return_201_and_location_when_data_is_valid() throws Exception {\n\
                   when(fooService.create(any())).thenReturn(savedDto);\n\
                   mockMvc.perform(post(\"/api/foos\")\n\
                           .contentType(MediaType.APPLICATION_JSON)\n\
                           .content(objectMapper.writeValueAsString(validRequest)))\n\
                       .andExpect(status().isCreated())\n\
                       .andExpect(header().exists(\"Location\"))\n\
                       .andExpect(jsonPath(\"$.id\").isNotEmpty());\n\
                 }\n\
             \n    @Test\n\
                 void should_return_400_when_mandatory_field_is_missing() throws Exception {\n\
                   mockMvc.perform(post(\"/api/foos\")\n\
                           .contentType(MediaType.APPLICATION_JSON).content(\"{}\"))\n\
                       .andExpect(status().isBadRequest());\n\
                 }\n\
               }",
        "service" =>
            "Layer pattern — @ExtendWith(MockitoExtension.class) (pure unit, no Spring context):\n\
             \n  @ExtendWith(MockitoExtension.class)\n\
               class FooServiceTest {\n\
                 @Mock FooRepository fooRepository;\n\
                 @InjectMocks FooService fooService;\n\
             \n    @Test\n\
                 void should_persist_entity_and_return_response_when_data_is_valid() {\n\
                   Foo saved = new Foo(); saved.setId(UUID.randomUUID());\n\
                   when(fooRepository.save(any(Foo.class))).thenReturn(saved);\n\
                   FooResponse response = fooService.create(request);\n\
                   assertThat(response.getId()).isNotNull();\n\
                   verify(fooRepository).save(any(Foo.class));\n\
                 }\n\
               }",
        "dto" | "domain" =>
            "Layer pattern — plain JUnit 5, no Spring context; test Bean Validation constraints:\n\
             \n  class FooRequestTest {\n\
                 private Validator validator;\n\
             \n    @BeforeEach void setUp() {\n\
                   validator = Validation.buildDefaultValidatorFactory().getValidator();\n\
                 }\n\
             \n    @Test void should_pass_when_all_mandatory_fields_are_present() {\n\
                   FooRequest req = buildValidRequest(); // set all mandatory fields\n\
                   assertThat(validator.validate(req)).isEmpty();\n\
                 }\n\
             \n    @Test void should_fail_when_name_is_blank() {\n\
                   FooRequest req = buildValidRequest(); req.setName(\"\");\n\
                   assertThat(validator.validate(req))\n\
                       .extracting(v -> v.getPropertyPath().toString()).contains(\"name\");\n\
                 }\n\
               }",
        _ => "Layer pattern: choose @WebMvcTest / @ExtendWith(MockitoExtension) / plain JUnit 5 based on what the class does.",
    };
    format!("{SPRING_BOOT_UNIT_TEST_COMMON}\n\n{layer_pattern}")
}

/// Returns the unit testing skill for the given technology and layer.
/// Used in: TDD Red phase test generation, fix loop for unit test files.
/// Layer values: "controller" | "service" | "dto" | "domain" | "" (generic)
pub fn unit_testing_skill(tech: &str, layer: &str) -> String {
    let t = tech.to_lowercase();
    if t.contains("spring") || t.contains("quarkus") || t.contains("micronaut")
        || (t.contains("java") && !t.contains("javascript")) || t.contains("kotlin")
    {
        spring_boot_unit_test_skill(layer)
    } else if t.contains("react") || t.contains("vite") {
        REACT_VITEST_UNIT_TEST_SKILL.to_string()
    } else if t.contains("angular") {
        ANGULAR_UNIT_TEST_SKILL.to_string()
    } else if t.contains("node") || t.contains("express") || t.contains("nest") {
        NODE_EXPRESS_UNIT_TEST_SKILL.to_string()
    } else {
        String::new()
    }
}

/// Returns the integration testing skill for the given technology.
/// Used in: plan prompt (for *IT.java steps), fix loop for integration test files.
pub fn integration_testing_skill(tech: &str) -> String {
    let t = tech.to_lowercase();
    if t.contains("spring") || t.contains("quarkus") || t.contains("micronaut")
        || (t.contains("java") && !t.contains("javascript")) || t.contains("kotlin")
    {
        SPRING_BOOT_INTEGRATION_TEST_SKILL.to_string()
    } else if t.contains("react") || t.contains("vite") {
        "Integration tests: use Playwright or Cypress for full end-to-end browser tests. \
         Use msw (Mock Service Worker) for API-level integration tests within Vitest.".to_string()
    } else if t.contains("node") || t.contains("express") {
        "Integration tests: use Jest + Supertest against the full Express app (see unit test skill — \
         Supertest tests already exercise the HTTP + service + repository stack).".to_string()
    } else {
        String::new()
    }
}

/// Returns the appropriate testing skill when fixing a test file.
/// Detects unit vs integration by file name convention (*IT.java = integration).
/// Returns empty string for non-test files — no testing skill needed.
pub fn testing_skill_for_file(file_path: &str, tech: &str) -> String {
    testing_skill_for_file_with_adrs(file_path, tech, &[])
}

/// ADR-aware variant of testing_skill_for_file.
/// When a "Testing Strategy" ADR exists its decision overrides the technology-based default
/// (e.g. "jest" keyword routes React to the Jest skill instead of the Vitest skill).
pub fn testing_skill_for_file_with_adrs(file_path: &str, tech: &str, adrs: &[Adr]) -> String {
    let is_test_file =
        file_path.ends_with("Test.java")
        || file_path.ends_with("IT.java")
        || file_path.ends_with(".test.ts")
        || file_path.ends_with(".test.tsx")
        || file_path.ends_with(".spec.ts")
        || file_path.ends_with(".spec.tsx");
    if !is_test_file {
        return String::new();
    }
    if file_path.ends_with("IT.java") {
        integration_testing_skill(tech)
    } else {
        testing_skill_from_adrs(adrs, tech, "")
    }
}

/// Resolves the unit testing skill for the given technology, consulting the Testing Strategy ADRs.
///
/// ADR titles are tech-specific so React and Node projects can coexist:
///   "Frontend Testing Strategy" → consulted for React/Vite tech
///   "Node.js Testing Strategy"  → consulted for Node/Express tech
///   "Testing Strategy"          → legacy fallback (pre-dating the split titles)
///
/// The ADR decision field carries a keyword that selects the skill:
///   React/Vite + "vitest"   → REACT_VITEST_UNIT_TEST_SKILL
///   React/Vite + "jest"     → REACT_JEST_UNIT_TEST_SKILL
///   Angular                 → ANGULAR_UNIT_TEST_SKILL (implicit, no ADR needed)
///   Node/Express + "vitest" → NODE_VITEST_UNIT_TEST_SKILL
///   Node/Express + "jest"   → NODE_EXPRESS_UNIT_TEST_SKILL
///
/// Falls back to unit_testing_skill(tech, layer) when no relevant ADR exists.
pub fn testing_skill_from_adrs(adrs: &[Adr], tech: &str, layer: &str) -> String {
    let t = tech.to_lowercase();
    let is_react = t.contains("react") || t.contains("vite");
    let is_node  = t.contains("node") || t.contains("express") || t.contains("nest");

    let adr = if is_react {
        adrs.iter().find(|a| {
            let title = a.title.to_lowercase();
            title.contains("frontend testing") || title == "testing strategy"
        })
    } else if is_node {
        adrs.iter().find(|a| {
            let title = a.title.to_lowercase();
            (title.contains("node") && title.contains("testing")) || title == "testing strategy"
        })
    } else {
        adrs.iter().find(|a| a.title.to_lowercase().contains("testing strategy"))
    };

    if let Some(adr) = adr {
        let d = adr.decision.to_lowercase();
        if is_react && d.contains("vitest") {
            return REACT_VITEST_UNIT_TEST_SKILL.to_string();
        }
        if is_react && d.contains("jest") && !d.contains("vitest") {
            return REACT_JEST_UNIT_TEST_SKILL.to_string();
        }
        if t.contains("angular") || d.contains("angular testbed") {
            return ANGULAR_UNIT_TEST_SKILL.to_string();
        }
        if is_node && d.contains("vitest") {
            return NODE_VITEST_UNIT_TEST_SKILL.to_string();
        }
        if is_node && d.contains("jest") && !d.contains("vitest") {
            return NODE_EXPRESS_UNIT_TEST_SKILL.to_string();
        }
    }
    unit_testing_skill(tech, layer)
}

/// Return the rendered skill block for the given technology.
/// Returns an empty string if no built-in skill matches (LLM gets no extra rules).
/// JVM skills receive dynamic package context; others are technology-only.
/// To add a new stack: implement a builder function, add a match arm here.
pub fn skill_for_technology(tech: &str, pkg: &str, pkg_path: &str, service_name: &str) -> String {
    let t = tech.to_lowercase();
    let skill: Option<TechStackSkill> =
        if t.contains("spring") || t.contains("quarkus") || t.contains("micronaut")
            || (t.contains("java") && !t.contains("javascript"))
            || t.contains("kotlin")
        {
            Some(spring_boot_skill(pkg, pkg_path, service_name))
        } else if t.contains("react") || t.contains("vite") {
            Some(react_vite_skill())
        } else if t.contains("angular") {
            Some(angular_skill())
        } else if t.contains("node") || t.contains("express") || t.contains("nest") {
            Some(node_express_skill())
        } else {
            None
        };
    skill.map(|s| s.render()).unwrap_or_default()
}

// ── Architecture skills ───────────────────────────────────────────────────────
// Architecture skills are orthogonal to tech-stack skills.
// They capture cross-cutting patterns that apply regardless of language or framework.
// Contract: every architecture skill fills three required sections:
//   vocabulary      — terms this pattern introduces and their precise meaning in code
//   structural_rules — naming, layering, and dependency rules
//   anti_patterns   — explicit prohibitions that prevent the most common mistakes
//
// Skills are derived from ADR decisions (keyword matching on the architecture-style ADR).
// To add a new architecture skill: implement a builder, add keyword detection below.

pub struct ArchitectureSkill {
    pub name: String,
    /// Terms this pattern introduces and their precise meaning in code.
    pub vocabulary: String,
    /// Naming, layering, and dependency rules.
    pub structural_rules: String,
    /// Explicit prohibitions — what NOT to do.
    pub anti_patterns: String,
}

impl ArchitectureSkill {
    pub fn render(&self) -> String {
        format!(
            "Architecture pattern — {name}:\n\
             \n\
             Vocabulary:\n{vocab}\n\
             \n\
             Structural rules:\n{rules}\n\
             \n\
             Anti-patterns (never do these):\n{anti}",
            name  = self.name,
            vocab = self.vocabulary,
            rules = self.structural_rules,
            anti  = self.anti_patterns,
        )
    }
}

fn ddd_skill() -> ArchitectureSkill {
    ArchitectureSkill {
        name: "Domain-Driven Design (DDD)".to_string(),
        vocabulary:
            "  Aggregate root: the single entry point to a cluster of related entities; holds invariants.\n\
             Entity: has identity (@Id), mutable state, lifecycle — modelled in domain/.\n\
             Value object: no identity, equality by value, immutable — use Java records.\n\
             Repository: one per aggregate root; returns fully-constructed aggregates.\n\
             Application service (@Service): orchestrates use cases, translates domain ↔ DTO.\n\
             Domain service: stateless; expresses a business operation that spans multiple entities."
            .to_string(),
        structural_rules:
            "  Use the ubiquitous language from the stories and domain registry in all identifiers —\n\
             class names, method names, field names. No technical synonyms (ProductData, ProductInfo)\n\
             when the agreed term is Product.\n\
             Business invariants live in the aggregate, not in the application service or controller.\n\
             DTOs live at the API boundary (dto/); never expose domain entities in REST responses.\n\
             Access nested entities only through their aggregate root — never inject a nested entity's\n\
             repository directly.\n\
             Repositories return domain objects; the service layer maps them to DTOs for callers."
            .to_string(),
        anti_patterns:
            "  No business logic in controllers or repositories — controllers translate HTTP;\n\
             repositories translate persistence.\n\
             No anemic domain model — an entity that is only getters/setters with all logic in\n\
             services is not DDD; move invariants into the entity.\n\
             No getById that silently returns null — throw a domain exception (ProductNotFoundException)\n\
             or return Optional with explicit handling at the call site."
            .to_string(),
    }
}

fn event_orientation_skill() -> ArchitectureSkill {
    ArchitectureSkill {
        name: "Event Orientation".to_string(),
        vocabulary:
            "  Domain event: a fact that happened — immutable, past tense, e.g. ProductRegistered.\n\
             Event publisher: the service layer that emits events after successful persistence.\n\
             Event listener: a separate class that reacts to one event; one concern per listener.\n\
             Transactional boundary: the unit-of-work that must complete before an event is visible."
            .to_string(),
        structural_rules:
            "  Name events in past tense using domain language: ProductRegistered, OrderPlaced.\n\
             Define event classes in the domain layer alongside the aggregate they describe.\n\
             Publish events from the service layer after the aggregate is persisted — never before.\n\
             Use @TransactionalEventListener(phase = AFTER_COMMIT) so listeners fire only on\n\
             successful commit; this prevents phantom events from rolled-back transactions.\n\
             Event payload: include the aggregate ID always; carry only what consumers need —\n\
             do not copy the full aggregate state.\n\
             One listener class per consuming concern; listeners must not call back into the\n\
             publishing service (no circular event chains)."
            .to_string(),
        anti_patterns:
            "  Never publish events before the database write commits — a rollback after publish\n\
             creates phantom events that consumers act on against data that was never saved.\n\
             Never use events for synchronous responses — if the caller needs a return value,\n\
             use a direct service call, not an event.\n\
             Never import ApplicationEventPublisher into the domain model — it is infrastructure;\n\
             the domain emits events as return values or via a domain service; the application\n\
             service calls the publisher.\n\
             ApplicationEventPublisher is included via spring-boot-starter — no extra Maven\n\
             dependency is needed or should be added."
            .to_string(),
    }
}

fn microservices_skill() -> ArchitectureSkill {
    ArchitectureSkill {
        name: "Microservices".to_string(),
        vocabulary:
            "  Bounded context: the domain scope of one service — it owns its data and its language.\n\
             Service contract: the OAS API surface and the domain events a service publishes;\n\
             the only things other services may depend on.\n\
             Anti-corruption layer: an adapter that translates between two bounded contexts so\n\
             their models stay independent."
            .to_string(),
        structural_rules:
            "  Each service owns exactly one database schema — no other service reads or writes\n\
             its tables directly.\n\
             Cross-service state changes: prefer async domain events over synchronous HTTP calls.\n\
             Synchronous HTTP (OAS contract) is acceptable for queries needing an immediate response.\n\
             A service's domain model classes are never imported by another service — duplicate\n\
             the fields you need as local DTOs rather than sharing a domain library.\n\
             Service names are kebab-case and match the bounded context they represent."
            .to_string(),
        anti_patterns:
            "  No shared database between services — even read-only access couples services to\n\
             each other's schema evolution.\n\
             No distributed transactions — use eventual consistency and compensating events.\n\
             No shared domain model library — a common-domain JAR couples release cycles and\n\
             violates bounded context autonomy.\n\
             No direct method calls into another service's internal classes — only through its\n\
             published OAS contract or domain events."
            .to_string(),
    }
}

/// Derive active architecture skills from the project's ADR decisions.
/// Scans each ADR for keywords and maps them to the corresponding skill.
/// Returns the rendered skills joined as a single string for prompt injection.
pub fn skills_for_architecture(adrs: &[Adr]) -> String {
    let text: String = adrs.iter()
        .map(|a| format!("{} {}", a.title, a.decision).to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    let mut skills: Vec<ArchitectureSkill> = Vec::new();
    if text.contains("domain-driven") || text.contains("domain driven") || text.contains("ddd") {
        skills.push(ddd_skill());
    }
    if text.contains("event-driven") || text.contains("event driven") || text.contains("domain event") {
        skills.push(event_orientation_skill());
    }
    if text.contains("microservice") {
        skills.push(microservices_skill());
    }

    if skills.is_empty() {
        return String::new();
    }
    skills.iter().map(|s| s.render()).collect::<Vec<_>>().join("\n\n")
}

// ── Build system skills ───────────────────────────────────────────────────────
// Build system skills are orthogonal to tech-stack and architecture skills.
// They capture how to write and fix build manifests correctly.
// The pom.xml fix loop only needs the build skill — not the tech-stack skill.
// The step prompt for a build file gets both: tech skill (which deps) + build skill (how to write).
//
// Contract: every build system skill fills three required sections:
//   manifest_contract — required sections, completeness rules, validity constraints
//   dependency_rules  — allowed registries/coordinates, scope keywords, version management
//   anti_patterns     — hallucination patterns this build system is prone to
//
// Detection: skill_for_build_system(file_path) matches by file name.
// To add a new build system: implement a builder, add a match arm in skill_for_build_system().

pub struct BuildSystemSkill {
    pub name: String,
    /// Required sections, completeness rules, structural validity constraints.
    pub manifest_contract: String,
    /// Allowed registries, coordinate rules, scope keywords, version management.
    pub dependency_rules: String,
    /// Hallucination patterns to avoid — specific to this build system.
    pub anti_patterns: String,
}

impl BuildSystemSkill {
    pub fn render(&self) -> String {
        format!(
            "Build system — {name}:\n\
             \n\
             Manifest contract:\n{manifest}\n\
             \n\
             Dependency rules:\n{deps}\n\
             \n\
             Anti-patterns (never do these):\n{anti}",
            name     = self.name,
            manifest = self.manifest_contract,
            deps     = self.dependency_rules,
            anti     = self.anti_patterns,
        )
    }
}

fn maven_skill() -> BuildSystemSkill {
    BuildSystemSkill {
        name: "Maven (pom.xml)".to_string(),
        manifest_contract:
            "  The pom.xml must be a complete, well-formed XML file ending with </project>.\n\
             A truncated POM is a fatal parse error — Maven refuses to read it.\n\
             Required sections in order:\n\
             - <modelVersion>4.0.0</modelVersion>\n\
             - <parent> block (Spring Boot BOM, if applicable)\n\
             - <groupId>, <artifactId>, <version>, <name>\n\
             - <properties> with <java.version>17</java.version>\n\
             - <dependencies> with all required starters\n\
             - <build> with <plugins> containing spring-boot-maven-plugin\n\
             When spring-boot-starter-parent is the <parent>, do NOT add <version> on\n\
             managed starters — the BOM manages them."
            .to_string(),
        dependency_rules:
            "  Only use groupIds that exist on Maven Central:\n\
             - org.springframework.boot  (starters — no explicit version needed with BOM)\n\
             - com.h2database            (h2, scope: runtime)\n\
             - org.projectlombok        (lombok, optional: true)\n\
             - com.fasterxml.jackson.*  (jackson-databind etc.)\n\
             - org.junit.*              (scope: test)\n\
             - org.assertj.*            (scope: test)\n\
             - org.mockito.*            (scope: test)\n\
             Scope keywords: omit for compile (default), <scope>test</scope> for test-only,\n\
             <scope>runtime</scope> for runtime-only."
            .to_string(),
        anti_patterns:
            "  Never add a <dependency> whose <groupId> matches or is derived from the project's\n\
             own <groupId> — your own classes are not published JARs.\n\
             Domain event classes (ProductCreated, OrderPlaced) live in the service's own\n\
             domain/ package — never add them as a Maven dependency.\n\
             ApplicationEventPublisher is in spring-context, already on the classpath via\n\
             spring-boot-starter — no extra <dependency> is needed or should be added.\n\
             Never truncate the file — the closing </project> tag is mandatory."
            .to_string(),
    }
}

fn gradle_skill() -> BuildSystemSkill {
    BuildSystemSkill {
        name: "Gradle (Groovy/Kotlin DSL)".to_string(),
        manifest_contract:
            "  Required block order: plugins {}, java {} toolchain, repositories {}, dependencies {}.\n\
             java {\n\
               toolchain { languageVersion = JavaLanguageVersion.of(17) }\n\
             }\n\
             repositories { mavenCentral() }\n\
             The file must be syntactically complete — Gradle fails silently on unterminated blocks."
            .to_string(),
        dependency_rules:
            "  Configuration keywords: implementation (compile), testImplementation (test-only),\n\
             runtimeOnly (runtime-only), annotationProcessor (APT — e.g. Lombok).\n\
             Spring Boot Gradle plugin manages versions — omit explicit version strings for\n\
             Spring Boot managed dependencies.\n\
             Same groupId restrictions as Maven: only well-known Maven Central groupIds."
            .to_string(),
        anti_patterns:
            "  Never use the deprecated compile configuration (removed in Gradle 7) — use implementation.\n\
             Same invented-coordinate prohibitions as Maven: no groupIds derived from the project,\n\
             no domain event JARs, no ApplicationEventPublisher dependency."
            .to_string(),
    }
}

fn npm_skill() -> BuildSystemSkill {
    BuildSystemSkill {
        name: "npm (package.json)".to_string(),
        manifest_contract:
            "  Required fields: name, version, \"private\": true (for apps), scripts,\n\
             dependencies, devDependencies.\n\
             scripts must include \"build\" and \"dev\".\n\
             TypeScript projects must include \"type-check\": \"tsc --noEmit\" in scripts.\n\
             Must be valid JSON — trailing commas cause a parse failure."
            .to_string(),
        dependency_rules:
            "  dependencies: runtime packages that ship to production (react, react-dom).\n\
             devDependencies: build tools and type stubs (vite, typescript, @types/*).\n\
             @types/* packages always belong in devDependencies, never dependencies.\n\
             Never use file: or link: references unless this is a configured monorepo workspace.\n\
             Only reference packages published to registry.npmjs.org."
            .to_string(),
        anti_patterns:
            "  Never add a package reference for files in your own src/ — those are TypeScript\n\
             imports, not npm packages.\n\
             Never add axios, node-fetch, or other HTTP client libraries unless the story\n\
             explicitly requires them — use the built-in fetch().\n\
             Never add @types/react to dependencies — it belongs in devDependencies."
            .to_string(),
    }
}

fn dotnet_skill() -> BuildSystemSkill {
    BuildSystemSkill {
        name: ".NET (csproj / MSBuild)".to_string(),
        manifest_contract:
            "  Opening tag for web APIs: <Project Sdk=\"Microsoft.NET.Sdk.Web\">\n\
             Required <PropertyGroup> elements:\n\
             - <TargetFramework>net8.0</TargetFramework>\n\
             - <Nullable>enable</Nullable>\n\
             - <ImplicitUsings>enable</ImplicitUsings>\n\
             <PackageReference> elements go inside an <ItemGroup>.\n\
             Must be well-formed XML."
            .to_string(),
        dependency_rules:
            "  Only packages from NuGet.org.\n\
             <ProjectReference> only for .csproj files that actually exist in the solution —\n\
             verify the path before adding.\n\
             Use Microsoft.AspNetCore.* packages — never Microsoft.AspNet.* (legacy, pre-.NET Core)."
            .to_string(),
        anti_patterns:
            "  Never add a <PackageReference> for types defined within the same project or solution.\n\
             Never reference a <ProjectReference> path that does not exist on disk.\n\
             Never use Microsoft.AspNet.* — the correct namespace is Microsoft.AspNetCore.*"
            .to_string(),
    }
}

/// Return the rendered build system skill for the given build file path.
/// Detected by file name; returns empty string if no built-in skill matches.
/// To add a new build system: implement a builder, add a match arm here.
pub fn skill_for_build_system(file_path: &str) -> String {
    let name = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let skill: Option<BuildSystemSkill> = if name == "pom.xml" {
        Some(maven_skill())
    } else if name == "build.gradle" || name == "build.gradle.kts"
           || name == "settings.gradle" || name == "settings.gradle.kts" {
        Some(gradle_skill())
    } else if name == "package.json" {
        Some(npm_skill())
    } else if file_path.ends_with(".csproj") {
        Some(dotnet_skill())
    } else {
        None
    };
    skill.map(|s| s.render()).unwrap_or_default()
}

fn plan_prompt_for_service(
    service: &ServiceEntry,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    adrs: &[Adr],
    existing_files: &[String],
    service_packages: &std::collections::HashMap<String, String>,
    arch_skills: &str,
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

    // Show the directory prefix for every service so the LLM never invents bare /src/ paths.
    let location_line = if is_front {
        format!(
            "Service directory prefix: frontend/{name}/\n\
             ALL file paths in steps MUST start with frontend/{name}/  — never use bare src/ paths.",
            name = service.name)
    } else {
        format!(
            "Service directory prefix: services/{name}/\n\
             ALL file paths in steps MUST start with services/{name}/  — never use bare /src/ or /events/ paths.",
            name = service.name)
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
    let arch_section = if arch_skills.is_empty() {
        String::new()
    } else {
        format!("\n{arch_skills}\n")
    };
    let t_lower = tech.to_lowercase();
    let is_jvm = t_lower.contains("spring") || t_lower.contains("quarkus")
        || (t_lower.contains("java") && !t_lower.contains("javascript"))
        || t_lower.contains("kotlin");
    let is_node = t_lower.contains("node") || t_lower.contains("express") || t_lower.contains("nest");
    let is_react = is_front || t_lower.contains("react") || t_lower.contains("vite") || t_lower.contains("angular");
    let testing_section = if is_jvm {
        let it_skill = integration_testing_skill(tech);
        format!(
            "\nTesting plan rules:\n\
             - Unit test files (*Test.java) are auto-generated per class by the TDD loop.\n\
               DO NOT include unit test files in this plan.\n\
             - Integration test files (*IT.java) test the full stack end-to-end.\n\
               Include them as the LAST step(s) in the plan.\n\
             {it_skill}\n"
        )
    } else if is_node {
        format!(
            "\nTesting plan rules:\n\
             - Include one unit test file (*.test.ts) per service module.\n\
               Unit tests mock the repository and test business logic in isolation.\n\
               Example: tests/productService.test.ts\n\
             - Include one route test file (*.test.ts) per route module using Supertest.\n\
               Route tests import {{ app }} from src/app.ts and exercise the full HTTP stack.\n\
               Example: tests/productRoutes.test.ts\n\
             - Route test files are the LAST step(s) in the plan.\n\
             - Do NOT include a test file for src/app.ts or src/index.ts.\n"
        )
    } else if is_react {
        format!(
            "\nTesting plan rules:\n\
             - Include one unit test file (*.test.tsx / *.test.ts) per component and per API module.\n\
             - Test files go in a FLAT tests/ directory — never in tests/unit/ or tests/integration/ subdirectories.\n\
             - ONLY use libraries from the decided testing strategy ADR.\n\
               Do NOT introduce msw, Playwright, Cypress, or any library not named in the ADR.\n\
             - Test files are the LAST step(s) in the plan.\n"
        )
    } else {
        let it_skill = integration_testing_skill(tech);
        format!(
            "\nTesting plan rules:\n\
             - Include unit test files as needed for each module.\n\
             - Include integration tests as the LAST step(s) in the plan.\n\
             {it_skill}\n"
        )
    };

    format!(
        "Generate implementation steps for service '{sname}' as part of story '{story_id}'.\n\
         \n\
         Story: As a {as_a}, I want {want}, so that {so_that}.\n\
         \n\
         Service: {sname}  Technology: {tech}\n\
         {location_line}\n\
         {skill_section}\
         {arch_section}\
         {testing_section}\n\
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
         - Order for backend: build config → domain → data layer → service → controller → tests\n\
         - Order for frontend (STRICT): API client (src/api/) MUST come before components that import it;\n\
           then components (src/components/); then App wiring (App.tsx / main.tsx); then tests LAST.\n\
           Reason: each file imports from the previous; generate in dependency order so types are available.\n\
         - description must name the specific classes, fields, and annotations\n\
         - ALL string values must be quoted with double quotes — every id, service, file, operation, and description\n\
         - Never use block scalars (>- or |) — always use a single quoted string on one line\n\
         - STRICT SCOPE: only include files that are directly required to implement this story.\n\
           Do NOT include: README, HELP.md, .gitignore, CSS files, config files (tsconfig, vite.config),\n\
           scaffolding artifacts, or any file that does not contain logic for this story.\n\
           Do NOT include event listeners, consumers, or subscribers unless the story explicitly requires\n\
           consuming a domain event — publishing and consuming are separate stories.\n\
           If no event broker appears in the architecture decisions, do not plan publisher infrastructure;\n\
           limit event handling to defining the event type only.\n\
           If in doubt, leave it out.\n",
        sname = service.name,
        story_id = story.id,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        tech = tech,
        location_line = location_line,
        skill_section = skill_section,
        arch_section = arch_section,
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

    let arch_skills = skills_for_architecture(adrs);

    let mut all_steps: Vec<ImplementationStep> = Vec::new();
    for service in &active {
        let prompt = plan_prompt_for_service(
            service, story, spec, contract_yaml, adrs, existing_files, service_packages, &arch_skills,
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
    session_written: &std::collections::HashMap<String, String>,
    arch_skills: &str,
    test_hint: Option<(&str, &str, bool)>,
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

    let tech_rules = skill_for_technology(technology, &pkg, &pkg_path, service_name);
    // For build manifest files, also inject the build system skill.
    // The tech skill says WHICH dependencies belong; the build skill says HOW to write the file.
    let build_rules = skill_for_build_system(&step.file);

    // For frontend (TypeScript) steps, include all sibling .ts/.tsx files written earlier
    // in this session. Roots does not index TypeScript, so this is the only way to give
    // the model knowledge of existing component signatures and exported types.
    let sibling_section = if is_frontend {
        let mut siblings: Vec<String> = session_written.iter()
            .filter(|(path, _)| {
                (path.ends_with(".ts") || path.ends_with(".tsx"))
                    && path.as_str() != step.file
            })
            .map(|(path, content)| format!("--- {} ---\n{}", path, content))
            .collect();
        siblings.sort();
        if siblings.is_empty() {
            String::new()
        } else {
            format!(
                "\nFiles already written for this frontend \
                 (match their component signatures and exported types exactly):\n\n{}\n",
                siblings.join("\n\n")
            )
        }
    } else {
        String::new()
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

    let test_hint_section = match test_hint {
        Some((tf, tc, true)) => format!(
            "\nSTUB ONLY — return a compilable skeleton, no business logic:\n\
             - Declare every class, field, constructor, and method the unit test below references.\n\
             - Method bodies: `return null;` for objects, `return 0;` for numbers, `return false;` for booleans, `return List.of();` for collections.\n\
             - Do NOT implement any logic — the Green phase replaces this stub with the real implementation.\n\
             \n\
             Unit test this stub must compile against:\n\
             --- {tf} ---\n\
             {tc}\n"
        ),
        Some((tf, tc, false)) => format!(
            "\nGREEN PHASE — implement to make all unit tests below pass.\n\
             Read the test file carefully: every assertion is a requirement.\n\
             \n\
             Unit tests that must pass:\n\
             --- {tf} ---\n\
             {tc}\n"
        ),
        None => String::new(),
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
         {sibling_section}\
         {current_section}\
         {roots_section}\
         {test_hint_section}\n\
         {tech_rules}\n\
         {build_rules}\n\
         {arch_rules}\n\
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
        sibling_section = sibling_section,
        current_section = current_section,
        roots_section = roots_section,
        test_hint_section = test_hint_section,
        tech_rules = tech_rules,
        build_rules = build_rules,
        arch_rules = arch_skills,
    )
}

fn strip_code_fences(raw: &str) -> String {
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
    if let Some(pos) = after_open.rfind("\n```") {
        after_open[..pos].to_string()
    } else {
        after_open.trim_end_matches("```").trim_end().to_string()
    }
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
    session_written: &std::collections::HashMap<String, String>,
    arch_skills: &str,
) -> Result<String, LlmError> {
    let prompt = step_prompt(
        story, spec, contract_yaml, step, current_content, roots_context,
        service_packages, services, session_written, arch_skills, None,
    );
    Ok(strip_code_fences(&client.complete_large(&prompt)?))
}

fn unit_test_stub_prompt(
    story: &UserStory,
    spec: &IntentSpec,
    step: &ImplementationStep,
    test_file: &str,
    service_packages: &std::collections::HashMap<String, String>,
    _services: &ServicesRegistry,
    adrs: &[Adr],
) -> String {
    let service_name = step.service.rsplit('/').next().unwrap_or(&step.service);
    let pkg = service_packages.get(service_name)
        .cloned()
        .unwrap_or_else(|| service_name.replace('-', "_"));

    let impl_file = &step.file;
    let class_name = std::path::Path::new(impl_file.as_str())
        .file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
    let test_class = format!("{}Test", class_name);

    let layer = if impl_file.contains("/controller/") { "controller" }
        else if impl_file.contains("/service/") { "service" }
        else if impl_file.contains("/dto/") { "dto" }
        else if impl_file.contains("/domain/") { "domain" }
        else { "class" };

    let schema_yaml = spec.entity_schema.as_ref()
        .map(|s| serde_yaml::to_string(s).unwrap_or_default())
        .unwrap_or_default();
    let scenarios_yaml = serde_yaml::to_string(&spec.scenarios).unwrap_or_default();

    let service_entry = _services.services.iter()
        .find(|s| s.name == service_name || s.name == step.service);
    let technology = service_entry.and_then(|s| s.technology.as_deref()).unwrap_or("unknown");
    let test_skill = testing_skill_from_adrs(adrs, technology, layer);

    format!(
        "Generate a JUnit 5 unit test class '{test_class}' to drive TDD for '{impl_class}'.\n\
         \n\
         Implementation file : {impl_file}\n\
         Test file to create : {test_file}\n\
         Layer               : {layer}\n\
         Package base        : {pkg}\n\
         Service             : {service_name}\n\
         \n\
         Story: As a {as_a}, I want {want}, so that {so_that}.\n\
         \n\
         Entity schema:\n\
         {schema_yaml}\n\
         BDD scenarios — one @Test method per scenario:\n\
         {scenarios_yaml}\n\
         \n\
         {test_skill}\n\
         \n\
         Method naming: should_<expected_outcome>_when_<condition>  (snake_case)\n\
         \n\
         Body structure:\n\
         // Arrange — build minimal valid inputs from entity schema field definitions\n\
         // Act     — call the method under test\n\
         // Assert  — verify the 'then' clause of the BDD scenario\n\
         \n\
         IMPORTANT:\n\
         - Write REAL assertions that verify actual behaviour.\n\
         - Tests will be Red naturally because the stub returns null/0/false.\n\
           The Green phase makes them pass. Do NOT use Assertions.fail().\n\
         - Package declaration: derive sub-package from the test file path.\n\
         - Import {impl_class} from its package under {pkg}.\n\
         - Use jakarta.* everywhere — never javax.*\n\
         \n\
         Return ONLY the raw Java file content — no code fences, no explanation.",
        test_class = test_class,
        impl_class = class_name,
        impl_file = impl_file,
        test_file = test_file,
        layer = layer,
        pkg = pkg,
        service_name = service_name,
        as_a = story.as_a,
        want = story.want,
        so_that = story.so_that,
        schema_yaml = schema_yaml,
        scenarios_yaml = scenarios_yaml,
        test_skill = test_skill,
    )
}

pub fn generate_unit_test_stub(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    step: &ImplementationStep,
    test_file: &str,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    adrs: &[Adr],
) -> Result<String, LlmError> {
    let prompt = unit_test_stub_prompt(story, spec, step, test_file, service_packages, services, adrs);
    Ok(strip_code_fences(&client.complete_large(&prompt)?))
}

pub fn execute_implementation_stub(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    session_written: &std::collections::HashMap<String, String>,
    arch_skills: &str,
    test_file: &str,
    test_content: &str,
) -> Result<String, LlmError> {
    let prompt = step_prompt(
        story, spec, contract_yaml, step, current_content, roots_context,
        service_packages, services, session_written, arch_skills,
        Some((test_file, test_content, true)),
    );
    Ok(strip_code_fences(&client.complete_large(&prompt)?))
}

pub fn execute_implementation_with_test(
    client: &LlmClient,
    story: &UserStory,
    spec: &IntentSpec,
    contract_yaml: &str,
    step: &ImplementationStep,
    current_content: Option<&str>,
    roots_context: Option<&str>,
    service_packages: &std::collections::HashMap<String, String>,
    services: &ServicesRegistry,
    session_written: &std::collections::HashMap<String, String>,
    arch_skills: &str,
    test_file: &str,
    test_content: &str,
) -> Result<String, LlmError> {
    let prompt = step_prompt(
        story, spec, contract_yaml, step, current_content, roots_context,
        service_packages, services, session_written, arch_skills,
        Some((test_file, test_content, false)),
    );
    Ok(strip_code_fences(&client.complete_large(&prompt)?))
}

fn fix_prompt(
    file_path: &str,
    content: &str,
    errors: &str,
    existing_files: &[String],
    referenced_files: &[(String, String)],
    skill: &str,
    arch_skills: &str,
) -> String {
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
         - The file must begin with the package declaration\n\
         - Constructor mismatch: look at the referenced file to find the available constructor(s).\n\
           If only a no-args constructor is present, use: Foo f = new Foo(); f.setField(value); ...\n\
           Do NOT call a multi-arg constructor that is not declared in the referenced file.\n\
           Do NOT add a new constructor to a class that lives in a referenced file — only fix THIS file."
    } else if file_path.ends_with("pom.xml") {
        "\n- Only use dependencies from well-known Maven Central groupIds:\n\
           org.springframework.boot, com.h2database, org.projectlombok, com.fasterxml.jackson.*,\n\
           org.junit.*, org.assertj.*, org.mockito.*\n\
         - Remove any dependency whose groupId is derived from this project — those are not published artifacts\n\
         - Domain event classes (e.g. ProductCreated) are in the service's own domain/ package; they are NOT\n\
           a separate JAR — remove any such dependency\n\
         - ApplicationEventPublisher is in spring-context (via spring-boot-starter); no extra dep needed\n\
         - For javax.validation use spring-boot-starter-validation\n\
         - For javax.persistence use spring-boot-starter-data-jpa\n\
         - Do not remove existing valid dependencies\n\
         - Keep the XML well-formed and end with </project>"
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
    // For TypeScript errors: include the content of related files so the model can fix
    // cross-file type mismatches (e.g. a missing props interface in an imported component).
    let referenced_section = if !referenced_files.is_empty() {
        let parts: Vec<String> = referenced_files.iter()
            .map(|(path, c)| format!("--- {} ---\n{}", path, c))
            .collect();
        let label = if ext == "java" {
            "Referenced files — check these for available constructors, setter methods, and field types \
             before writing any new() calls or method invocations:"
        } else {
            "Referenced files — check these for the correct component signatures and exported types:"
        };
        format!(
            "\n{label}\n\n{}\n",
            parts.join("\n\n")
        )
    } else {
        String::new()
    };
    let skill_section = if skill.is_empty() {
        String::new()
    } else {
        format!("\n{skill}\n")
    };
    let arch_section = if arch_skills.is_empty() {
        String::new()
    } else {
        format!("\n{arch_skills}\n")
    };
    format!(
        "Fix the {lang} file below so that all listed errors are resolved.\n\
         \n\
         File: {file_path}\n\
         \n\
         Errors:\n\
         {errors}\n\
         {files_section}\
         {referenced_section}\
         {skill_section}\
         {arch_section}\n\
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
    referenced_files: &[(String, String)],
    skill: &str,
    arch_skills: &str,
) -> Result<String, LlmError> {
    let raw = client.complete_large(&fix_prompt(file_path, content, errors, existing_files, referenced_files, skill, arch_skills))?;
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
