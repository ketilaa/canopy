use canopy_core::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExploreError {
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

    pub fn complete(&self, prompt: &str) -> Result<String, ExploreError> {
        self.complete_with_max_tokens(prompt, 4096)
    }

    /// Use for code generation where output can be significantly larger than planning artifacts.
    pub fn complete_large(&self, prompt: &str) -> Result<String, ExploreError> {
        self.complete_with_max_tokens(prompt, 8192)
    }

    fn complete_with_max_tokens(&self, prompt: &str, max_tokens: u32) -> Result<String, ExploreError> {
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

    fn call_anthropic(&self, prompt: &str, max_tokens: u32) -> Result<(String, serde_json::Value), ExploreError> {
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
            .map_err(|e| ExploreError::Http(e.to_string()))?;
        let json: serde_json::Value = response
            .into_json()
            .map_err(|e| ExploreError::JsonParse(e.to_string()))?;
        let text = json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| ExploreError::UnexpectedShape(
                format!("expected content[0].text, got: {json}")
            ))?
            .to_string();
        Ok((text, json))
    }

    fn call_openai_compatible(&self, prompt: &str) -> Result<(String, serde_json::Value), ExploreError> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}]
        });
        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = ureq::post(&url)
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| ExploreError::Http(e.to_string()))?;
        let json: serde_json::Value = response
            .into_json()
            .map_err(|e| ExploreError::JsonParse(e.to_string()))?;
        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| ExploreError::UnexpectedShape(
                format!("expected choices[0].message.content, got: {json}")
            ))?
            .to_string();
        Ok((text, json))
    }
}

fn format_answers(answers: &[AnsweredQuestion]) -> String {
    if answers.is_empty() {
        return "(no additional context provided)".to_string();
    }
    answers.iter()
        .map(|a| format!("Q: {}\nA: {}", a.question, a.answer))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn questions_prompt(idea: &Idea) -> String {
    format!(
        r#"You are helping to capture the vision behind a new software idea.

The developer described this idea:
{description}

Ask only the questions genuinely needed to write a clear vision statement.
If the description is already clear enough, return an empty list.
Maximum 2 questions.

Focus ONLY on: what core problem the system solves for its users, and what the system is meant to do.

DO NOT ask about: who specific user types or roles are, system boundaries, technology stack,
security, compliance, integrations, performance, scalability, deployment, or architecture.
All of those emerge later through behavioral requirements and architecture decisions.

Return ONLY a JSON object. No explanation. No code fences. Exact format:
{{"questions": ["question 1", "question 2", "question 3"]}}"#,
        description = idea.description
    )
}

fn vision_prompt(idea: &Idea, answers: &[AnsweredQuestion]) -> String {
    let qa = format_answers(answers);
    format!(
        r#"You are an experienced software architect.

Software idea: {description}

Q&A context:
{qa}

Write a project vision as YAML with this exact structure:
project: <short readable name, 2-4 plain words, no abbreviations, no CamelCase concatenation — e.g. "Merchant Shop" not "MerchantProxyEcomm">
problem: <the core problem being solved, 1-2 sentences>
goals:
  - <high-level outcome the system should achieve — not a feature, not an implementation detail>
  - <another outcome>
  - <another outcome>

Goals must be durable, high-level outcomes — still valid even if specific features change.
Do not pre-commit to specific features, technologies, or integrations in goals.

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        description = idea.description,
        qa = qa
    )
}

fn delivery_intents_prompt(idea: &Idea, vision: &Vision, answers: &[AnsweredQuestion]) -> String {
    let vision_yaml = serde_yaml::to_string(vision).unwrap_or_default();
    let qa = format_answers(answers);
    format!(
        r#"You are an experienced product strategist and software architect.

Project vision:
{vision_yaml}

Original idea: {description}

Q&A context:
{qa}

Produce an ordered list of delivery intents as YAML. Each intent is a coherent slice of value that can be designed, built, and delivered independently. Order them from foundational to differentiating — earlier intents enable later ones.

intents:
  - title: <short action-oriented title, e.g. "User authentication">
    description: <what is built and how it works, 1-2 sentences>
    value: <the concrete user or business value this delivers>

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        vision_yaml = vision_yaml,
        description = idea.description,
        qa = qa
    )
}

fn architecture_principles_prompt(vision: &Vision, intents: &DeliveryIntents, answers: &[AnsweredQuestion]) -> String {
    let vision_yaml = serde_yaml::to_string(vision).unwrap_or_default();
    let intents_yaml = serde_yaml::to_string(intents).unwrap_or_default();
    let qa = format_answers(answers);
    format!(
        r#"You are an experienced software architect.

Project vision:
{vision_yaml}

Delivery intents:
{intents_yaml}

Q&A context:
{qa}

Capture architectural principles, constraints, and structural commitments for this system as YAML.

Do NOT name specific technologies, frameworks, or databases. Capture what is known now:
- principles: how the system should behave architecturally
- constraints: non-negotiable requirements (compliance, deployment environment, team expertise, integration mandates)
- structural_commitments: system-wide decisions that must be consistent across all delivery intents

CRITICAL: Output must start directly with "principles:" — do NOT add a top-level wrapper key.

principles:
  - <architectural principle, e.g. "Stateless application tier — no session state in services">
constraints:
  - <hard constraint, e.g. "Must deploy on-premise, no public cloud">
structural_commitments:
  deployment_topology: <e.g. "Modular monolith, extractable to microservices">
  integration_style: <e.g. "Event-driven integration between bounded contexts">
  data_ownership: <e.g. "Shared database with schema-per-module boundaries">

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        vision_yaml = vision_yaml,
        intents_yaml = intents_yaml,
        qa = qa
    )
}

pub fn generate_questions(client: &LlmClient, idea: &Idea) -> Result<ExploreQuestions, ExploreError> {
    let raw = client.complete(&questions_prompt(idea))?;
    serde_json::from_str(&raw)
        .map_err(|e| ExploreError::JsonParse(format!("{e}. Raw was: {raw}")))
}

pub fn generate_vision(
    client: &LlmClient,
    idea: &Idea,
    answers: &[AnsweredQuestion],
) -> Result<Vision, ExploreError> {
    let raw = client.complete(&vision_prompt(idea, answers))?;
    serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

pub fn generate_delivery_intents(
    client: &LlmClient,
    idea: &Idea,
    vision: &Vision,
    answers: &[AnsweredQuestion],
) -> Result<DeliveryIntents, ExploreError> {
    let raw = client.complete(&delivery_intents_prompt(idea, vision, answers))?;
    serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

pub fn generate_architecture_principles(
    client: &LlmClient,
    vision: &Vision,
    intents: &DeliveryIntents,
    answers: &[AnsweredQuestion],
) -> Result<ArchitecturePrinciples, ExploreError> {
    let raw = client.complete(&architecture_principles_prompt(vision, intents, answers))?;
    parse_architecture_principles(&raw)
}

fn domain_model_prompt(
    vision: &Vision,
    intents: &DeliveryIntents,
    principles: &ArchitecturePrinciples,
) -> String {
    let vision_yaml = serde_yaml::to_string(vision).unwrap_or_default();
    let intents_yaml = serde_yaml::to_string(intents).unwrap_or_default();
    let principles_yaml = serde_yaml::to_string(principles).unwrap_or_default();
    format!(
        r#"You are an experienced domain-driven design practitioner.

Project vision:
{vision_yaml}

Delivery intents:
{intents_yaml}

Architecture principles:
{principles_yaml}

Model the complete domain for this system. Cover ALL entities required across ALL delivery intents.

Return ONLY valid YAML:
entities:
  - <entity name, PascalCase>
entities_detail:
  - name: <entity name>
    attributes:
      - <fieldName>: <type and short description>
events:
  - <domain event, PascalCase past tense, e.g. UserRegistered>
relationships:
  - <plain English relationship, e.g. "User has many Orders">

Rules:
- Every entity in 'entities' must appear in 'entities_detail'
- Attributes use map format: fieldName: type description
- Event names are PascalCase past tense
- Relationships use plain English

Return ONLY valid YAML. No explanation. No code fences. No markdown."#
    )
}

pub fn generate_domain_model(
    client: &LlmClient,
    vision: &Vision,
    intents: &DeliveryIntents,
    principles: &ArchitecturePrinciples,
) -> Result<DomainModel, ExploreError> {
    let raw = client.complete(&domain_model_prompt(vision, intents, principles))?;
    serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

fn component_architecture_prompt(
    vision: &Vision,
    intents: &DeliveryIntents,
    principles: &ArchitecturePrinciples,
    registry: &DomainRegistry,
) -> String {
    let vision_yaml = serde_yaml::to_string(vision).unwrap_or_default();
    let principles_yaml = serde_yaml::to_string(principles).unwrap_or_default();
    let entities_summary = if registry.entities.is_empty() {
        "(none yet — domain accumulates through planning)".to_string()
    } else {
        registry.entities.iter()
            .map(|e| match e.description() {
                Some(d) => format!("{} — {}", e.name(), d),
                None => e.name().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let intents_yaml = serde_yaml::to_string(intents).unwrap_or_default();
    format!(
        r#"You are an experienced software architect deriving a component architecture.

Project vision:
{vision_yaml}

Architecture principles (MUST be respected — constraints are non-negotiable):
{principles_yaml}

Domain entities known so far: {entities_summary}

Delivery intents (full detail — use these to identify distinct components and services):
{intents_yaml}

Your task: identify every component this system requires and select a concrete technology for each.

Step 1 — derive components from the delivery intents and structural commitments:
- Read the structural_commitments carefully. If deployment_topology is microservices, name each service separately.
- Each delivery intent that touches a distinct bounded context is a candidate for its own backend service.
- If multiple intents describe separate user-facing surfaces (storefront vs backoffice), name each frontend app separately.
- Distinguish data stores (relational DBs, document stores) from messaging infrastructure (event buses, queues).
- Do not merge conceptually distinct things into one component to keep the list short.

Step 2 — select technologies:
- Every selection MUST satisfy ALL constraints and respect the structural commitments.
- Use current stable versions. Do not name specific version numbers for the deployment platform unless certain they are current.
- Apply domain entities as hints for where data lives.

Return ONLY valid YAML shaped to match the actual system — do not use a fixed template.
Use these categories as top-level keys (omit any that do not apply):

frontend_apps:
  - name: <app name, e.g. storefront>
    technology: <framework and version>
    purpose: <one line>

backend_services:
  - name: <service name derived from bounded context>
    technology: <runtime and framework>
    purpose: <one line>

data_stores:
  - name: <store name>
    technology: <database and version>
    owned_by: <service name(s)>

messaging:
  - name: <broker or bus name>
    technology: <technology and version>
    purpose: <one line>

deployment:
  platform: <orchestration platform>
  strategy: <one line describing how services are deployed>

reasoning:
  - <one entry per significant technology decision, citing the principle or constraint it satisfies>

Return ONLY valid YAML. No explanation. No code fences. No markdown."#
    )
}

pub fn generate_component_architecture(
    client: &LlmClient,
    vision: &Vision,
    intents: &DeliveryIntents,
    principles: &ArchitecturePrinciples,
    registry: &DomainRegistry,
) -> Result<ComponentArchitecture, ExploreError> {
    let raw = client.complete(&component_architecture_prompt(vision, intents, principles, registry))?;
    serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

fn adrs_prompt(comp_arch: &ComponentArchitecture, principles: &ArchitecturePrinciples) -> String {
    let arch_yaml = serde_yaml::to_string(comp_arch).unwrap_or_default();
    let principles_yaml = serde_yaml::to_string(principles).unwrap_or_default();
    format!(
        r#"You are an experienced software architect writing Architecture Decision Records.

Component architecture:
{arch_yaml}

Architecture principles:
{principles_yaml}

Write one ADR for each major technology decision in the component architecture.

Return ONLY a YAML list:
- title: <decision title, e.g. "Use PostgreSQL as primary database">
  decision: <the decision in one sentence>
  reason: <why this technology was chosen>
  alternatives:
    - <alternative that was considered>

Return ONLY valid YAML. No explanation. No code fences. No markdown."#
    )
}

pub fn generate_adrs(
    client: &LlmClient,
    comp_arch: &ComponentArchitecture,
    principles: &ArchitecturePrinciples,
) -> Result<Vec<Adr>, ExploreError> {
    let raw = client.complete(&adrs_prompt(comp_arch, principles))?;
    if let Ok(adrs) = serde_yaml::from_str::<Vec<Adr>>(&raw) {
        return Ok(adrs);
    }
    // Fallback: LLM may wrap the list in a key
    let value: serde_yaml::Value = serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw: raw.clone() })?;
    for key in &["adrs", "decisions", "records"] {
        if let Some(inner) = value.get(*key) {
            if let Ok(adrs) = serde_yaml::from_value::<Vec<Adr>>(inner.clone()) {
                return Ok(adrs);
            }
        }
    }
    serde_yaml::from_str::<Vec<Adr>>(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

fn intent_spec_prompt(
    intent: &DeliveryIntent,
    registry: &DomainRegistry,
    comp_arch: &ComponentArchitecture,
) -> String {
    let arch_yaml = serde_yaml::to_string(comp_arch).unwrap_or_default();
    let known_entities = if registry.entities.is_empty() {
        "(none yet — introduce what this intent requires)".to_string()
    } else {
        registry.entities.iter()
            .map(|e| match e.description() {
                Some(d) => format!("{} — {}", e.name(), d),
                None => e.name().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let known_events = if registry.events.is_empty() {
        "(none yet)".to_string()
    } else {
        registry.events.iter()
            .map(|e| match e.description() {
                Some(d) => format!("{} — {}", e.name(), d),
                None => e.name().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        r#"You are an experienced product and engineering lead writing a behavioral specification.

Intent to specify:
  title: {title}
  description: {description}
  value: {value}

Known entities from prior plans (reuse these names — do not redefine, extend only if needed):
{known_entities}

Known events from prior plans:
{known_events}

Component architecture:
{arch_yaml}

Write a precise behavioral specification for this delivery intent.

Return ONLY valid YAML:
intent_ref: "{title}"
scenarios:
  - id: <short-slug-001>
    name: <scenario name>
    given:
      - <precondition using entity names above where applicable>
    when: <user action or system event>
    then:
      - <observable outcome>
    constraints:
      - <optional: measurable bound, e.g. "Response under 200ms at p99">
out_of_scope:
  - <what is explicitly NOT covered by this intent>
open_questions:
  - <ambiguity that must be resolved before or during implementation>

Rules:
- Reuse known entity names verbatim (User not "the user", Session not "auth token")
- Introduce new entity names only when this intent genuinely requires a new concept
- Each scenario covers ONE observable behavior
- constraints list may be empty for scenarios with no measurable bounds
- Be explicit about out_of_scope to prevent creep

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        title = intent.title,
        description = intent.description,
        value = intent.value,
    )
}

pub fn generate_intent_spec(
    client: &LlmClient,
    intent: &DeliveryIntent,
    registry: &DomainRegistry,
    comp_arch: &ComponentArchitecture,
) -> Result<IntentSpec, ExploreError> {
    let raw = client.complete(&intent_spec_prompt(intent, registry, comp_arch))?;
    if let Ok(spec) = serde_yaml::from_str::<IntentSpec>(&raw) {
        return Ok(spec);
    }
    // Fallback for wrapped responses
    let value: serde_yaml::Value = serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw: raw.clone() })?;
    for key in &["spec", "intent_spec", "specification"] {
        if let Some(inner) = value.get(*key) {
            if let Ok(spec) = serde_yaml::from_value::<IntentSpec>(inner.clone()) {
                return Ok(spec);
            }
        }
    }
    serde_yaml::from_str::<IntentSpec>(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

fn implementation_plan_prompt(
    intent: &DeliveryIntent,
    intent_index: usize,
    spec: &IntentSpec,
    registry: &DomainRegistry,
    comp_arch: &ComponentArchitecture,
    all_intents: &DeliveryIntents,
    answered_questions: &[AnsweredQuestion],
) -> String {
    let spec_yaml = serde_yaml::to_string(spec).unwrap_or_default();
    let known_entities = if registry.entities.is_empty() {
        "(none yet)".to_string()
    } else {
        registry.entities.iter()
            .map(|e| match e.description() {
                Some(d) => format!("{} — {}", e.name(), d),
                None => e.name().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let arch_yaml = serde_yaml::to_string(comp_arch).unwrap_or_default();
    let all_intents_yaml = serde_yaml::to_string(all_intents).unwrap_or_default();
    let qa = format_answers(answered_questions);
    format!(
        r#"You are an experienced software architect decomposing a delivery intent into implementation tasks.

Delivery intent (index {intent_index}):
  title: {title}
  description: {description}

Intent specification:
{spec_yaml}

Known entities from prior plans (reuse verbatim, extend only if this intent adds new ones):
{known_entities}

Component architecture:
{arch_yaml}

All delivery intents (for dependency analysis):
{all_intents_yaml}

Resolved open questions:
{qa}

Generate an implementation plan as YAML. Tasks must be ordered from foundational to dependent.

Return ONLY valid YAML:
intent_ref: "{title}"
intent_index: {intent_index}
status: draft
depends_on_intents:
  - <index of a prior delivery intent this depends on, or leave empty>
domain_scope:
  entities:
    - <entity from domain model involved in this intent>
  events:
    - <event from domain model triggered in this intent>
  relationships:
    - <relationship relevant to this intent>
tasks:
  - id: task-001
    title: <what to build — one logical thing>
    task_type: <schema | api_contract | implementation | test | integration>
    inputs:
      - <task id or artifact this depends on, or leave empty>
    outputs:
      - <file path that will be created>
    acceptance_criteria_refs:
      - <scenario id from spec>
    estimated_complexity: <low | medium | high>
    blocking: <true if subsequent tasks cannot start until this completes>
reasoning:
  - <why tasks are ordered and scoped as they are>
open_questions:
  - question: <ambiguity>
    blocking: <true if implementation cannot proceed without answer>
    default_assumption: <assumption to use if not blocking>
    answer: null

Rules:
- Each task does ONE logical thing (schema OR endpoint OR test — not both)
- Schema and migration tasks are typically blocking
- Test tasks reference the scenarios they verify via acceptance_criteria_refs
- If a question is blocking, leave answer as null
- depends_on_intents should list intent indices whose artifacts this intent requires

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        title = intent.title,
        description = intent.description,
        intent_index = intent_index,
    )
}

pub fn generate_implementation_plan(
    client: &LlmClient,
    intent: &DeliveryIntent,
    intent_index: usize,
    spec: &IntentSpec,
    registry: &DomainRegistry,
    comp_arch: &ComponentArchitecture,
    all_intents: &DeliveryIntents,
    answered_questions: &[AnsweredQuestion],
) -> Result<ImplementationPlan, ExploreError> {
    let raw = client.complete(&implementation_plan_prompt(
        intent, intent_index, spec, registry, comp_arch, all_intents, answered_questions,
    ))?;
    if let Ok(plan) = serde_yaml::from_str::<ImplementationPlan>(&raw) {
        return Ok(plan);
    }
    // Fallback for wrapped responses
    let value: serde_yaml::Value = serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw: raw.clone() })?;
    for key in &["implementation_plan", "plan"] {
        if let Some(inner) = value.get(*key) {
            if let Ok(plan) = serde_yaml::from_value::<ImplementationPlan>(inner.clone()) {
                return Ok(plan);
            }
        }
    }
    serde_yaml::from_str::<ImplementationPlan>(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

fn user_stories_prompt(
    vision: &Vision,
    domain: Option<&DomainRegistry>,
    comp_arch: Option<&ComponentArchitecture>,
) -> String {
    let vision_yaml = serde_yaml::to_string(vision).unwrap_or_default();
    let domain_section = domain
        .map(|d| format!("Domain vocabulary:\n{}", serde_yaml::to_string(d).unwrap_or_default()))
        .unwrap_or_default();
    let arch_section = comp_arch
        .map(|a| format!("Component architecture:\n{}", serde_yaml::to_string(a).unwrap_or_default()))
        .unwrap_or_default();
    format!(
        r#"You are an experienced product strategist writing user stories for a software project.

Vision:
{vision_yaml}

{domain_section}

{arch_section}

Generate a complete set of user stories covering the core functionality implied by the vision.

Rules:
- Assign each story a short, stable ID using a domain-area prefix and zero-padded number, e.g. auth-001, catalog-002, cart-001
- Use only actors that appear in the vision's users list
- The "so_that" must state a concrete business or user benefit — never a technical detail
- depends_on lists IDs of stories that must exist before this story can be built
- Order stories from foundational (no dependencies) to differentiating (many dependencies)
- Omit stories for infrastructure, deployment, or internal tooling

Return ONLY valid YAML in this exact shape. No explanation. No code fences. No markdown.

stories:
  - id: <area-NNN>
    as_a: <role from vision users>
    want: <capability>
    so_that: <concrete benefit>
    depends_on: []"#,
        vision_yaml = vision_yaml,
        domain_section = domain_section,
        arch_section = arch_section,
    )
}

pub fn generate_user_stories(
    client: &LlmClient,
    vision: &Vision,
    domain: Option<&DomainRegistry>,
    comp_arch: Option<&ComponentArchitecture>,
) -> Result<UserStories, ExploreError> {
    let raw = client.complete_large(&user_stories_prompt(vision, domain, comp_arch))?;
    if let Ok(stories) = serde_yaml::from_str::<UserStories>(&raw) {
        return Ok(stories);
    }
    serde_yaml::from_str::<UserStories>(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
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
- Assign each story a short stable ID: use a lowercase domain-area prefix and zero-padded number (e.g. catalog-001, product-003)
- Choose the prefix from the domain area the story belongs to, not from the intent wording
- The next ID number must be higher than any existing ID with the same prefix
- Reuse a known role if it fits; introduce a new role only when genuinely needed
- Use DDD and domain language in the "want" field — prefer domain verbs (register, activate, promote,
  publish, place, ship) over CRUD verbs (add, create, update, delete)
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
) -> Result<UserStories, ExploreError> {
    let raw = client.complete_large(&stories_from_intent_prompt(
        intent, context, existing_stories, roles,
    ))?;
    serde_yaml::from_str::<UserStories>(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
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
  Use PascalCase singular nouns (Product, Order, Customer, Money, Address).
  Include only real-world domain concepts — things that exist in the business domain.
  Never include: service names (ProductRegistry, CatalogService), infrastructure (Database, EventBus),
  UI concepts (Form, Page), or technical constructs. If it ends in "Service", "Registry",
  "Repository", "Manager", or "Handler" it is not a domain entity.
  Prefer domain language over CRUD language: "Order" not "OrderRecord", "Product" not "ProductItem".

Events: things that happened to a specific entity, named in past tense.
  Naming rule — strictly enforced: every event name MUST start with the name of the entity it belongs to.

  Two kinds of events only:
  1. Lifecycle events — created, updated, deleted:
       ProductCreated, ProductUpdated, ProductDeleted
     Any field-level change (uploading an image, editing a description, changing a price)
     is just ProductUpdated — do NOT create a separate event per field.
  2. Business operation events — meaningful state transitions or domain actions:
       ProductPromotedToCatalog, ProductActivated, ProductDeactivated,
       OrderPlaced, OrderShipped, OrderCancelled, CustomerRegistered
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
) -> Result<DomainRegistry, ExploreError> {
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
        .map_err(|source| ExploreError::YamlParse { source, raw: stripped.to_string() })
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

pub fn suggest_domain_entities(client: &LlmClient, idea: &Idea) -> Result<Vec<String>, ExploreError> {
    let raw = client.complete(&domain_bootstrap_prompt(idea))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<Vec<String>>(stripped)
        .map_err(|e| ExploreError::JsonParse(format!("{e}. Raw was: {raw}")))
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

Return ONLY a JSON array of strings. No explanation. No code fences.
["role one", "role two"]"#,
        description = idea.description
    )
}

pub fn suggest_roles(client: &LlmClient, idea: &Idea) -> Result<Vec<String>, ExploreError> {
    let raw = client.complete(&roles_bootstrap_prompt(idea))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<Vec<String>>(stripped)
        .map_err(|e| ExploreError::JsonParse(format!("{e}. Raw was: {raw}")))
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

Identify the architectural questions NOT YET answered that MUST be resolved before a specification can be written for this story.

Include ALL of:
1. Structural questions — service ownership, data responsibility, integration contracts, event design, API boundaries
2. UI questions — if the story has a human actor performing an action, there must be a frontend component
   through which they act. Ask what UI delivers this capability and propose it as a new service if not yet decided.
3. Tech stack questions — for every new service or frontend introduced, what technology should it be built with?
   This is MANDATORY — never omit a tech stack proposal for a newly introduced service or frontend.
   Suggest the most pragmatic and common choice, but a human will decide before accepting.
4. Infrastructure questions — if not yet decided:
   - Persistent storage: what database does each service use to store its data?
     Propose one per service that owns data. Suggest the most appropriate type (relational, document, etc.)
   - Event infrastructure: if the story involves publishing or subscribing to events,
     what event broker/bus is used? Propose it if not yet decided.

Naming rules — strictly enforced:
- Service and infrastructure component names: kebab-case only (product-registry, catalog-service, backoffice, storefront, redpanda, postgresql)
  Never append "Service", "DB", or "Database" as a suffix to service names.
- Domain event names: PascalCase past tense, prefixed with the entity name (ProductCreated, OrderPlaced, CustomerRegistered)
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
) -> Result<ProposedAdrs, ExploreError> {
    let raw = client.complete_large(&architectural_questions_prompt(story, existing_adrs, services))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_yaml::from_str::<ProposedAdrs>(stripped)
        .map_err(|source| ExploreError::YamlParse { source, raw: stripped.to_string() })
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

## Creation story detection

This is a CREATION STORY if the want contains a creation verb (register, create, add, onboard,
submit, publish) OR if the domain events include a {{Entity}}Created event for an entity in
the want statement.

If this is a creation story, you MUST output an entity_schema section before scenarios.
If this is NOT a creation story, omit entity_schema entirely.

### entity_schema rules

Identify the primary entity being created and define its fields in three categories:

system_generated — fields the system sets automatically; the actor never provides these:
  - Always include: id (uuid), createdAt (datetime)
  - Always include: modifiedAt (datetime) — null at creation, updated on every write
  - Include business-operation timestamps for any domain event that implies a later state
    transition on this entity (e.g. ProductPromotedToCatalog → promotedAt datetime,
    ProductActivated → activatedAt datetime). Set these to null at creation.
  - Do not include actor-provided fields here.

mandatory — fields the actor MUST provide when registering the entity:
  - These are the minimum data required for the entity to exist in the domain.
  - Infer from domain context, industry norms, and common sense for this entity type.
  - Do not include system_generated fields here.

optional — fields the actor MAY provide; nullable or defaulted:
  - These enrich the entity but are not required for it to exist.

Field format: name (camelCase), type (uuid | string | integer | decimal | boolean | datetime),
description (one sentence).

### Scenario grounding rule (CRITICAL)

When entity_schema is present, BDD scenarios MUST be grounded in it:
- "when" MUST explicitly name the mandatory fields the actor submits
- "then" MUST reference at least the system-generated fields set at creation
  (e.g. "the system assigns an id and sets createdAt to the current timestamp")
- Also include a scenario for the missing-mandatory-field failure case

Write BDD scenarios (Given/When/Then) as acceptance criteria. Additional rules:
- Scenarios describe OBSERVABLE BEHAVIOR from the user's perspective — never internal API calls,
  HTTP verbs, JSON payloads, or implementation details
- Given describes the BUSINESS STATE before the action — never infrastructure health, service availability, or deployment topology
- When describes what the actor does — one action per scenario
- Then describes what the actor observes or what business state has changed
- Use the exact kebab-case service names defined above
- intent_ref must be exactly: {story_id}
- Scenario IDs must follow the pattern: {story_id}-01, {story_id}-02, etc.

Return ONLY valid YAML — no prose, no code fences.
YAML string rules — you MUST follow these to avoid parse errors:
- Any string value containing a colon (:) MUST be enclosed in double quotes
- Any list item ending with a question mark (?) MUST be enclosed in double quotes
- type values use plain strings only: string, integer, decimal, uuid, datetime, boolean — never angle-bracket generics like array<string>; use "[string]" instead

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
) -> Result<IntentSpec, ExploreError> {
    let raw = client.complete_large(&story_spec_prompt(story, adrs, services, domain))?;
    let stripped = raw
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let fixed = fix_yaml_colon_in_scalars(stripped);
    serde_yaml::from_str::<IntentSpec>(&fixed)
        .map_err(|source| ExploreError::YamlParse { source, raw: fixed })
}

/// Quote bare YAML scalar values that contain colons — a common LLM mistake that
/// causes serde_yaml to treat them as nested mappings.
fn fix_yaml_colon_in_scalars(yaml: &str) -> String {
    yaml.lines().map(|line| {
        // Match lines of the form: <indent><key>: <value> where value is unquoted and contains ':'
        if let Some(colon_pos) = line.find(": ") {
            let (key_part, rest) = line.split_at(colon_pos + 2);
            let value = rest.trim_end();
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

pub fn arch_needs_jvm(comp_arch: &ComponentArchitecture) -> bool {
    let value = &comp_arch.0;
    for section in &["frontend_apps", "backend_services"] {
        if let Some(seq) = value.get(*section).and_then(|v| v.as_sequence()) {
            for component in seq {
                if let Some(tech) = component.get("technology").and_then(|v| v.as_str()) {
                    let t = tech.to_lowercase();
                    if t.contains("spring") || t.contains("java") || t.contains("kotlin")
                        || t.contains("maven") || t.contains("gradle")
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub fn generate_scaffold_plan_static(group_id: &str, comp_arch: &ComponentArchitecture) -> ScaffoldPlan {
    let mut commands = Vec::new();
    let value = &comp_arch.0;

    for (section_key, working_dir) in &[
        ("frontend_apps", "frontend"),
        ("backend_services", "services"),
    ] {
        if let Some(seq) = value.get(*section_key).and_then(|v| v.as_sequence()) {
            for component in seq {
                let name = component.get("name").and_then(|v| v.as_str()).unwrap_or_default();
                let technology = component.get("technology").and_then(|v| v.as_str()).unwrap_or_default();
                if name.is_empty() || technology.is_empty() {
                    continue;
                }
                match technology_to_command(name, technology, group_id, working_dir) {
                    Some(cmd) => commands.push(cmd),
                    None => eprintln!("  (skipping '{name}': no scaffold template for '{technology}')"),
                }
            }
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

fn developer_prompt(
    intent: &DeliveryIntent,
    spec: &IntentSpec,
    plan: &ImplementationPlan,
    comp_arch: &ComponentArchitecture,
    roots_context: Option<&str>,
) -> String {
    let spec_yaml = serde_yaml::to_string(spec).unwrap_or_default();
    let plan_yaml = serde_yaml::to_string(plan).unwrap_or_default();
    let arch_yaml = serde_yaml::to_string(comp_arch).unwrap_or_default();
    let roots_section = match roots_context {
        Some(ctx) => format!("\nRepository context from Roots:\n{ctx}\n"),
        None => String::new(),
    };
    // Build pending task list (not yet completed)
    let pending: Vec<String> = plan.tasks.iter()
        .filter(|t| !t.completed)
        .map(|t| format!("  - [{}] {} (outputs: {})", t.id, t.title, t.outputs.join(", ")))
        .collect();
    let pending_summary = if pending.is_empty() {
        "  (all tasks completed)".to_string()
    } else {
        pending.join("\n")
    };
    format!(
        r#"You are an expert software developer implementing a delivery intent.

Intent: {title}
Description: {description}
{roots_section}
Component architecture:
{arch_yaml}

Behavioral specification:
{spec_yaml}

Implementation plan:
{plan_yaml}

Pending tasks to implement:
{pending_summary}

Generate ALL files required to implement the pending tasks. This is a greenfield implementation — write complete, production-quality file contents.

Return ONLY a JSON object. No explanation. No code fences. No markdown:
{{"files": [{{"path": "relative/path/to/file.ext", "content": "full file content"}}]}}

Rules:
- Use the technology stack from the component architecture
- Each file must be complete and syntactically correct — no stubs, no TODOs
- path is relative to the project root
- Implement exactly what the specification scenarios describe
- Do not generate files for tasks that are already completed
- Include tests in a separate file when the plan has test tasks
- Follow idiomatic conventions for the chosen technology"#,
        title = intent.title,
        description = intent.description,
    )
}

pub fn generate_files(
    client: &LlmClient,
    intent: &DeliveryIntent,
    spec: &IntentSpec,
    plan: &ImplementationPlan,
    comp_arch: &ComponentArchitecture,
    roots_context: Option<&str>,
) -> Result<DeveloperOutput, ExploreError> {
    let prompt = developer_prompt(intent, spec, plan, comp_arch, roots_context);
    let raw = client.complete_large(&prompt)?;
    // Try direct parse first
    if let Ok(output) = serde_json::from_str::<DeveloperOutput>(&raw) {
        return Ok(output);
    }
    // Strip possible markdown code fences
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<DeveloperOutput>(stripped)
        .map_err(|e| ExploreError::JsonParse(format!("{e}. Raw was: {raw}")))
}

fn validator_prompt(
    intent: &DeliveryIntent,
    spec: &IntentSpec,
    generated_files: &[(String, String)],
) -> String {
    let spec_yaml = serde_yaml::to_string(spec).unwrap_or_default();
    let files_section: String = generated_files.iter()
        .map(|(path, content)| format!("### {path}\n```\n{content}\n```"))
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        r#"You are a rigorous QA engineer validating an implementation against its behavioral specification.

Intent: {title}

Behavioral specification:
{spec_yaml}

Implemented files:
{files_section}

For EACH scenario in the specification, determine whether the implementation satisfies it.

Return ONLY a JSON object. No explanation. No code fences. No markdown:
{{
  "intent_ref": "{title}",
  "results": [
    {{
      "scenario_id": "<scenario id>",
      "scenario_name": "<scenario name>",
      "passed": true,
      "reasoning": "<why this scenario passes or fails>",
      "issues": ["<specific issue if failed>"]
    }}
  ]
}}

Rules:
- passed is true only if the implementation fully satisfies ALL given/when/then conditions and constraints
- reasoning must cite specific code evidence (file name, line context) when marking passed
- issues must list exactly what is missing or wrong when passed is false
- Cover every scenario — no omissions"#,
        title = intent.title,
    )
}

pub fn validate_spec(
    client: &LlmClient,
    intent: &DeliveryIntent,
    spec: &IntentSpec,
    generated_files: &[(String, String)],
) -> Result<ValidationReport, ExploreError> {
    let prompt = validator_prompt(intent, spec, generated_files);
    let raw = client.complete(&prompt)?;
    let stripped = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let mut report: ValidationReport = serde_json::from_str(stripped)
        .map_err(|e| ExploreError::JsonParse(format!("{e}. Raw was: {raw}")))?;
    report.recompute_totals();
    Ok(report)
}

fn parse_architecture_principles(raw: &str) -> Result<ArchitecturePrinciples, ExploreError> {
    if let Ok(ap) = serde_yaml::from_str::<ArchitecturePrinciples>(raw) {
        return Ok(ap);
    }
    // Fallback: LLM sometimes wraps output in a top-level key despite instructions.
    let value: serde_yaml::Value = serde_yaml::from_str(raw)
        .map_err(|source| ExploreError::YamlParse { source, raw: raw.to_string() })?;
    for key in &["architecture_principles", "architecture", "principles_doc"] {
        if let Some(inner) = value.get(*key) {
            return serde_yaml::from_value(inner.clone())
                .map_err(|source| ExploreError::YamlParse { source, raw: raw.to_string() });
        }
    }
    serde_yaml::from_str::<ArchitecturePrinciples>(raw)
        .map_err(|source| ExploreError::YamlParse { source, raw: raw.to_string() })
}
