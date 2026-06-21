use roots_storage::{RelationshipRow, Store};

use crate::error::ContextError;
use crate::facts;
use crate::packet::ProjectContextPacket;

pub fn project_context(
    store: &Store,
    workspace_id: &str,
    project_name: &str,
) -> Result<ProjectContextPacket, ContextError> {
    let symbols = store.query_project_symbols(workspace_id, project_name)?;

    let language = symbols.first()
        .map(|s| s.language.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let mut all_rels: Vec<RelationshipRow> = Vec::new();
    for sym in &symbols {
        let deps = store.query_deps(workspace_id, &sym.fqn)?;
        all_rels.extend(deps);
    }
    all_rels.dedup_by(|a, b| {
        a.from_symbol == b.from_symbol && a.to_symbol == b.to_symbol && a.kind == b.kind
    });

    let facts = facts::from_relationships(&all_rels);

    Ok(ProjectContextPacket { project: project_name.to_string(), language, symbols, facts })
}
