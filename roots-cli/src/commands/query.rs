use crate::db::{open_store, resolve_workspace};
use crate::output;

pub fn run(workspace: Option<&str>, term: &str) -> Result<(), String> {
    let workspace_id = resolve_workspace(workspace)?;
    let store = open_store()?;
    let results = store.query_prefix(&workspace_id, term).map_err(|e| e.to_string())?;
    output::json(&results);
    Ok(())
}
