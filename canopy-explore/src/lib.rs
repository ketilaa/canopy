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
    pub fn from_env(debug: bool) -> Result<Self, ExploreError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ExploreError::MissingApiKey)?;
        Ok(Self {
            api_key,
            model: "claude-sonnet-4-6".to_string(),
            debug,
            provider: LlmProvider::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
        })
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
        r#"You are an experienced software architect helping a developer explore a new software idea.

The developer described this idea:
{description}

Generate 3 to 10 targeted follow-up questions that will reveal the most important unknowns.
Focus on: scope boundaries, target users, key constraints, non-functional requirements, and critical technical decisions.

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
project: <short project name, 1-4 words>
problem: <the core problem being solved, 1-2 sentences>
users:
  - <primary user type>
  - <secondary user type if applicable>
goals:
  - <key goal 1>
  - <key goal 2>
  - <key goal 3>

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
        registry.entities.join(", ")
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
        registry.entities.join(", ")
    };
    let known_events = if registry.events.is_empty() {
        "(none yet)".to_string()
    } else {
        registry.events.join(", ")
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
        registry.entities.join(", ")
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

fn scaffold_plan_prompt(project_name: &str, group_id: &str, comp_arch: &ComponentArchitecture) -> String {
    let arch_yaml = serde_yaml::to_string(comp_arch).unwrap_or_default();
    format!(
        r#"You are a software architect generating project scaffold commands.

Project name: {project_name}
Java groupId / package: {group_id}
Component architecture:
{arch_yaml}

Generate the exact scaffold commands to initialize each component. Use only official ecosystem tools.

Exact syntax reference — copy these patterns, do not invent flags:

Next.js:
  npx create-next-app@latest <name> --typescript --tailwind --app --no-git

Vite (React, Vue, Svelte, etc. — NOT Angular):
  npm create vite@latest <name> -- --template <template>
  Valid templates: vanilla, vanilla-ts, vue, vue-ts, react, react-ts, react-swc,
                   react-swc-ts, preact, preact-ts, lit, lit-ts, svelte, svelte-ts,
                   solid, solid-ts, qwik, qwik-ts
  NOTE: flags MUST come after the bare "--" separator when using npm create

Angular (use ng new, NOT Vite — Vite's Angular template is interactive and cannot be scripted):
  npx @angular/cli@latest new <name> --style=css --routing --skip-git --no-interactive
  Replace --style=css with --style=scss or --style=less if preferred
  Always include --skip-git — the project is already inside a git repository

Spring Boot microservice (Java) — PREFERRED for any backend service or API:
  curl -G https://start.spring.io/starter.tgz \
    -d dependencies=web,actuator -d language=java -d type=maven-project \
    -d bootVersion=3.4.1 \
    -d groupId=<groupId> -d artifactId=<artifactId> -d name=<artifactId> \
    | tar -xzvf - -C <artifactId>
  Add dependencies as needed: web,actuator,data-jpa,postgresql,kafka,security,validation
  Always mkdir <artifactId> first, then extract into it with -C <artifactId>

Spring Boot microservice (Kotlin) — use instead of Java when project uses Kotlin:
  curl -G https://start.spring.io/starter.tgz \
    -d dependencies=web,actuator -d language=kotlin -d type=gradle-project \
    -d bootVersion=3.4.1 \
    -d groupId=<groupId> -d artifactId=<artifactId> -d name=<artifactId> \
    | tar -xzvf - -C <artifactId>

Maven plain Java library (NOT for microservices — only for shared libs with no framework):
  mvn archetype:generate -DgroupId=<groupId> -DartifactId=<artifactId> \
    -DarchetypeArtifactId=maven-archetype-quickstart -DarchetypeVersion=1.4 \
    -DinteractiveMode=false

Gradle (Kotlin/Java):
  gradle init --type kotlin-application --dsl kotlin --no-incubating

Rust:
  cargo new <name>

.NET:
  dotnet new webapi -n <name>

Python:
  mkdir -p <name> && touch <name>/main.py <name>/requirements.txt

For each component in the architecture, produce one entry.

Return ONLY valid YAML:
commands:
  - label: <human-readable component name, e.g. "storefront (Next.js)">
    command: <exact command with all flags — follow the syntax reference above exactly>
    working_dir: <directory to run from, relative to project root, use "." for root>
    creates: <directory or file the command creates, e.g. "storefront/">

Rules:
- Follow the syntax reference exactly — do not paraphrase or invent flags
- Use the provided groupId for all Java/Kotlin components; derive artifactId from the component name
- The 'creates' value must be the actual directory/file name the command produces
- Omit components that have no scaffold tool (document them in a comment in the label instead)

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        project_name = project_name,
    )
}

pub fn generate_scaffold_plan(
    client: &LlmClient,
    project_name: &str,
    group_id: &str,
    comp_arch: &ComponentArchitecture,
) -> Result<ScaffoldPlan, ExploreError> {
    let raw = client.complete(&scaffold_plan_prompt(project_name, group_id, comp_arch))?;
    if let Ok(plan) = serde_yaml::from_str::<ScaffoldPlan>(&raw) {
        return Ok(plan);
    }
    // Fallback for wrapped responses
    let value: serde_yaml::Value = serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw: raw.clone() })?;
    for key in &["scaffold_plan", "scaffold"] {
        if let Some(inner) = value.get(*key) {
            if let Ok(plan) = serde_yaml::from_value::<ScaffoldPlan>(inner.clone()) {
                return Ok(plan);
            }
        }
    }
    serde_yaml::from_str::<ScaffoldPlan>(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
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
