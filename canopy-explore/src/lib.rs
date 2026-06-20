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
        if self.debug {
            eprintln!("\n╔══ LLM INPUT ═══════════════════════════════════════════╗");
            eprintln!("{prompt}");
            eprintln!("╚════════════════════════════════════════════════════════╝\n");
        }

        let (text, json) = match self.provider {
            LlmProvider::Anthropic => self.call_anthropic(prompt)?,
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

    fn call_anthropic(&self, prompt: &str) -> Result<(String, serde_json::Value), ExploreError> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
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
