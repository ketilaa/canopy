use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Java,
    Kotlin,
    TypeScript,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "java" => Some(Self::Java),
            "kt" | "kts" => Some(Self::Kotlin),
            "ts" | "tsx" => Some(Self::TypeScript),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::TypeScript => "typescript",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "java" => Ok(Self::Java),
            "kotlin" => Ok(Self::Kotlin),
            "typescript" => Ok(Self::TypeScript),
            other => Err(format!("unknown language: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub name:         String,
    pub path:         String,
    pub language:     Language,
    pub workspace_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_extension_java() {
        assert_eq!(Language::from_extension("java"), Some(Language::Java));
    }

    #[test]
    fn from_extension_kotlin() {
        assert_eq!(Language::from_extension("kt"), Some(Language::Kotlin));
        assert_eq!(Language::from_extension("kts"), Some(Language::Kotlin));
    }

    #[test]
    fn from_extension_typescript() {
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("tsx"), Some(Language::TypeScript));
    }

    #[test]
    fn from_extension_unknown() {
        assert_eq!(Language::from_extension("rs"), None);
        assert_eq!(Language::from_extension("py"), None);
    }

    #[test]
    fn roundtrip_from_str() {
        use std::str::FromStr;
        assert_eq!(Language::from_str("java").unwrap(), Language::Java);
        assert_eq!(Language::from_str("kotlin").unwrap(), Language::Kotlin);
        assert_eq!(Language::from_str("typescript").unwrap(), Language::TypeScript);
        assert!(Language::from_str("ruby").is_err());
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(Language::Java.to_string(), "java");
        assert_eq!(Language::Kotlin.to_string(), "kotlin");
        assert_eq!(Language::TypeScript.to_string(), "typescript");
    }
}
