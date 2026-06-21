use roots_storage::Store;

use crate::error::ContextError;
use crate::facts;
use crate::impact;
use crate::packet::{ImpactSummary, SymbolContextPacket};

pub fn symbol_context(
    store: &Store,
    workspace_id: &str,
    fqn: &str,
) -> Result<SymbolContextPacket, ContextError> {
    let symbol = store.query_by_fqn(workspace_id, fqn)?
        .ok_or_else(|| ContextError::NotFound(fqn.to_string()))?;

    let callers    = store.query_callers(workspace_id, fqn)?;
    let callees    = store.query_callees(workspace_id, fqn)?;
    let deps       = store.query_deps(workspace_id, fqn)?;
    let transitive = store.query_impact(workspace_id, fqn)?;

    let fan_in  = callers.len();
    let fan_out = deps.len();
    let sc      = impact::score(fan_in, fan_out);

    let mut all_rels = callers.clone();
    all_rels.extend(callees.clone());
    all_rels.extend(deps.clone());
    all_rels.dedup_by(|a, b| {
        a.from_symbol == b.from_symbol && a.to_symbol == b.to_symbol && a.kind == b.kind
    });
    let facts = facts::from_relationships(&all_rels);

    Ok(SymbolContextPacket {
        symbol,
        callers,
        callees,
        deps,
        impact: ImpactSummary {
            fan_in,
            fan_out,
            score:            sc,
            risk:             impact::classify(sc).to_string(),
            transitive_count: transitive.len(),
        },
        facts,
    })
}
