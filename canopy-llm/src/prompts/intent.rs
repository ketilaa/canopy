use crate::client::{LlmClient, LlmError};
use crate::prompts::yaml_util::strip_code_fence;
use canopy_core::*;

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
- "want" MUST explicitly name the primary domain object being acted upon, and it MUST be
  consistent with the intent statement above. NEVER use a generic object name: account, record,
  item, thing, object, entity. WRONG: "register in the system", "create a record", "submit
  information". CORRECT pattern — "<domain verb> <real domain object from the intent>": if the
  intent is about manufacturers, write "register a manufacturer"; if about products, "register a
  product"; if about customers, "register a customer".
- "so_that" must state a single concrete business or user benefit — one idea, no "and", no chained thoughts
- A creation story includes all actor-provided attributes — mandatory and optional. Split into an update story only when the intent explicitly describes editing an existing record.
- One intent action = one story. Do not decompose a single action into sub-steps.
- "depends_on" lists IDs of stories (existing or new in this batch) that must exist first
- "depends_on" is ALWAYS a YAML sequence, even for a single dependency — `depends_on: [<story-id>]`,
  NEVER a bare string like `depends_on: <story-id>`
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
    let stripped = strip_code_fence(&raw);
    serde_yaml::from_str::<UserStories>(&stripped)
        .map_err(|source| LlmError::YamlParse { source, raw: stripped })
}

fn domain_extraction_prompt(intent: &str, stories: &[UserStory]) -> String {
    let stories_text = stories
        .iter()
        .map(|s| format!("- As a {}, I want to {}, so that {}", s.as_a, s.want, s.so_that))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"You are identifying domain vocabulary from a set of user stories.

Original behavioral intent these stories were derived from:
{intent}

Stories derived from it:
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
        intent = intent,
        stories_text = stories_text,
    )
}

/// Live-verified bug this fixes: with only `want` text ("register in the system" — no entity
/// named at all) this extracted `User`/`UserRegistered` instead of `Manufacturer`/
/// `ManufacturerRegistered`, an information-loss bug rather than a reasoning failure — the
/// original intent statement ("Manufacturers must be registered...") named the entity
/// explicitly, but that text never reached this call. Passing the intent and the full story
/// (as_a/want/so_that, not want alone) gives the model everything a human reviewer would
/// actually read before naming the domain vocabulary.
pub fn extract_domain_from_stories(
    client: &LlmClient,
    intent: &str,
    stories: &[UserStory],
) -> Result<DomainRegistry, LlmError> {
    if stories.is_empty() {
        return Ok(DomainRegistry::default());
    }
    let raw = client.complete(&domain_extraction_prompt(intent, stories))?;
    let stripped = strip_code_fence(&raw);
    serde_yaml::from_str::<DomainRegistry>(&stripped)
        .map_err(|source| LlmError::YamlParse { source, raw: stripped })
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
    let stripped = strip_code_fence(&raw);
    serde_json::from_str::<Vec<String>>(&stripped)
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
    let stripped = strip_code_fence(&raw);
    serde_json::from_str::<Vec<String>>(&stripped)
        .map_err(|e| LlmError::JsonParse(format!("{e}. Raw was: {raw}")))
}

