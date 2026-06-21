use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationshipKind {
    Imports,
    Extends,
    Implements,
    Calls,
    References,
}

impl RelationshipKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Imports    => "IMPORTS",
            Self::Extends    => "EXTENDS",
            Self::Implements => "IMPLEMENTS",
            Self::Calls      => "CALLS",
            Self::References => "REFERENCES",
        }
    }
}

impl std::fmt::Display for RelationshipKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RelationshipKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "IMPORTS"    => Ok(Self::Imports),
            "EXTENDS"    => Ok(Self::Extends),
            "IMPLEMENTS" => Ok(Self::Implements),
            "CALLS"      => Ok(Self::Calls),
            "REFERENCES" => Ok(Self::References),
            other        => Err(format!("unknown relationship kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub from_symbol:  String,
    pub to_symbol:    String,
    pub kind:         RelationshipKind,
    pub file:         String,
    pub line:         Option<u32>,
    pub workspace_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relationship_kind_roundtrip() {
        use std::str::FromStr;
        for (s, k) in [
            ("IMPORTS",    RelationshipKind::Imports),
            ("EXTENDS",    RelationshipKind::Extends),
            ("IMPLEMENTS", RelationshipKind::Implements),
            ("CALLS",      RelationshipKind::Calls),
            ("REFERENCES", RelationshipKind::References),
        ] {
            assert_eq!(RelationshipKind::from_str(s).unwrap(), k);
            assert_eq!(k.as_str(), s);
        }
    }

    #[test]
    fn relationship_serializes_to_json() {
        let rel = Relationship {
            from_symbol:  "com.example.OrderService".into(),
            to_symbol:    "com.example.Repository".into(),
            kind:         RelationshipKind::Implements,
            file:         "src/OrderService.java".into(),
            line:         Some(3),
            workspace_id: "acme".into(),
        };
        let json = serde_json::to_string(&rel).unwrap();
        assert!(json.contains("\"IMPLEMENTS\""));
        assert!(json.contains("\"com.example.OrderService\""));
    }
}
