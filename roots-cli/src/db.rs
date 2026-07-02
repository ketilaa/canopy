use std::path::PathBuf;

use roots_storage::Store;

use crate::config::RootsConfig;

const ROOTS_DIR: &str = ".roots";
const DB_FILE: &str = "index.db";

/// Search current dir upward for a .roots/index.db, like git finds .git.
pub fn find_db() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(ROOTS_DIR).join(DB_FILE);
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Search current dir upward for a .roots/ directory.
pub fn find_roots_dir() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(ROOTS_DIR);
        if candidate.join(DB_FILE).exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub fn open_store() -> Result<Store, String> {
    let path = find_db().ok_or_else(|| {
        "no .roots/index.db found — run `roots init` first".to_string()
    })?;
    let store = Store::open(&path).map_err(|e| e.to_string())?;
    store.init_schema().map_err(|e| e.to_string())?;
    Ok(store)
}

pub fn roots_dir_for_cwd() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(ROOTS_DIR)
}

pub fn db_path_for_cwd() -> PathBuf {
    roots_dir_for_cwd().join(DB_FILE)
}

/// Resolve the active workspace from:
/// 1. explicit --workspace flag value (if provided)
/// 2. active_workspace in .roots/config.toml
/// Returns Err with an actionable message if neither is set.
pub fn resolve_workspace(explicit: Option<&str>) -> Result<String, String> {
    if let Some(ws) = explicit {
        return Ok(ws.to_string());
    }
    let roots_dir = find_roots_dir().ok_or_else(|| {
        "no .roots/index.db found — run `roots init` first".to_string()
    })?;
    let config = RootsConfig::load(&roots_dir);
    config.active_workspace.ok_or_else(|| {
        "no active workspace set — run `roots workspace use <name>` or pass --workspace <name>".to_string()
    })
}
