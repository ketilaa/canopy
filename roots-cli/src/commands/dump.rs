use crate::db::{open_store, resolve_workspace};
use crate::output;

pub fn run(workspace: Option<&str>) -> Result<(), String> {
    let workspace_id = resolve_workspace(workspace)?;
    let store = open_store()?;
    let all = store.dump_all(&workspace_id).map_err(|e| e.to_string())?;
    output::json(&all);
    Ok(())
}
