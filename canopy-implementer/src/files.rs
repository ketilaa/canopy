//! The model's answer: a list of files to write, and the safe way to write them.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FilesError {
    #[error("model output is not valid for the files schema: {0}")]
    Parse(String),
    #[error("unsafe path {path:?}: {reason}")]
    UnsafePath { path: String, reason: &'static str },
    #[error("path {0:?} appears more than once")]
    Duplicate(String),
    #[error("could not write {path:?}: {message}")]
    Io { path: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedFiles {
    pub files: Vec<GeneratedFile>,
}

impl GeneratedFiles {
/// The JSON schema the model's answer is constrained to.
pub fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "files": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }
            }
        },
        "required": ["files"]
    })
}

pub fn parse(raw: &str) -> Result<Self, FilesError> {
    serde_json::from_str(raw).map_err(|e| FilesError::Parse(e.to_string()))
}

/// Writes every file under `root` and returns the paths written. Validates every path first,
/// so a rejected answer leaves the workspace untouched.
pub fn write_into(&self, root: &Path) -> Result<Vec<PathBuf>, FilesError> {
    self.validate_paths()?;
    let mut written = Vec::with_capacity(self.files.len());
    for file in &self.files {
        let target = root.join(&file.path);
        let io = |e: std::io::Error| FilesError::Io { path: file.path.clone(), message: e.to_string() };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(&target, &file.content).map_err(io)?;
        written.push(target);
    }
    Ok(written)
}

fn validate_paths(&self) -> Result<(), FilesError> {
    let mut seen = std::collections::HashSet::new();
    for file in &self.files {
        let unsafe_path = |reason| FilesError::UnsafePath { path: file.path.clone(), reason };
        if file.path.trim().is_empty() {
            return Err(unsafe_path("path is empty"));
        }
        let path = Path::new(&file.path);
        if path.is_absolute() {
            return Err(unsafe_path("path is absolute"));
        }
        if path.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(unsafe_path("path leaves the workspace or is not normalised"));
        }
        if !seen.insert(file.path.clone()) {
            return Err(FilesError::Duplicate(file.path.clone()));
        }
    }
    Ok(())
}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(pairs: &[(&str, &str)]) -> GeneratedFiles {
        GeneratedFiles {
            files: pairs
                .iter()
                .map(|(p, c)| GeneratedFile { path: p.to_string(), content: c.to_string() })
                .collect(),
        }
    }

    #[test]
    fn parses_a_valid_answer() {
        let raw = r#"{"files":[{"path":"src/A.java","content":"class A {}"}]}"#;
        assert_eq!(GeneratedFiles::parse(raw).unwrap(), files(&[("src/A.java", "class A {}")]));
    }

    #[test]
    fn rejects_an_answer_missing_a_field() {
        let raw = r#"{"files":[{"path":"src/A.java"}]}"#;
        assert!(matches!(GeneratedFiles::parse(raw), Err(FilesError::Parse(_))));
    }

    #[test]
    fn rejects_an_answer_with_unknown_fields() {
        let raw = r#"{"files":[],"explanation":"here you go"}"#;
        assert!(matches!(GeneratedFiles::parse(raw), Err(FilesError::Parse(_))));
    }

    #[test]
    fn schema_requires_files_with_path_and_content() {
        let s = GeneratedFiles::schema();
        assert_eq!(s["required"], serde_json::json!(["files"]));
        assert_eq!(s["properties"]["files"]["items"]["required"], serde_json::json!(["path", "content"]));
    }

    #[test]
    fn writes_files_creating_directories() {
        let dir = tempfile::tempdir().unwrap();
        let written = files(&[("src/main/A.java", "class A {}")]).write_into(dir.path()).unwrap();
        assert_eq!(written, vec![dir.path().join("src/main/A.java")]);
        assert_eq!(std::fs::read_to_string(dir.path().join("src/main/A.java")).unwrap(), "class A {}");
    }

    #[test]
    fn rejects_absolute_paths_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let err = files(&[("src/ok.txt", "x"), ("/etc/evil", "y")]).write_into(dir.path()).unwrap_err();
        assert!(matches!(err, FilesError::UnsafePath { .. }));
        assert!(!dir.path().join("src/ok.txt").exists(), "a rejected answer must leave the workspace untouched");
    }

    #[test]
    fn rejects_parent_directory_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let err = files(&[("../outside.txt", "y")]).write_into(dir.path()).unwrap_err();
        assert!(matches!(err, FilesError::UnsafePath { .. }));
    }

    #[test]
    fn rejects_empty_paths() {
        let dir = tempfile::tempdir().unwrap();
        let err = files(&[("", "y")]).write_into(dir.path()).unwrap_err();
        assert!(matches!(err, FilesError::UnsafePath { .. }));
    }

    #[test]
    fn rejects_duplicate_paths() {
        let dir = tempfile::tempdir().unwrap();
        let err = files(&[("a.txt", "1"), ("a.txt", "2")]).write_into(dir.path()).unwrap_err();
        assert_eq!(err, FilesError::Duplicate("a.txt".to_string()));
    }
}
