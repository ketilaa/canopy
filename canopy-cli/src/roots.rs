use roots_context::{feature_context, FeatureContextPacket};
use roots_storage::Store;

const INDEX_PATH: &str = ".roots/index.db";

fn workspace_id() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "workspace".to_string())
}

fn open_store() -> Option<Store> {
    let path = std::path::Path::new(INDEX_PATH);
    if !path.exists() {
        return None;
    }
    Store::open(path).ok()
}

/// Ensures `.roots/` is initialized and the index is current.
/// Delegates to the `roots` binary for actual parsing and indexing.
/// Silently no-ops if the binary is not installed.
pub fn ensure_indexed() {
    if !binary_available() {
        return;
    }
    if !std::path::Path::new(INDEX_PATH).exists() {
        let _ = std::process::Command::new("roots").arg("init").status();
    }
    let _ = std::process::Command::new("roots").arg("index").status();
}

/// Returns entity names (Class, Interface, Enum) from the Roots index.
/// Returns None when no index exists or the index is empty.
pub fn entity_vocabulary() -> Option<Vec<String>> {
    let store = open_store()?;
    let ws = workspace_id();
    let symbols = store.dump_all(&ws).ok()?;
    let names: Vec<String> = symbols
        .into_iter()
        .filter(|s| matches!(s.kind.as_str(), "class" | "interface" | "enum"))
        .map(|s| s.name)
        .collect();
    if names.is_empty() { None } else { Some(names) }
}

/// Returns a feature context packet for the given goal, or None when no index exists.
pub fn get_feature_context(goal: &str) -> Option<FeatureContextPacket> {
    let store = open_store()?;
    let ws = workspace_id();
    feature_context(&store, &ws, goal).ok()
}

fn binary_available() -> bool {
    std::process::Command::new("roots")
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
