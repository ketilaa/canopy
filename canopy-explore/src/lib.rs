use canopy_core::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExploreError {
    #[error("ANTHROPIC_API_KEY environment variable is not set. Export it before running canopy.")]
    MissingApiKey,
    #[error("HTTP request to Anthropic API failed: {0}")]
    Http(String),
    #[error("Failed to parse JSON from Anthropic response: {0}")]
    JsonParse(String),
    #[error("Failed to parse YAML from LLM response: {source}\nRaw LLM output:\n{raw}")]
    YamlParse {
        #[source]
        source: serde_yaml::Error,
        raw: String,
    },
    #[error("Unexpected Anthropic response shape: {0}")]
    UnexpectedShape(String),
}

pub struct LlmClient {
    api_key: String,
    model: String,
    debug: bool,
}

impl LlmClient {
    pub fn from_env(debug: bool) -> Result<Self, ExploreError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ExploreError::MissingApiKey)?;
        Ok(Self {
            api_key,
            model: "claude-sonnet-4-6".to_string(),
            debug,
        })
    }

    pub fn complete(&self, prompt: &str) -> Result<String, ExploreError> {
        if self.debug {
            eprintln!("\n╔══ LLM INPUT ═══════════════════════════════════════════╗");
            eprintln!("{prompt}");
            eprintln!("╚════════════════════════════════════════════════════════╝\n");
        }

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [{"role": "user", "content": prompt}]
        });

        let response = ureq::post("https://api.anthropic.com/v1/messages")
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
                format!("expected content[0].text string, got: {json}")
            ))?
            .to_string();

        if self.debug {
            let model = json["model"].as_str().unwrap_or(&self.model);
            let input_tokens = json["usage"]["input_tokens"].as_u64().unwrap_or(0);
            let output_tokens = json["usage"]["output_tokens"].as_u64().unwrap_or(0);
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

fn requirements_prompt(idea: &Idea, vision: &Vision, answers: &[AnsweredQuestion]) -> String {
    let vision_yaml = serde_yaml::to_string(vision).unwrap_or_default();
    let qa = format_answers(answers);
    format!(
        r#"You are an experienced software architect.

Project vision:
{vision_yaml}

Original idea: {description}

Q&A context:
{qa}

Generate comprehensive requirements as YAML:
functional:
  - <what the system must do — one concrete requirement per item>
non_functional:
  - <quality attributes: performance, security, scalability, accessibility, etc.>

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        vision_yaml = vision_yaml,
        description = idea.description,
        qa = qa
    )
}

fn domain_prompt(vision: &Vision, requirements: &Requirements) -> String {
    let vision_yaml = serde_yaml::to_string(vision).unwrap_or_default();
    let req_yaml = serde_yaml::to_string(requirements).unwrap_or_default();
    format!(
        r#"You are an experienced software architect applying Domain-Driven Design.

Project vision:
{vision_yaml}

Requirements:
{req_yaml}

Generate a DDD domain model as YAML:
entities:
  - EntityName
entities_detail:
  - name: EntityName
    attributes:
      - fieldName: type description
events:
  - SomethingHappened
relationships:
  - "EntityA owns many EntityB"
  - "EntityC collaborates with EntityD"

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        vision_yaml = vision_yaml,
        req_yaml = req_yaml
    )
}

fn architecture_prompt(vision: &Vision, requirements: &Requirements, domain: &DomainModel) -> String {
    let vision_yaml = serde_yaml::to_string(vision).unwrap_or_default();
    let req_yaml = serde_yaml::to_string(requirements).unwrap_or_default();
    let domain_yaml = serde_yaml::to_string(domain).unwrap_or_default();
    format!(
        r#"You are an experienced software architect.

Project vision:
{vision_yaml}

Requirements:
{req_yaml}

Domain model:
{domain_yaml}

Recommend a concrete, specific architecture as YAML. Use real names (e.g. "SvelteKit" not "a frontend framework"):
frontend:
  framework: <specific framework>
backend:
  framework: <specific framework>
database: <specific database>
deployment: <specific hosting approach>
reasoning:
  - <why this frontend choice fits the project's users and goals>
  - <why this backend choice fits the requirements>
  - <why this database fits the domain model>
  - <why this deployment approach fits the team and scale>

Return ONLY valid YAML. No explanation. No code fences. No markdown."#,
        vision_yaml = vision_yaml,
        req_yaml = req_yaml,
        domain_yaml = domain_yaml
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

pub fn generate_requirements(
    client: &LlmClient,
    idea: &Idea,
    vision: &Vision,
    answers: &[AnsweredQuestion],
) -> Result<Requirements, ExploreError> {
    let raw = client.complete(&requirements_prompt(idea, vision, answers))?;
    serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

pub fn generate_domain(
    client: &LlmClient,
    vision: &Vision,
    requirements: &Requirements,
) -> Result<DomainModel, ExploreError> {
    let raw = client.complete(&domain_prompt(vision, requirements))?;
    serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}

pub fn generate_architecture(
    client: &LlmClient,
    vision: &Vision,
    requirements: &Requirements,
    domain: &DomainModel,
) -> Result<Architecture, ExploreError> {
    let raw = client.complete(&architecture_prompt(vision, requirements, domain))?;
    serde_yaml::from_str(&raw)
        .map_err(|source| ExploreError::YamlParse { source, raw })
}
