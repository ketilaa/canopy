use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;

/// An entity attribute as returned by the LLM.
/// The LLM naturally produces YAML key-value maps (`- fieldName: description`)
/// rather than plain strings, so we accept both forms.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EntityAttribute {
    /// Plain string: `- "fieldName: type description"`
    Inline(String),
    /// Map entry: `- fieldName: type description`
    Typed(BTreeMap<String, String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idea {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreQuestions {
    pub questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnsweredQuestion {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vision {
    pub project: String,
    pub problem: String,
    pub users: Vec<String>,
    pub goals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirements {
    pub functional: Vec<String>,
    pub non_functional: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDetail {
    pub name: String,
    pub attributes: Vec<EntityAttribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainModel {
    pub entities: Vec<String>,
    pub entities_detail: Vec<EntityDetail>,
    pub events: Vec<String>,
    pub relationships: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryIntent {
    pub title: String,
    pub description: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryIntents {
    pub intents: Vec<DeliveryIntent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralCommitments {
    pub deployment_topology: String,
    pub integration_style: String,
    pub data_ownership: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitecturePrinciples {
    pub principles: Vec<String>,
    pub constraints: Vec<String>,
    pub structural_commitments: StructuralCommitments,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentArchitecture {
    pub frontend: serde_yaml::Value,
    pub backend: serde_yaml::Value,
    pub database: serde_yaml::Value,
    pub deployment: serde_yaml::Value,
    pub reasoning: Vec<serde_yaml::Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentSpec {
    pub intent_ref: String,
    pub scenarios: Vec<Scenario>,
    #[serde(default)]
    pub out_of_scope: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenQuestion {
    pub question: String,
    pub blocking: bool,
    #[serde(default)]
    pub default_assumption: Option<String>,
    #[serde(default)]
    pub answer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationTask {
    pub id: String,
    pub title: String,
    pub task_type: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria_refs: Vec<String>,
    pub estimated_complexity: String,
    pub blocking: bool,
    #[serde(default)]
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperOutput {
    pub files: Vec<GeneratedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub scenario_id: String,
    pub scenario_name: String,
    pub passed: bool,
    pub reasoning: String,
    #[serde(default)]
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub intent_ref: String,
    pub passed: usize,
    pub total: usize,
    pub results: Vec<ValidationResult>,
}

impl ValidationReport {
    pub fn recompute_totals(&mut self) {
        self.total = self.results.len();
        self.passed = self.results.iter().filter(|r| r.passed).count();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainScope {
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub relationships: Vec<String>,
}

/// Accumulated entity and event vocabulary across all planned delivery intents.
/// Built incrementally by `canopy plan` — no upfront global modeling required.
/// In repository mode, Roots is the authoritative source and supersedes this file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainRegistry {
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
}

impl DomainRegistry {
    pub fn merge(&mut self, scope: &DomainScope) {
        for name in &scope.entities {
            if !self.entities.contains(name) {
                self.entities.push(name.clone());
            }
        }
        for name in &scope.events {
            if !self.events.contains(name) {
                self.events.push(name.clone());
            }
        }
    }
}

fn default_draft() -> String {
    "draft".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationPlan {
    pub intent_ref: String,
    pub intent_index: usize,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default = "default_draft")]
    pub status: String,
    #[serde(default)]
    pub depends_on_intents: Vec<usize>,
    #[serde(default)]
    pub domain_scope: DomainScope,
    pub tasks: Vec<ImplementationTask>,
    #[serde(default)]
    pub reasoning: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<OpenQuestion>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vision_yaml_round_trip() {
        let v = Vision {
            project: "TestProject".into(),
            problem: "A test problem.".into(),
            users: vec!["Developer".into()],
            goals: vec!["Ship fast".into(), "Stay simple".into()],
        };
        let yaml = serde_yaml::to_string(&v).unwrap();
        let v2: Vision = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(v.project, v2.project);
        assert_eq!(v.goals, v2.goals);
    }

    #[test]
    fn requirements_yaml_round_trip() {
        let r = Requirements {
            functional: vec!["User can login".into()],
            non_functional: vec!["Response under 200ms".into()],
        };
        let yaml = serde_yaml::to_string(&r).unwrap();
        let r2: Requirements = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(r.functional, r2.functional);
        assert_eq!(r.non_functional, r2.non_functional);
    }

    #[test]
    fn domain_model_yaml_round_trip() {
        let d = DomainModel {
            entities: vec!["User".into(), "Order".into()],
            entities_detail: vec![EntityDetail {
                name: "User".into(),
                attributes: vec![
                    EntityAttribute::Inline("id: UUID".into()),
                    EntityAttribute::Typed({
                        let mut m = BTreeMap::new();
                        m.insert("email".into(), "String unique address".into());
                        m
                    }),
                ],
            }],
            events: vec!["OrderPlaced".into()],
            relationships: vec!["User places Order".into()],
        };
        let yaml = serde_yaml::to_string(&d).unwrap();
        let d2: DomainModel = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(d.entities, d2.entities);
        assert_eq!(d.entities_detail[0].name, d2.entities_detail[0].name);
        assert_eq!(d.entities_detail[0].attributes.len(), 2);
    }

    #[test]
    fn entity_attribute_map_format_parses() {
        let yaml = "- userId: UUID unique identifier\n- email: string email address\n";
        let attrs: Vec<EntityAttribute> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(attrs.len(), 2);
        assert!(matches!(&attrs[0], EntityAttribute::Typed(_)));
    }

    #[test]
    fn delivery_intents_yaml_round_trip() {
        let di = DeliveryIntents {
            intents: vec![
                DeliveryIntent {
                    title: "User authentication".into(),
                    description: "Users can register and log in securely.".into(),
                    value: "Enables personalized access to all features.".into(),
                },
                DeliveryIntent {
                    title: "Dashboard".into(),
                    description: "Users see an overview of their data.".into(),
                    value: "Gives users immediate insight after login.".into(),
                },
            ],
        };
        let yaml = serde_yaml::to_string(&di).unwrap();
        let di2: DeliveryIntents = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(di.intents.len(), di2.intents.len());
        assert_eq!(di.intents[0].title, di2.intents[0].title);
        assert_eq!(di.intents[1].value, di2.intents[1].value);
    }

    #[test]
    fn architecture_principles_yaml_round_trip() {
        let ap = ArchitecturePrinciples {
            principles: vec!["Stateless application tier".into()],
            constraints: vec!["Must deploy on-premise".into(), "Team expertise: Rust".into()],
            structural_commitments: StructuralCommitments {
                deployment_topology: "Modular monolith".into(),
                integration_style: "Event-driven via internal event bus".into(),
                data_ownership: "Shared database, schema-per-module".into(),
            },
        };
        let yaml = serde_yaml::to_string(&ap).unwrap();
        let ap2: ArchitecturePrinciples = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(ap.principles, ap2.principles);
        assert_eq!(ap.constraints.len(), 2);
        assert_eq!(
            ap.structural_commitments.deployment_topology,
            ap2.structural_commitments.deployment_topology
        );
    }

    #[test]
    fn component_architecture_yaml_round_trip() {
        let a = ComponentArchitecture {
            frontend: serde_yaml::Value::String("React".into()),
            backend: serde_yaml::Value::String("Axum".into()),
            database: serde_yaml::Value::String("PostgreSQL".into()),
            deployment: serde_yaml::Value::String("AWS".into()),
            reasoning: vec![serde_yaml::Value::String("Strong ecosystem".into())],
        };
        let yaml = serde_yaml::to_string(&a).unwrap();
        let a2: ComponentArchitecture = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(a.database, a2.database);
        assert_eq!(a.reasoning, a2.reasoning);
    }

    #[test]
    fn component_architecture_rich_fields_parse() {
        let yaml = r#"
frontend:
  framework: Next.js 14
  ui_library: Tailwind CSS
backend:
  services:
    api: Axum
    worker: Tokio
database:
  catalogue_service:
    engine: PostgreSQL 16
    port: 5433
  cart_service:
    engine: Redis 7
    port: 6379
deployment:
  platform: AWS ECS
  regions:
    - eu-west-1
reasoning:
  - Strong ecosystem
"#;
        let a: ComponentArchitecture = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(a.database, serde_yaml::Value::Mapping(_)));
        assert!(matches!(a.deployment, serde_yaml::Value::Mapping(_)));
        assert_eq!(a.reasoning.len(), 1);
    }

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
  explorer:
    provider: anthropic
    model: claude-sonnet-4-6
"#;
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.default.as_ref().unwrap().provider, LlmProvider::Ollama);
        assert_eq!(cfg.default.as_ref().unwrap().model, "qwen2.5:32b");
        let explorer = cfg.agents.as_ref().unwrap().get("explorer").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Anthropic);
    }

    #[test]
    fn canopy_config_for_agent_falls_back_to_default() {
        let cfg: CanopyConfig = serde_yaml::from_str(
            "default:\n  provider: ollama\n  model: qwen2.5:32b\n"
        ).unwrap();
        let resolved = cfg.for_agent("explorer").unwrap();
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
  explorer:
    provider: anthropic
    model: claude-haiku-4-5-20251001
"#;
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let resolved = cfg.for_agent("explorer").unwrap();
        assert_eq!(resolved.provider, LlmProvider::Anthropic);
        assert_eq!(resolved.model, "claude-haiku-4-5-20251001");
    }

    #[test]
    fn canopy_config_for_agent_returns_none_when_no_match() {
        let cfg = CanopyConfig { default: None, agents: None };
        assert!(cfg.for_agent("explorer").is_none());
    }

    #[test]
    fn canopy_config_full_with_base_url_parses() {
        let yaml = "default:\n  provider: ollama\n  model: \"qwen2.5:32b\"\n\nagents:\n  explorer:\n    provider: ollama\n    model: \"qwen2.5:32b\"\n    base_url: \"http://localhost:11434\"\n";
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let explorer = cfg.for_agent("explorer").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Ollama);
        assert_eq!(explorer.model, "qwen2.5:32b");
        assert_eq!(explorer.base_url.unwrap(), "http://localhost:11434");
    }

    #[test]
    fn domain_registry_merge_deduplicates() {
        let mut reg = DomainRegistry {
            entities: vec!["User".into(), "Session".into()],
            events: vec!["UserLoggedIn".into()],
        };
        let scope = DomainScope {
            entities: vec!["Session".into(), "Order".into()],
            events: vec!["UserLoggedIn".into(), "OrderPlaced".into()],
            relationships: vec![],
        };
        reg.merge(&scope);
        assert_eq!(reg.entities, vec!["User", "Session", "Order"]);
        assert_eq!(reg.events, vec!["UserLoggedIn", "OrderPlaced"]);
    }

    #[test]
    fn domain_registry_merge_empty_base() {
        let mut reg = DomainRegistry::default();
        let scope = DomainScope {
            entities: vec!["Product".into()],
            events: vec!["ProductCreated".into()],
            relationships: vec![],
        };
        reg.merge(&scope);
        assert_eq!(reg.entities, vec!["Product"]);
        assert_eq!(reg.events, vec!["ProductCreated"]);
    }

    #[test]
    fn intent_spec_yaml_round_trip() {
        let spec = IntentSpec {
            intent_ref: "User Authentication".into(),
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
    fn implementation_plan_yaml_round_trip() {
        let plan = ImplementationPlan {
            intent_ref: "User Authentication".into(),
            intent_index: 0,
            generated_at: "1750000000".into(),
            status: "draft".into(),
            depends_on_intents: vec![],
            domain_scope: DomainScope {
                entities: vec!["User".into(), "Session".into()],
                events: vec!["UserLoggedIn".into()],
                relationships: vec!["User has many Sessions".into()],
            },
            tasks: vec![ImplementationTask {
                id: "task-001".into(),
                title: "Define User schema".into(),
                task_type: "schema".into(),
                inputs: vec![],
                outputs: vec!["migrations/001_users.sql".into()],
                acceptance_criteria_refs: vec!["auth-001".into()],
                estimated_complexity: "low".into(),
                blocking: true,
                completed: false,
            }],
            reasoning: vec!["Schema is a blocking prerequisite".into()],
            open_questions: vec![OpenQuestion {
                question: "Is email case-sensitive?".into(),
                blocking: true,
                default_assumption: None,
                answer: None,
            }],
        };
        let yaml = serde_yaml::to_string(&plan).unwrap();
        let plan2: ImplementationPlan = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(plan.intent_ref, plan2.intent_ref);
        assert_eq!(plan.tasks.len(), plan2.tasks.len());
        assert_eq!(plan.tasks[0].blocking, plan2.tasks[0].blocking);
        assert_eq!(plan.open_questions[0].blocking, plan2.open_questions[0].blocking);
        assert_eq!(plan.status, plan2.status);
    }

    #[test]
    fn implementation_plan_defaults_status_to_draft() {
        let yaml = "intent_ref: Test\nintent_index: 0\ndomain_scope: {}\ntasks: []\n";
        let plan: ImplementationPlan = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(plan.status, "draft");
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
        let yaml = "default:\n  provider: ollama\n  model: qwen2.5:32b\n\nagents:\n  explorer:\n    provider: ollama\n    model: qwen2.5:32b\n    base_url: http://localhost:11434\n";
        let cfg: CanopyConfig = serde_yaml::from_str(yaml).unwrap();
        let explorer = cfg.for_agent("explorer").unwrap();
        assert_eq!(explorer.provider, LlmProvider::Ollama);
        assert_eq!(explorer.model, "qwen2.5:32b");
    }
}
