use serde::{Deserialize, Serialize};

use crate::Language;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Class,
    Interface,
    Enum,
    Function,
    Method,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Enum => "enum",
            Self::Function => "function",
            Self::Method => "method",
        }
    }
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SymbolKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "class" => Ok(Self::Class),
            "interface" => Ok(Self::Interface),
            "enum" => Ok(Self::Enum),
            "function" => Ok(Self::Function),
            "method" => Ok(Self::Method),
            other => Err(format!("unknown symbol kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name:         String,
    pub kind:         SymbolKind,
    pub file:         String,
    pub language:     Language,
    pub project:      String,
    pub workspace_id: String,
    pub line:         u32,
    pub fqn:          String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_kind_roundtrip() {
        use std::str::FromStr;
        for (s, k) in [
            ("class", SymbolKind::Class),
            ("interface", SymbolKind::Interface),
            ("enum", SymbolKind::Enum),
            ("function", SymbolKind::Function),
            ("method", SymbolKind::Method),
        ] {
            assert_eq!(SymbolKind::from_str(s).unwrap(), k);
            assert_eq!(k.as_str(), s);
        }
    }

    #[test]
    fn symbol_serializes_to_json() {
        let sym = Symbol {
            name:         "OrderService".into(),
            kind:         SymbolKind::Class,
            file:         "src/OrderService.java".into(),
            language:     Language::Java,
            project:      "orders".into(),
            workspace_id: "acme".into(),
            line:         10,
            fqn:          "com.example.orders.OrderService".into(),
        };
        let json = serde_json::to_string(&sym).unwrap();
        assert!(json.contains("\"kind\":\"class\""));
        assert!(json.contains("\"language\":\"java\""));
    }
}
