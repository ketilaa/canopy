//! Remembers what files looked like before an attempt wrote them, so a failed attempt can be undone
//! exactly. The model never decides whether a failure is kept.

use std::path::{Path, PathBuf};

pub struct Snapshot {
    root: PathBuf,
    /// Relative path and the content before the first capture (`None`: the file did not exist).
    saved: Vec<(String, Option<Vec<u8>>)>,
}

impl Snapshot {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            saved: Vec::new(),
        }
    }

    /// Records the current state of each path. A path captured before keeps its first state.
    pub fn capture(&mut self, paths: &[String]) {
        for path in paths {
            if self.saved.iter().any(|(saved, _)| saved == path) {
                continue;
            }
            let content = std::fs::read(self.root.join(path)).ok();
            self.saved.push((path.clone(), content));
        }
    }

    /// Puts every captured path back as it was: rewrites changed files, deletes created ones.
    /// Returns the paths it restored.
    pub fn restore(&self) -> Vec<String> {
        for (path, content) in &self.saved {
            let target = self.root.join(path);
            match content {
                Some(bytes) => {
                    let _ = std::fs::write(&target, bytes);
                }
                None => {
                    let _ = std::fs::remove_file(&target);
                }
            }
        }
        self.saved.iter().map(|(path, _)| path.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn restore_puts_a_modified_file_back() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a/A.java", "original");
        let mut snap = Snapshot::new(dir.path());
        snap.capture(&["a/A.java".to_string()]);
        write(dir.path(), "a/A.java", "changed");
        assert_eq!(snap.restore(), vec!["a/A.java".to_string()]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a/A.java")).unwrap(),
            "original"
        );
    }

    #[test]
    fn restore_deletes_a_file_the_attempt_created() {
        let dir = tempfile::tempdir().unwrap();
        let mut snap = Snapshot::new(dir.path());
        snap.capture(&["new/B.java".to_string()]);
        write(dir.path(), "new/B.java", "created");
        snap.restore();
        assert!(!dir.path().join("new/B.java").exists());
    }

    #[test]
    fn the_first_captured_state_wins() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "A.java", "original");
        let mut snap = Snapshot::new(dir.path());
        snap.capture(&["A.java".to_string()]);
        write(dir.path(), "A.java", "second");
        snap.capture(&["A.java".to_string()]);
        snap.restore();
        assert_eq!(std::fs::read_to_string(dir.path().join("A.java")).unwrap(), "original");
    }

    #[test]
    fn restoring_an_untouched_file_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "A.java", "same");
        let mut snap = Snapshot::new(dir.path());
        snap.capture(&["A.java".to_string()]);
        snap.restore();
        assert_eq!(std::fs::read_to_string(dir.path().join("A.java")).unwrap(), "same");
    }
}
