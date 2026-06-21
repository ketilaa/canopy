use std::fs;

use roots_storage::Store;

use crate::config::RootsConfig;
use crate::db::db_path_for_cwd;
use crate::output;

pub fn run() -> Result<(), String> {
    let db_path = db_path_for_cwd();

    if db_path.exists() {
        output::json(&serde_json::json!({
            "status": "already initialized",
            "path": db_path.to_string_lossy()
        }));
        return Ok(());
    }

    let roots_dir = db_path.parent().unwrap();
    fs::create_dir_all(roots_dir).map_err(|e| e.to_string())?;

    let store = Store::open(&db_path).map_err(|e| e.to_string())?;
    store.init_schema().map_err(|e| e.to_string())?;
    store.upsert_workspace("default", "default").map_err(|e| e.to_string())?;

    let mut config = RootsConfig::load(roots_dir);
    if config.active_workspace.is_none() {
        config.active_workspace = Some("default".to_string());
        config.save(roots_dir)?;
    }

    output::json(&serde_json::json!({
        "status": "initialized",
        "path": db_path.to_string_lossy(),
        "active_workspace": "default"
    }));
    Ok(())
}
