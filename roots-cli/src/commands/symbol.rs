use crate::db::{open_store, resolve_workspace};
use crate::output;

pub fn run(workspace: Option<&str>, name: &str) -> Result<(), String> {
    let workspace_id = resolve_workspace(workspace)?;
    let store = open_store()?;
    let results = store.query_exact(&workspace_id, name).map_err(|e| e.to_string())?;
    output::json(&results);
    Ok(())
}
