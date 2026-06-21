use crate::db::{open_store, resolve_workspace};
use crate::output;

pub fn run(workspace: Option<&str>, symbol: &str) -> Result<(), String> {
    let workspace_id = resolve_workspace(workspace)?;
    let store = open_store()?;
    let callers = store.query_callers(&workspace_id, symbol).map_err(|e| e.to_string())?;
    output::json(&callers);
    Ok(())
}
