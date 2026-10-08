//! The only way the model's paths and glob patterns touch the filesystem.
//!
//! A path is checked in two passes. First lexically: no absolute paths, no `..`, no backslashes,
//! no NUL, nothing in a denied directory. Then against the filesystem: every component must exist
//! and none may be a symlink, whether it points outside the repository or inside it. The canonical
//! result is finally asserted to lie under the canonical root, as a last line of defence.
//!
//! Known gap: a check-then-use race if something else modifies the repository during a run. The
//! harness is the only writer and runs one step at a time, so this is documented, not engineered
//! around.

use std::path::{Component, Path, PathBuf};

const MAX_PATH_LEN: usize = 512;
const MAX_PATTERN_LEN: usize = 200;
/// Directories the model may neither list nor read. Compared case-insensitively (macOS is).
const DENIED_DIRS: [&str; 4] = [".git", "target", "node_modules", ".idea"];

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PathError {
    #[error("the path is empty")]
    Empty,
    #[error("the path is too long")]
    TooLong,
    #[error("the path contains a NUL character")]
    Nul,
    #[error("use a path relative to the repository root, not an absolute one")]
    Absolute,
    #[error("paths may not contain `..`")]
    ParentDir,
    #[error("use forward slashes, not backslashes")]
    Backslash,
    #[error("`{0}` is not available to you")]
    Denied(String),
    #[error("`{0}` does not exist")]
    Missing(String),
    #[error("`{0}` is or passes through a symbolic link, which is not allowed")]
    Symlink(String),
    #[error("`{0}` resolves outside the repository")]
    OutsideRoot(String),
}

/// A repository-relative path that passed the lexical checks, normalised with forward slashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPath {
    relative: PathBuf,
}

impl RepoPath {
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        if raw.contains('\0') {
            return Err(PathError::Nul);
        }
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(PathError::Empty);
        }
        if raw.len() > MAX_PATH_LEN {
            return Err(PathError::TooLong);
        }
        if raw.contains('\\') {
            return Err(PathError::Backslash);
        }
        let mut relative = PathBuf::new();
        for component in Path::new(raw).components() {
            match component {
                Component::Prefix(_) | Component::RootDir => return Err(PathError::Absolute),
                Component::ParentDir => return Err(PathError::ParentDir),
                Component::CurDir => {}
                Component::Normal(name) => relative.push(name),
            }
        }
        let Some(first) = relative.components().next() else {
            return Err(PathError::Empty);
        };
        let first = first.as_os_str().to_string_lossy();
        if is_denied_name(&first) {
            return Err(PathError::Denied(first.into_owned()));
        }
        Ok(Self { relative })
    }

    /// The path as the model should see and write it: relative, forward slashes.
    pub fn as_str(&self) -> String {
        self.relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }

    /// The location under `root`. Every component must exist and none may be a symlink. A directory
    /// resolves fine; the caller decides whether it needs a file.
    pub fn resolve(&self, root: &Path) -> Result<PathBuf, PathError> {
        let shown = self.as_str();
        let mut current = root.to_path_buf();
        for component in self.relative.components() {
            current.push(component);
            let metadata = std::fs::symlink_metadata(&current).map_err(|_| PathError::Missing(shown.clone()))?;
            if metadata.file_type().is_symlink() {
                return Err(PathError::Symlink(shown));
            }
        }
        // Defence in depth: whatever the checks above missed, the real location must be under the root.
        let canonical_root = std::fs::canonicalize(root).map_err(|_| PathError::Missing(shown.clone()))?;
        let canonical = std::fs::canonicalize(&current).map_err(|_| PathError::Missing(shown.clone()))?;
        if !canonical.starts_with(&canonical_root) {
            return Err(PathError::OutsideRoot(shown));
        }
        Ok(current)
    }
}

/// Checks a glob pattern before it is matched against repository-relative paths.
pub fn validate_pattern(pattern: &str) -> Result<(), PathError> {
    if pattern.contains('\0') {
        return Err(PathError::Nul);
    }
    if pattern.trim().is_empty() {
        return Err(PathError::Empty);
    }
    if pattern.len() > MAX_PATTERN_LEN {
        return Err(PathError::TooLong);
    }
    if pattern.contains('\\') {
        return Err(PathError::Backslash);
    }
    if pattern.starts_with('/') {
        return Err(PathError::Absolute);
    }
    // Any `..`, even inside an alternation like {a,..}: patterns are matched against relative paths
    // the harness itself lists, so nothing legitimate needs it.
    if pattern.contains("..") {
        return Err(PathError::ParentDir);
    }
    Ok(())
}

/// Whether a top-level directory or file name is off limits.
pub fn is_denied_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    DENIED_DIRS.contains(&lower.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn ok(raw: &str) -> String {
        RepoPath::parse(raw)
            .unwrap_or_else(|e| panic!("{raw:?} should parse: {e}"))
            .as_str()
    }

    fn err(raw: &str) -> PathError {
        RepoPath::parse(raw).expect_err(&format!("{raw:?} should be rejected"))
    }

    #[test]
    fn plain_relative_paths_parse_and_a_leading_dot_slash_is_dropped() {
        assert_eq!(ok("src/main/A.java"), "src/main/A.java");
        assert_eq!(ok("./src/A.java"), "src/A.java");
        assert_eq!(ok("src//A.java"), "src/A.java");
        assert_eq!(ok("src/"), "src");
    }

    #[test]
    fn absolute_paths_are_rejected() {
        assert_eq!(err("/etc/passwd"), PathError::Absolute);
        assert_eq!(err("//server/share"), PathError::Absolute);
    }

    #[test]
    fn parent_directory_components_are_rejected_wherever_they_appear() {
        for raw in ["../x", "a/../../x", "a/../b", "src/..", ".."] {
            assert_eq!(err(raw), PathError::ParentDir, "{raw}");
        }
    }

    #[test]
    fn backslashes_nul_and_empty_paths_are_rejected() {
        assert_eq!(err("C:\\Windows\\x"), PathError::Backslash);
        assert_eq!(err("a\\b"), PathError::Backslash);
        assert_eq!(err("a\0b"), PathError::Nul);
        assert_eq!(err(""), PathError::Empty);
        assert_eq!(err("   "), PathError::Empty);
        assert_eq!(err("."), PathError::Empty);
        assert_eq!(err("./"), PathError::Empty);
    }

    #[test]
    fn overlong_paths_are_rejected() {
        assert_eq!(err(&"a/".repeat(300)), PathError::TooLong);
    }

    #[test]
    fn denied_directories_are_rejected_case_insensitively() {
        for raw in [
            ".git/config",
            ".GIT/config",
            ".Git",
            "target/classes/A.class",
            "TARGET/x",
            "node_modules/x",
            ".idea/x",
        ] {
            assert!(matches!(err(raw), PathError::Denied(_)), "{raw}");
        }
        assert_eq!(
            ok("src/target/A.java"),
            "src/target/A.java",
            "only a top-level directory is denied"
        );
    }

    fn repo() -> (tempfile::TempDir, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/main")).unwrap();
        std::fs::write(root.path().join("src/main/A.java"), "class A {}").unwrap();
        std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        (root, outside)
    }

    fn resolve(root: &Path, raw: &str) -> Result<PathBuf, PathError> {
        RepoPath::parse(raw)?.resolve(root)
    }

    #[test]
    fn an_existing_file_and_an_existing_directory_resolve_under_the_root() {
        let (root, _outside) = repo();
        assert_eq!(
            resolve(root.path(), "src/main/A.java").unwrap(),
            root.path().join("src/main/A.java")
        );
        assert!(resolve(root.path(), "src/main").unwrap().is_dir());
    }

    #[test]
    fn a_missing_path_is_reported_as_missing() {
        let (root, _outside) = repo();
        assert_eq!(
            resolve(root.path(), "src/main/B.java"),
            Err(PathError::Missing("src/main/B.java".into()))
        );
    }

    #[test]
    fn a_symlink_to_a_file_outside_the_repository_is_refused() {
        let (root, outside) = repo();
        symlink(outside.path().join("secret.txt"), root.path().join("src/leak.txt")).unwrap();
        assert!(matches!(
            resolve(root.path(), "src/leak.txt"),
            Err(PathError::Symlink(_))
        ));
    }

    #[test]
    fn a_symlink_to_a_file_inside_the_repository_is_refused_too() {
        let (root, _outside) = repo();
        symlink(root.path().join("src/main/A.java"), root.path().join("alias.java")).unwrap();
        assert!(matches!(resolve(root.path(), "alias.java"), Err(PathError::Symlink(_))));
    }

    #[test]
    fn a_symlinked_directory_component_is_refused() {
        let (root, outside) = repo();
        symlink(outside.path(), root.path().join("src/escape")).unwrap();
        assert!(matches!(
            resolve(root.path(), "src/escape/secret.txt"),
            Err(PathError::Symlink(_))
        ));
    }

    #[test]
    fn a_symlink_loop_is_refused_not_followed() {
        let (root, _outside) = repo();
        symlink(root.path().join("loop_b"), root.path().join("loop_a")).unwrap();
        symlink(root.path().join("loop_a"), root.path().join("loop_b")).unwrap();
        assert!(matches!(resolve(root.path(), "loop_a"), Err(PathError::Symlink(_))));
    }

    #[test]
    fn a_dangling_symlink_is_refused_as_a_symlink() {
        let (root, _outside) = repo();
        symlink(root.path().join("nowhere"), root.path().join("dangling")).unwrap();
        assert!(matches!(resolve(root.path(), "dangling"), Err(PathError::Symlink(_))));
    }

    #[test]
    fn a_root_that_is_itself_reached_through_a_symlink_still_works() {
        let (root, _outside) = repo();
        let holder = tempfile::tempdir().unwrap();
        let via = holder.path().join("via");
        symlink(root.path(), &via).unwrap();
        assert!(
            resolve(&via, "src/main/A.java").is_ok(),
            "the root is the caller's choice and is trusted"
        );
    }

    #[test]
    fn patterns_may_not_be_absolute_or_contain_parent_references_or_backslashes() {
        for good in ["src/**/*.java", "**/*.java", "*.xml", "src/main/*"] {
            assert_eq!(validate_pattern(good), Ok(()), "{good}");
        }
        assert_eq!(validate_pattern("/**"), Err(PathError::Absolute));
        assert_eq!(validate_pattern("/etc/*"), Err(PathError::Absolute));
        assert_eq!(validate_pattern("../**"), Err(PathError::ParentDir));
        assert_eq!(validate_pattern("a/../**"), Err(PathError::ParentDir));
        assert_eq!(validate_pattern("{src,..}/*"), Err(PathError::ParentDir));
        assert_eq!(validate_pattern("a\\b"), Err(PathError::Backslash));
        assert_eq!(validate_pattern("a\0b"), Err(PathError::Nul));
        assert_eq!(validate_pattern(""), Err(PathError::Empty));
        assert_eq!(validate_pattern(&"a".repeat(300)), Err(PathError::TooLong));
    }

    #[test]
    fn denied_names_are_recognised_case_insensitively() {
        assert!(is_denied_name(".git") && is_denied_name(".GIT") && is_denied_name("Target"));
        assert!(!is_denied_name("src") && !is_denied_name("targets"));
    }
}
