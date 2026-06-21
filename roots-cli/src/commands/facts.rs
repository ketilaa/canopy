use roots_context::symbol_facts;

use crate::db::{open_store, resolve_workspace};
use crate::output;

pub fn run(workspace: Option<&str>, symbol: &str) -> Result<(), String> {
    let workspace_id = resolve_workspace(workspace)?;
    let store        = open_store()?;
    let facts        = symbol_facts(&store, &workspace_id, symbol)
        .map_err(|e| e.to_string())?;
    output::json(&facts);
    Ok(())
}
