use roots_core::Workspace;

use crate::config::RootsConfig;
use crate::db::{find_roots_dir, open_store};
use crate::output;

pub enum WorkspaceCmd {
    List,
    Add { name: String },
    Use { name: String },
}

pub fn run(cmd: WorkspaceCmd) -> Result<(), String> {
    match cmd {
        WorkspaceCmd::List => {
            let store = open_store()?;
            let workspaces = store.list_workspaces().map_err(|e| e.to_string())?;
            output::json(&workspaces);
            Ok(())
        }

        WorkspaceCmd::Add { name } => {
            Workspace::validate_slug(&name)?;
            let store = open_store()?;
            store.upsert_workspace(&name, &name).map_err(|e| e.to_string())?;
            output::json(&serde_json::json!({
                "status": "created",
                "id": name
            }));
            Ok(())
        }

        WorkspaceCmd::Use { name } => {
            let store = open_store()?;
            if !store.workspace_exists(&name).map_err(|e| e.to_string())? {
                return Err(format!("workspace '{}' does not exist — run `roots workspace add {}` first", name, name));
            }

            let roots_dir = find_roots_dir().ok_or_else(|| {
                "no .roots/index.db found — run `roots init` first".to_string()
            })?;

            let mut config = RootsConfig::load(&roots_dir);
            config.active_workspace = Some(name.clone());
            config.save(&roots_dir)?;

            output::json(&serde_json::json!({
                "status": "ok",
                "active_workspace": name
            }));
            Ok(())
        }
    }
}
