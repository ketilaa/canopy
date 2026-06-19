use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
}
