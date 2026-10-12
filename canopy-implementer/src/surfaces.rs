//! The declared surface of Java files (annotations, constructors, method signatures with return types), asked of
//! Roots as an external command: `roots surface <file>...` prints JSON, and nothing of Roots is linked.
//!
//! A step is shown the surface of a read-only file instead of its whole source: the declarations are what a
//! consumer needs, and the bodies are noise. A file Roots cannot answer for is shown in full instead.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// Answers, per path, the surface text of the files it can.
pub trait Surfaces: Send + Sync {
    fn surfaces(&self, root: &Path, paths: &[String]) -> HashMap<String, String>;
}

/// `roots surface`, run in the workspace root.
pub struct RootsSurfaces {
    binary: String,
}

impl RootsSurfaces {
    pub fn new(binary: &str) -> Self {
        Self { binary: binary.to_string() }
    }
}

impl Surfaces for RootsSurfaces {
    fn surfaces(&self, root: &Path, paths: &[String]) -> HashMap<String, String> {
        if paths.is_empty() {
            return HashMap::new();
        }
        Command::new(&self.binary)
            .arg("surface")
            .args(paths)
            .current_dir(root)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| parse_answer(&String::from_utf8_lossy(&out.stdout)))
            .unwrap_or_default()
    }
}

/// The text of each answered file. A file with an `error`, or with nothing declared, is left out.
pub fn parse_answer(json: &str) -> HashMap<String, String> {
    let Ok(Value::Array(answers)) = serde_json::from_str::<Value>(json) else {
        return HashMap::new();
    };
    answers
        .iter()
        .filter_map(|a| {
            let file = a.get("file")?.as_str()?;
            let text = a.get("text")?.as_str()?;
            (!text.trim().is_empty()).then(|| (file.to_string(), text.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_of_each_answered_file_is_kept_and_errors_and_empty_surfaces_are_left_out() {
        let answer = r#"[
          {"file": "a/A.java", "text": "public class A\n", "surface": {}},
          {"file": "a/Gone.java", "error": "No such file"},
          {"file": "a/Empty.java", "text": "", "surface": {}}
        ]"#;
        let parsed = parse_answer(answer);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed["a/A.java"], "public class A\n");
        assert!(parse_answer("not json").is_empty());
    }

    #[test]
    fn a_missing_binary_answers_nothing_so_the_caller_falls_back_to_the_source() {
        let surfaces = RootsSurfaces::new("definitely-not-a-binary-on-the-path");
        assert!(surfaces.surfaces(Path::new("."), &["a/A.java".to_string()]).is_empty());
    }
}
