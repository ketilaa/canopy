//! `roots surface <file>...`: the declared surface of Java source files, parsed from the files themselves.
//!
//! No workspace or index is needed, so it answers for a file written a moment ago. One JSON array, one object per
//! file: `file`, `surface` (types, annotations, members) and `text` (the compact rendering a model reads). A file
//! that cannot be read gets an `error` instead; the others are still answered.

use serde_json::{json, Value};

use crate::output;

pub fn run(files: &[String]) -> Result<(), String> {
    output::json(&answers(files));
    Ok(())
}

fn answers(files: &[String]) -> Vec<Value> {
    files
        .iter()
        .map(|file| match std::fs::read_to_string(file) {
            Ok(source) => {
                let surface = roots_parser::java_surface(&source);
                json!({"file": file, "text": surface.render(), "surface": surface})
            }
            Err(e) => json!({"file": file, "error": e.to_string()}),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_file_is_answered_with_its_surface_and_text_and_a_missing_file_with_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Widget.java");
        std::fs::write(&path, "package p;\npublic record Widget(String id) {}\n").unwrap();
        let missing = dir.path().join("Gone.java");
        let answers = answers(&[path.display().to_string(), missing.display().to_string()]);
        assert_eq!(answers.len(), 2);
        assert_eq!(answers[0]["surface"]["types"][0]["kind"], "record");
        assert_eq!(answers[0]["text"], "public record Widget(String id)\n");
        assert!(answers[1]["error"].is_string());
        assert!(answers[1].get("surface").is_none());
    }
}
