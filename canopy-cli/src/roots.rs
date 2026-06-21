use serde::Deserialize;

#[derive(Deserialize)]
struct RootsSymbol {
    name: String,
    kind: String,
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

/// Ensures `.roots/` is initialized and the index is current.
/// Silently no-ops if the `roots` binary is not installed.
pub fn ensure_indexed() {
    if !binary_available() {
        return;
    }
    if !std::path::Path::new(".roots/index.db").exists() {
        let _ = std::process::Command::new("roots").arg("init").status();
    }
    let _ = std::process::Command::new("roots").arg("index").status();
}

/// Returns entity names (Class, Interface, Enum symbols) from the Roots index.
/// Returns None when Roots is not available or the index is empty.
pub fn entity_vocabulary() -> Option<Vec<String>> {
    if !std::path::Path::new(".roots/index.db").exists() {
        return None;
    }
    let output = std::process::Command::new("roots")
        .arg("dump")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let symbols: Vec<RootsSymbol> = serde_json::from_slice(&output.stdout).ok()?;
    let names: Vec<String> = symbols
        .into_iter()
        .filter(|s| matches!(s.kind.as_str(), "Class" | "Interface" | "Enum"))
        .map(|s| s.name)
        .collect();
    if names.is_empty() { None } else { Some(names) }
}
