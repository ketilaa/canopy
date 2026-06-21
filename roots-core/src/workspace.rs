use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id:   String,
    pub name: String,
}

impl Workspace {
    pub fn validate_slug(slug: &str) -> Result<(), String> {
        if slug.is_empty() {
            return Err("workspace id must not be empty".into());
        }
        if !slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
            return Err(format!(
                "workspace id '{}' must contain only lowercase letters, digits, hyphens, or underscores",
                slug
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_slugs_pass() {
        assert!(Workspace::validate_slug("acme-ecommerce").is_ok());
        assert!(Workspace::validate_slug("my_project").is_ok());
        assert!(Workspace::validate_slug("proj123").is_ok());
    }

    #[test]
    fn invalid_slugs_fail() {
        assert!(Workspace::validate_slug("").is_err());
        assert!(Workspace::validate_slug("My-Project").is_err());
        assert!(Workspace::validate_slug("has space").is_err());
        assert!(Workspace::validate_slug("special!char").is_err());
    }
}
