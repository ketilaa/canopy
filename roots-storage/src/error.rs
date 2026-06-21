#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("language parse error: {0}")]
    LanguageParse(String),
    #[error("symbol kind parse error: {0}")]
    SymbolKindParse(String),
}
