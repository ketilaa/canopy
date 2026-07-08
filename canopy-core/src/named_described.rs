use serde::{Deserialize, Serialize};

/// Constructs a `Simple`/`Described` variant uniformly — lets generic code (e.g. a
/// bootstrap helper) build any of the `named_described!` types without matching on
/// which concrete type it is.
pub trait Described: Sized {
    fn simple(name: String) -> Self;
    fn described(name: String, description: String) -> Self;
}

/// Generates a "named item with optional human-curated description" type that
/// serializes as a plain string (no description) or `{name, description}` map (with
/// one). A macro, not a shared type alias — a type alias would make e.g. `DomainEntity`
/// and `DomainEvent` the literal same Rust type, silently allowing one to be pushed
/// into a `Vec` of the other.
macro_rules! named_described {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(untagged)]
        pub enum $name {
            Simple(String),
            Described { name: String, description: String },
        }

        impl $name {
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

        impl Described for $name {
            fn simple(name: String) -> Self { Self::Simple(name) }
            fn described(name: String, description: String) -> Self { Self::Described { name, description } }
        }
    };
}

named_described!(DomainEntity, "A domain entity with an optional human-curated description.");
named_described!(DomainEvent, "A domain event with an optional human-curated description.");
named_described!(Role, "A user role with an optional human-curated description.");

#[cfg(test)]
mod tests {
    use super::*;

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
}
