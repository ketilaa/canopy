//! The tools the model may call to find and read files, and the harness's record of what it read.
//!
//! The model cannot mutate anything through a tool. It reads, and it submits files for the harness
//! to write. Every path and pattern goes through [`crate::sandbox`] first.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::observe::ToolOutcome;
use crate::sandbox::{is_denied_name, validate_pattern, PathError, RepoPath};

pub const MAX_READ_BYTES: u64 = 16 * 1024;
pub const MAX_GLOB_RESULTS: usize = 200;

/// A tool as offered to the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

impl ToolSpec {
    /// The OpenAI-compatible `tools` entry.
    pub fn to_wire(&self) -> Value {
        json!({ "type": "function", "function": {
            "name": self.name, "description": self.description, "parameters": self.parameters } })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub outcome: ToolOutcome,
    /// What the model is told.
    pub text: String,
}

pub struct ToolBox {
    root: PathBuf,
    read: Mutex<BTreeSet<String>>,
}

impl ToolBox {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            read: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn specs() -> Vec<ToolSpec> {
        Self::specs_with(false)
    }

    /// With `declare_imports`, `submit_files` first asks for a checklist: for every file, the classes it uses from
    /// another package and the import for each, written before the files themselves.
    pub fn specs_with(declare_imports: bool) -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "glob",
                description: "List the files in the repository that match a glob pattern, for example \
                              src/**/*.java. Paths are relative to the repository root.",
                parameters: json!({
                    "type": "object",
                    "properties": { "pattern": { "type": "string", "description": "A glob pattern, relative to the repository root." } },
                    "required": ["pattern"]
                }),
            },
            ToolSpec {
                name: "read_file",
                description: "Read one text file. The path is relative to the repository root.",
                parameters: json!({
                    "type": "object",
                    "properties": { "path": { "type": "string", "description": "A file path, relative to the repository root." } },
                    "required": ["path"]
                }),
            },
            ToolSpec {
                name: "submit_files",
                description: "Finish the task: submit the complete new content of every file you create or \
                              change. Every existing file you change must have been read with read_file first.",
                parameters: if declare_imports {
                    json!({
                        "type": "object",
                        "properties": {
                            "declared_imports": {
                                "type": "array",
                                "description": "Checklist, written before the files: for every file, each class it uses \
                                                from another package and its qualified name. Empty when none.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "file": { "type": "string" },
                                        "import": { "type": "string", "description": "For example java.util.Optional" }
                                    },
                                    "required": ["file", "import"]
                                }
                            },
                            "files": { "type": "array", "items": {
                                "type": "object",
                                "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                                "required": ["path", "content"]
                            } }
                        },
                        "required": ["declared_imports", "files"]
                    })
                } else {
                    json!({
                        "type": "object",
                        "properties": { "files": { "type": "array", "items": {
                            "type": "object",
                            "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                            "required": ["path", "content"]
                        } } },
                        "required": ["files"]
                    })
                },
            },
        ]
    }

    /// Runs `glob` or `read_file`. `submit_files` is handled by the loop, not here.
    pub fn call(&self, name: &str, arguments: &str) -> ToolResult {
        match name {
            "glob" => match string_argument(arguments, "pattern") {
                Ok(pattern) => self.glob(&pattern),
                Err(message) => error(message),
            },
            "read_file" => match string_argument(arguments, "path") {
                Ok(path) => self.read_file(&path),
                Err(message) => error(message),
            },
            other => error(format!(
                "There is no tool called `{other}`. Available tools: glob, read_file, submit_files."
            )),
        }
    }

    fn glob(&self, pattern: &str) -> ToolResult {
        if let Err(e) = validate_pattern(pattern) {
            return refused(format!("Refused: {e}."));
        }
        let matcher = match globset::GlobBuilder::new(pattern).literal_separator(true).build() {
            Ok(glob) => glob.compile_matcher(),
            Err(e) => return error(format!("`{pattern}` is not a valid glob pattern: {e}.")),
        };
        let mut found = Vec::new();
        walk(&self.root, "", &matcher, &mut found);
        found.sort();
        if found.is_empty() {
            let names_a_directory = RepoPath::parse(pattern)
                .ok()
                .and_then(|p| p.resolve(&self.root).ok())
                .is_some_and(|p| p.is_dir());
            let text = if names_a_directory {
                let dir = pattern.trim_end_matches('/');
                format!("No files match `{pattern}`. It is a directory: use `{dir}/**` to list the files in it.")
            } else {
                format!("No files match `{pattern}`.")
            };
            return ok(text);
        }
        let total = found.len();
        found.truncate(MAX_GLOB_RESULTS);
        let mut text = found.join("\n");
        if total > MAX_GLOB_RESULTS {
            text.push_str(&format!(
                "\n({} more matches not shown; use a narrower pattern)",
                total - MAX_GLOB_RESULTS
            ));
        }
        ok(text)
    }

    fn read_file(&self, path: &str) -> ToolResult {
        let repo_path = match RepoPath::parse(path) {
            Ok(p) => p,
            Err(e) => return refused(format!("Refused: {e}.")),
        };
        let shown = repo_path.as_str();
        let resolved = match repo_path.resolve(&self.root) {
            Ok(p) => p,
            Err(PathError::Missing(_)) => {
                return error(format!("`{shown}` does not exist. Use glob to find the right path."))
            }
            Err(e) => return refused(format!("Refused: {e}.")),
        };
        let Ok(metadata) = std::fs::metadata(&resolved) else {
            return error(format!("`{shown}` could not be read."));
        };
        if metadata.is_dir() {
            return error(format!(
                "`{shown}` is a directory, not a file. Use glob with `{shown}/**` to list the files in it."
            ));
        }
        if metadata.len() > MAX_READ_BYTES {
            return refused(format!(
                "Refused: `{shown}` is {} bytes, over the {MAX_READ_BYTES} byte limit for read_file.",
                metadata.len()
            ));
        }
        let Ok(bytes) = std::fs::read(&resolved) else {
            return error(format!("`{shown}` could not be read."));
        };
        if bytes.contains(&0) {
            return refused(format!("Refused: `{shown}` is a binary file."));
        }
        let Ok(text) = String::from_utf8(bytes) else {
            return refused(format!("Refused: `{shown}` is not valid UTF-8 text."));
        };
        self.read.lock().unwrap().insert(shown);
        ok(if text.is_empty() {
            "(the file is empty)".to_string()
        } else {
            text
        })
    }

    /// Whether `path` (in any spelling the sandbox normalises) was successfully read.
    pub fn has_read(&self, path: &str) -> bool {
        RepoPath::parse(path).is_ok_and(|p| self.read.lock().unwrap().contains(&p.as_str()))
    }

    pub fn files_read(&self) -> Vec<String> {
        self.read.lock().unwrap().iter().cloned().collect()
    }
}

fn ok(text: String) -> ToolResult {
    ToolResult {
        outcome: ToolOutcome::Ok,
        text,
    }
}

fn refused(text: String) -> ToolResult {
    ToolResult {
        outcome: ToolOutcome::Refused,
        text,
    }
}

fn error(text: String) -> ToolResult {
    ToolResult {
        outcome: ToolOutcome::Error,
        text,
    }
}

fn string_argument(arguments: &str, key: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(arguments)
        .map_err(|_| format!("The arguments are not valid JSON. Send an object like {{\"{key}\": \"...\"}}."))?;
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("The argument `{key}` is required and must be a string."))
}

/// Lists regular files under `dir`, as paths relative to the repository root. Symlinks are neither
/// listed nor followed, and top-level denied directories are skipped.
fn walk(dir: &Path, relative: &str, matcher: &globset::GlobMatcher, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if relative.is_empty() && is_denied_name(&name) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            continue;
        }
        let path = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        if file_type.is_dir() {
            walk(&entry.path(), &path, matcher, found);
        } else if file_type.is_file() && matcher.is_match(&path) {
            found.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn repo() -> (tempfile::TempDir, ToolBox) {
        let dir = tempfile::tempdir().unwrap();
        let w = |rel: &str, content: &str| {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        };
        w("pom.xml", "<project/>");
        w("src/main/java/com/example/A.java", "class A {}");
        w("src/main/java/com/example/B.java", "class B {}");
        w("src/test/java/com/example/ATest.java", "class ATest {}");
        w("target/classes/A.class", "binary-ish");
        w(".git/config", "[core]");
        let tools = ToolBox::new(dir.path());
        (dir, tools)
    }

    fn call(t: &ToolBox, name: &str, args: Value) -> ToolResult {
        t.call(name, &args.to_string())
    }

    fn glob(t: &ToolBox, pattern: &str) -> ToolResult {
        call(t, "glob", json!({ "pattern": pattern }))
    }

    fn read(t: &ToolBox, path: &str) -> ToolResult {
        call(t, "read_file", json!({ "path": path }))
    }

    #[test]
    fn the_model_is_offered_glob_read_file_and_submit_files_with_their_required_arguments() {
        let specs = ToolBox::specs();
        let names: Vec<_> = specs.iter().map(|s| s.name).collect();
        assert_eq!(names, ["glob", "read_file", "submit_files"]);
        assert_eq!(specs[0].parameters["required"], json!(["pattern"]));
        assert_eq!(specs[1].parameters["required"], json!(["path"]));
        assert_eq!(specs[2].parameters["required"], json!(["files"]));
        assert_eq!(specs[0].to_wire()["function"]["name"], "glob");
        assert_eq!(specs[0].to_wire()["type"], "function");
    }

    #[test]
    fn glob_lists_matching_repository_relative_paths_sorted() {
        let (_dir, t) = repo();
        let r = glob(&t, "src/**/*.java");
        assert_eq!(r.outcome, ToolOutcome::Ok);
        assert_eq!(
            r.text,
            "src/main/java/com/example/A.java\nsrc/main/java/com/example/B.java\nsrc/test/java/com/example/ATest.java"
        );
    }

    #[test]
    fn a_single_star_does_not_cross_directories() {
        let (_dir, t) = repo();
        assert_eq!(glob(&t, "*.xml").text, "pom.xml");
        assert!(glob(&t, "*.java").text.starts_with("No files match"));
        assert_eq!(glob(&t, "**/A*.java").text.lines().count(), 2);
    }

    #[test]
    fn glob_never_lists_denied_directories() {
        let (_dir, t) = repo();
        let everything = glob(&t, "**").text;
        assert!(
            !everything.contains("target/") && !everything.contains(".git/"),
            "{everything}"
        );
        assert!(everything.contains("pom.xml"));
    }

    #[test]
    fn glob_refuses_absolute_and_parent_patterns() {
        let (_dir, t) = repo();
        for pattern in ["/**", "/etc/*", "../**", "src/../../*", "{src,..}/*"] {
            assert_eq!(glob(&t, pattern).outcome, ToolOutcome::Refused, "{pattern}");
        }
    }

    #[test]
    fn glob_with_no_match_says_so_and_hints_when_the_pattern_names_a_directory() {
        let (_dir, t) = repo();
        let none = glob(&t, "src/**/*.kt");
        assert_eq!(
            (none.outcome, none.text.as_str()),
            (ToolOutcome::Ok, "No files match `src/**/*.kt`.")
        );
        let dir = glob(&t, "src/main");
        assert!(
            dir.text.contains("is a directory") && dir.text.contains("src/main/**"),
            "{}",
            dir.text
        );
    }

    #[test]
    fn glob_neither_lists_symlinks_nor_descends_into_symlinked_directories() {
        let (dir, t) = repo();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.java"), "class S {}").unwrap();
        symlink(outside.path(), dir.path().join("src/escape")).unwrap();
        symlink(dir.path().join("pom.xml"), dir.path().join("alias.xml")).unwrap();
        let all = glob(&t, "**").text;
        assert!(
            !all.contains("secret.java") && !all.contains("escape") && !all.contains("alias.xml"),
            "{all}"
        );
    }

    #[test]
    fn glob_caps_the_number_of_results_and_says_how_many_were_left_out() {
        let (dir, t) = repo();
        for i in 0..(MAX_GLOB_RESULTS + 5) {
            std::fs::write(dir.path().join(format!("f{i:03}.txt")), "x").unwrap();
        }
        let r = glob(&t, "*.txt");
        assert_eq!(r.outcome, ToolOutcome::Ok);
        assert_eq!(r.text.lines().filter(|l| l.ends_with(".txt")).count(), MAX_GLOB_RESULTS);
        assert!(r.text.contains("5 more"), "{}", r.text);
    }

    #[test]
    fn read_file_returns_the_content_and_remembers_the_read_in_any_spelling() {
        let (_dir, t) = repo();
        let r = read(&t, "./src/main/java/com/example/A.java");
        assert_eq!((r.outcome, r.text.as_str()), (ToolOutcome::Ok, "class A {}"));
        assert!(t.has_read("src/main/java/com/example/A.java"));
        assert!(t.has_read("./src/main/java/com/example/A.java"));
        assert!(!t.has_read("src/main/java/com/example/B.java"));
        assert_eq!(t.files_read(), ["src/main/java/com/example/A.java"]);
    }

    #[test]
    fn read_file_refuses_paths_outside_the_sandbox_and_remembers_nothing() {
        let (dir, t) = repo();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "LEAKED-CONTENT-123").unwrap();
        symlink(outside.path().join("secret.txt"), dir.path().join("leak.txt")).unwrap();
        symlink(dir.path().join("pom.xml"), dir.path().join("alias.xml")).unwrap();
        symlink(outside.path(), dir.path().join("escape")).unwrap();
        for path in [
            "/etc/passwd",
            "../outside.txt",
            "src/../../x",
            "leak.txt",
            "alias.xml",
            "escape/secret.txt",
            ".git/config",
            ".GIT/config",
            "target/classes/A.class",
            "a\\b",
        ] {
            let r = read(&t, path);
            assert_eq!(r.outcome, ToolOutcome::Refused, "{path}: {}", r.text);
            assert!(!r.text.contains("LEAKED-CONTENT-123"), "{path}");
        }
        assert!(t.files_read().is_empty());
    }

    #[test]
    fn read_file_reports_a_missing_file_and_a_directory_as_errors_with_a_next_step() {
        let (_dir, t) = repo();
        let missing = read(&t, "src/main/java/com/example/Nope.java");
        assert_eq!(missing.outcome, ToolOutcome::Error);
        assert!(
            missing.text.contains("does not exist") && missing.text.contains("glob"),
            "{}",
            missing.text
        );
        let directory = read(&t, "src/main");
        assert_eq!(directory.outcome, ToolOutcome::Error);
        assert!(
            directory.text.contains("directory") && directory.text.contains("src/main/**"),
            "{}",
            directory.text
        );
        assert!(t.files_read().is_empty());
    }

    #[test]
    fn read_file_refuses_large_binary_and_non_utf8_files() {
        let (dir, t) = repo();
        std::fs::write(dir.path().join("big.txt"), vec![b'a'; (MAX_READ_BYTES + 1) as usize]).unwrap();
        std::fs::write(dir.path().join("bin.dat"), [1u8, 0, 2]).unwrap();
        std::fs::write(dir.path().join("latin1.txt"), [0xE9u8, 0xE8]).unwrap();
        for path in ["big.txt", "bin.dat", "latin1.txt"] {
            assert_eq!(read(&t, path).outcome, ToolOutcome::Refused, "{path}");
        }
        assert!(t.files_read().is_empty());
    }

    #[test]
    fn an_empty_file_is_reported_as_empty_not_as_nothing() {
        let (dir, t) = repo();
        std::fs::write(dir.path().join("empty.txt"), "").unwrap();
        let r = read(&t, "empty.txt");
        assert_eq!((r.outcome, r.text.as_str()), (ToolOutcome::Ok, "(the file is empty)"));
        assert!(t.has_read("empty.txt"));
    }

    #[test]
    fn bad_arguments_and_unknown_tools_are_errors_the_model_can_act_on() {
        let (_dir, t) = repo();
        assert_eq!(t.call("glob", "not json").outcome, ToolOutcome::Error);
        assert_eq!(t.call("glob", "{}").outcome, ToolOutcome::Error);
        assert_eq!(t.call("read_file", r#"{"path": 5}"#).outcome, ToolOutcome::Error);
        let unknown = t.call("delete_file", "{}");
        assert_eq!(unknown.outcome, ToolOutcome::Error);
        assert!(
            unknown.text.contains("glob") && unknown.text.contains("read_file"),
            "{}",
            unknown.text
        );
    }

    #[test]
    fn the_declared_imports_variant_of_submit_files_asks_for_imports_before_files() {
        let plain = ToolBox::specs();
        assert!(plain[2].parameters["properties"].get("declared_imports").is_none());
        let declared = ToolBox::specs_with(true);
        let submit = &declared[2].parameters;
        assert_eq!(submit["required"], json!(["declared_imports", "files"]));
        let keys: Vec<&String> = submit["properties"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["declared_imports", "files"], "the checklist must come first so it is written first");
        assert_eq!(submit["properties"]["declared_imports"]["items"]["required"], json!(["file", "import"]));
        assert_eq!(declared[0], plain[0]);
    }
}
