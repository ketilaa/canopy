use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idea {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adr {
    pub title: String,
    pub decision: String,
    pub reason: String,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub given: Vec<String>,
    pub when: String,
    pub then: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldValidation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<FieldValidation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub entity: String,
    #[serde(default)]
    pub system_generated: Vec<FieldDef>,
    #[serde(default)]
    pub mandatory: Vec<FieldDef>,
    #[serde(default)]
    pub optional: Vec<FieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentSpec {
    pub intent_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_schema: Option<EntitySchema>,
    pub scenarios: Vec<Scenario>,
    #[serde(default)]
    pub out_of_scope: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

/// A domain entity with an optional human-curated description.
/// Serializes as a plain string when there is no description (backward-compatible),
/// or as a map `{name, description}` when a description is present.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DomainEntity {
    Simple(String),
    Described { name: String, description: String },
}

impl DomainEntity {
    pub fn name(&self) -> &str {
        match self {
            Self::Simple(n) => n,
            Self::Described { name, .. } => name,
        }
    }
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Simple(_) => None,
            Self::Described { description, .. } => Some(description),
        }
    }
}

/// A domain event with an optional human-curated description.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DomainEvent {
    Simple(String),
    Described { name: String, description: String },
}

impl DomainEvent {
    pub fn name(&self) -> &str {
        match self {
            Self::Simple(n) => n,
            Self::Described { name, .. } => name,
        }
    }
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Simple(_) => None,
            Self::Described { description, .. } => Some(description),
        }
    }
}

/// A user role with an optional human-curated description.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Role {
    Simple(String),
    Described { name: String, description: String },
}

impl Role {
    pub fn name(&self) -> &str {
        match self {
            Self::Simple(n) => n,
            Self::Described { name, .. } => name,
        }
    }
    pub fn description(&self) -> Option<&str> {
        match self {
            Self::Simple(_) => None,
            Self::Described { description, .. } => Some(description),
        }
    }
}

/// Accumulated entity and event vocabulary across all planned delivery intents.
/// Built incrementally by `canopy intent` — no upfront global modeling required.
/// In repository mode, Roots is the authoritative source and supersedes this file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainRegistry {
    #[serde(default)]
    pub entities: Vec<DomainEntity>,
    #[serde(default)]
    pub events: Vec<DomainEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldCommand {
    pub label: String,
    pub command: String,
    pub working_dir: String,
    pub creates: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldPlan {
    #[serde(default)]
    pub generated_at: String,
    pub commands: Vec<ScaffoldCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum StoryStatus {
    #[default]
    Draft,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStory {
    pub id: String,
    pub as_a: String,
    pub want: String,
    pub so_that: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub status: StoryStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserStories {
    pub stories: Vec<UserStory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RolesRegistry {
    #[serde(default)]
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceEntry {
    pub name: String,
    #[serde(default)]
    pub responsibilities: Vec<String>,
    /// Technology stack decided via ADR (e.g. "Spring Boot 4.1.0", "Angular", "React + Vite")
    #[serde(default)]
    pub technology: Option<String>,
    /// "frontend" | "service" — drives scaffold working directory
    #[serde(default)]
    pub component_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServicesRegistry {
    #[serde(default)]
    pub services: Vec<ServiceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedAdr {
    pub question: String,
    pub title: String,
    pub decision: String,
    pub reason: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub service_responsibilities: Vec<String>,
    /// For tech-stack ADRs: the canonical technology identifier used for scaffold dispatch
    #[serde(default)]
    pub technology: Option<String>,
    /// For tech-stack ADRs: "frontend" | "service"
    #[serde(default)]
    pub component_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProposedAdrs {
    #[serde(default)]
    pub proposals: Vec<ProposedAdr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanopyConfig {
    pub default: Option<AgentLlmConfig>,
    pub agents: Option<HashMap<String, AgentLlmConfig>>,
}

impl CanopyConfig {
    pub fn for_agent(&self, agent: &str) -> Option<AgentLlmConfig> {
        self.agents
            .as_ref()
            .and_then(|m| m.get(agent))
            .or_else(|| self.default.as_ref())
            .cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    #[default]
    Pending,
    Done,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationStep {
    pub id: String,
    pub service: String,
    pub file: String,
    pub operation: String,
    pub description: String,
    #[serde(default, deserialize_with = "deserialize_string_or_seq")]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub status: StepStatus,
}

fn deserialize_string_or_seq<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{SeqAccess, Visitor};
    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a sequence or empty-list string")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            let t = v.trim();
            if t == "[]" || t.is_empty() { Ok(vec![]) } else { Ok(vec![t.to_string()]) }
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut out = Vec::new();
            while let Some(s) = seq.next_element()? { out.push(s); }
            Ok(out)
        }
        fn visit_none<E: serde::de::Error>(self) -> Result<Vec<String>, E> { Ok(vec![]) }
        fn visit_unit<E: serde::de::Error>(self) -> Result<Vec<String>, E> { Ok(vec![]) }
    }
    deserializer.deserialize_any(V)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryPlan {
    pub story_id: String,
    #[serde(default)]
    pub steps: Vec<ImplementationStep>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adr_yaml_round_trip() {
        let adr = Adr {
            title: "Use PostgreSQL".into(),
            decision: "PostgreSQL as primary database".into(),
            reason: "Relational model fits domain".into(),
            alternatives: vec!["MongoDB".into()],
        };
        let yaml = serde_yaml::to_string(&adr).unwrap();
        let adr2: Adr = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(adr.title, adr2.title);
        assert_eq!(adr.alternatives, adr2.alternatives);
    }

    #[test]
    fn canopy_config_yaml_round_trip() {
        let yaml = r#"
default:
  provider: ollama
  model: qwen2.5:32b
agents:
  intent:
    provider: anthropic
    model: claude-sonnet-4-6
"#;
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.default.as_ref().unwrap().provider, LlmProvider::Ollama);
        assert_eq!(cfg.default.as_ref().unwrap().model, "qwen2.5:32b");
        let explorer = cfg.agents.as_ref().unwrap().get("intent").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Anthropic);
    }

    #[test]
    fn canopy_config_for_agent_falls_back_to_default() {
        let cfg: CanopyConfig = serde_yaml::from_str(
            "default:\n  provider: ollama\n  model: qwen2.5:32b\n"
        ).unwrap();
        let resolved = cfg.for_agent("intent").unwrap();
        assert_eq!(resolved.provider, LlmProvider::Ollama);
        assert_eq!(resolved.model, "qwen2.5:32b");
    }

    #[test]
    fn canopy_config_for_agent_prefers_specific_over_default() {
        let yaml = r#"
default:
  provider: ollama
  model: qwen2.5:32b
agents:
  intent:
    provider: anthropic
    model: claude-haiku-4-5-20251001
"#;
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let resolved = cfg.for_agent("intent").unwrap();
        assert_eq!(resolved.provider, LlmProvider::Anthropic);
        assert_eq!(resolved.model, "claude-haiku-4-5-20251001");
    }

    #[test]
    fn canopy_config_for_agent_returns_none_when_no_match() {
        let cfg = CanopyConfig { default: None, agents: None };
        assert!(cfg.for_agent("intent").is_none());
    }

    #[test]
    fn canopy_config_full_with_base_url_parses() {
        let yaml = "default:\n  provider: ollama\n  model: \"qwen2.5:32b\"\n\nagents:\n  intent:\n    provider: ollama\n    model: \"qwen2.5:32b\"\n    base_url: \"http://localhost:11434\"\n";
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let explorer = cfg.for_agent("intent").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Ollama);
        assert_eq!(explorer.model, "qwen2.5:32b");
        assert_eq!(explorer.base_url.unwrap(), "http://localhost:11434");
    }

    #[test]
    fn domain_entity_described_roundtrip() {
        let entity = DomainEntity::Described {
            name: "Product".into(),
            description: "A sellable item managed by the business.".into(),
        };
        let yaml = serde_yaml::to_string(&entity).unwrap();
        let entity2: DomainEntity = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(entity2.name(), "Product");
        assert_eq!(entity2.description(), Some("A sellable item managed by the business."));
    }

    #[test]
    fn domain_entity_simple_roundtrip() {
        let entity = DomainEntity::Simple("Order".into());
        let yaml = serde_yaml::to_string(&entity).unwrap();
        let entity2: DomainEntity = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(entity2.name(), "Order");
        assert_eq!(entity2.description(), None);
    }

    #[test]
    fn role_described_roundtrip() {
        let role = Role::Described {
            name: "product manager".into(),
            description: "Manages product registration in the backoffice.".into(),
        };
        let yaml = serde_yaml::to_string(&role).unwrap();
        let role2: Role = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(role2.name(), "product manager");
        assert_eq!(role2.description(), Some("Manages product registration in the backoffice."));
    }

    #[test]
    fn intent_spec_yaml_round_trip() {
        let spec = IntentSpec {
            intent_ref: "User Authentication".into(),
            entity_schema: None,
            scenarios: vec![Scenario {
                id: "auth-001".into(),
                name: "Successful login".into(),
                given: vec!["A registered User exists".into()],
                when: "The user submits valid credentials".into(),
                then: vec!["A Session token is returned".into()],
                constraints: vec!["Response under 300ms at p99".into()],
            }],
            out_of_scope: vec!["OAuth/SSO".into()],
            open_questions: vec!["Is email case-sensitive?".into()],
        };
        let yaml = serde_yaml::to_string(&spec).unwrap();
        let spec2: IntentSpec = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(spec.intent_ref, spec2.intent_ref);
        assert_eq!(spec.scenarios.len(), spec2.scenarios.len());
        assert_eq!(spec.scenarios[0].constraints, spec2.scenarios[0].constraints);
        assert_eq!(spec.out_of_scope, spec2.out_of_scope);
    }

    #[test]
    fn scaffold_plan_yaml_round_trip() {
        let plan = ScaffoldPlan {
            generated_at: "1750000000".into(),
            commands: vec![ScaffoldCommand {
                label: "storefront (Next.js)".into(),
                command: "npx create-next-app@latest storefront --typescript --tailwind --app".into(),
                working_dir: ".".into(),
                creates: "storefront/".into(),
            }],
        };
        let yaml = serde_yaml::to_string(&plan).unwrap();
        let plan2: ScaffoldPlan = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(plan.commands.len(), plan2.commands.len());
        assert_eq!(plan.commands[0].label, plan2.commands[0].label);
        assert_eq!(plan.commands[0].creates, plan2.commands[0].creates);
    }

    #[test]
    fn canopy_config_full_unquoted_parses() {
        let yaml = "default:\n  provider: ollama\n  model: qwen2.5:32b\n\nagents:\n  intent:\n    provider: ollama\n    model: qwen2.5:32b\n    base_url: http://localhost:11434\n";
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let explorer = cfg.for_agent("intent").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Ollama);
        assert_eq!(explorer.model, "qwen2.5:32b");
    }
}
