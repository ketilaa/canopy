use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RootsConfig {
    pub active_workspace: Option<String>,
}

impl RootsConfig {
    pub fn load(roots_dir: &Path) -> Self {
        let path = Self::config_path(roots_dir);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|contents| toml::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, roots_dir: &Path) -> Result<(), String> {
        let path = Self::config_path(roots_dir);
        let contents = toml::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, contents).map_err(|e| e.to_string())
    }

    pub fn config_path(roots_dir: &Path) -> PathBuf {
        roots_dir.join("config.toml")
    }
}
