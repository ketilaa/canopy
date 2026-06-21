use roots_context::{feature_context, file_context, project_context, symbol_context};

use crate::db::{open_store, resolve_workspace};
use crate::output;

pub fn run(
    workspace: Option<&str>,
    symbol:    Option<&str>,
    feature:   Option<&str>,
    project:   Option<&str>,
    file:      Option<&str>,
) -> Result<(), String> {
    let workspace_id = resolve_workspace(workspace)?;
    let store        = open_store()?;

    match (symbol, feature, project, file) {
        (Some(fqn), None, None, None) => {
            let packet = symbol_context(&store, &workspace_id, fqn)
                .map_err(|e| e.to_string())?;
            output::json(&packet);
        }
        (None, Some(goal), None, None) => {
            let packet = feature_context(&store, &workspace_id, goal)
                .map_err(|e| e.to_string())?;
            output::json(&packet);
        }
        (None, None, Some(name), None) => {
            let packet = project_context(&store, &workspace_id, name)
                .map_err(|e| e.to_string())?;
            output::json(&packet);
        }
        (None, None, None, Some(path)) => {
            let packet = file_context(&store, &workspace_id, path)
                .map_err(|e| e.to_string())?;
            output::json(&packet);
        }
        _ => {
            return Err(
                "provide exactly one of: <symbol>, --feature <goal>, --project <name>, --file <path>"
                    .to_string(),
            );
        }
    }

    Ok(())
}
