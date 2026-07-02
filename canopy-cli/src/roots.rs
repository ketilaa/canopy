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
    let store = Store::open(path).ok()?;
    store.init_schema().ok()?;
    Some(store)
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

/// Returns a compact type surface for the given class names, scoped to `service_dir`.
///
/// For each class, the surface lists its constructors and methods with signatures:
///   ProductResponse {
///     ProductResponse()
///     void setId(UUID id)
///     UUID getId()
///     ...
///   }
///
/// Returns None when the index is unavailable, the class is not found, or no
/// signatures have been stored yet (pre-V4 index — caller falls back to raw files).
pub fn get_class_surface(class_names: &[&str], service_dir: &str) -> Option<String> {
    let store = open_store()?;
    let ws = workspace_id();
    let mut surfaces: Vec<String> = Vec::new();

    for &class_name in class_names {
        let candidates = store.query_exact(&ws, class_name).ok()?;
        // Prefer symbols whose file lives under this service.
        let class_sym = candidates.iter()
            .find(|s| s.file.starts_with(service_dir) && matches!(s.kind.as_str(), "class" | "interface"))
            .or_else(|| candidates.iter().find(|s| matches!(s.kind.as_str(), "class" | "interface")))?;

        let file_syms = store.query_file_symbols(&ws, &class_sym.file).ok()?;
        let members: Vec<String> = file_syms.iter()
            .filter(|s| s.kind == "method")
            .filter_map(|s| s.signature.as_deref().map(|sig| format!("  {}{}", s.name, sig)))
            .collect();

        // If no member has a signature, the index is pre-V4 — skip to avoid misleading context.
        let has_sigs = file_syms.iter().any(|s| s.kind == "method" && s.signature.is_some());
        if !has_sigs {
            return None;
        }

        if members.is_empty() {
            surfaces.push(format!("class {} {{}}", class_sym.name));
        } else {
            surfaces.push(format!("class {} {{\n{}\n}}", class_sym.name, members.join("\n")));
        }
    }

    if surfaces.is_empty() { None } else { Some(surfaces.join("\n\n")) }
}

/// Re-runs `roots index` if an index already exists. No-ops when Roots is not set up.
/// Call after writing new source files to keep the index current.
pub fn reindex() {
    if !std::path::Path::new(INDEX_PATH).exists() {
        return;
    }
    if !binary_available() {
        return;
    }
    let _ = std::process::Command::new("roots").arg("index").status();
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
