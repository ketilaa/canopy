use roots_storage::Store;

use crate::error::ContextError;
use crate::facts;
use crate::packet::FileContextPacket;

pub fn file_context(
    store: &Store,
    workspace_id: &str,
    file_path: &str,
) -> Result<FileContextPacket, ContextError> {
    let symbols       = store.query_file_symbols(workspace_id, file_path)?;
    let relationships = store.query_file_relationships(workspace_id, file_path)?;
    let language      = symbols.first()
        .map(|s| s.language.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let facts = facts::from_relationships(&relationships);

    Ok(FileContextPacket {
        file: file_path.to_string(),
        language,
        symbols,
        relationships,
        facts,
    })
}
