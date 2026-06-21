use roots_storage::StorageError;

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("symbol not found: {0}")]
    NotFound(String),
}
