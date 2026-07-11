use canopy_llm::ToolCall;
use serde::Deserialize;

const INDEX_PATH: &str = ".roots/index.db";

/// One indexed symbol, as returned by the `roots` CLI's JSON output (`symbol`, `dump`,
/// `context`). Mirrors `roots_storage::SymbolRow`'s shape but is deserialized from the CLI's
/// stdout rather than a linked `roots-storage` type — canopy only talks to Roots as an external
/// command (see CLAUDE.md's Roots section), never by linking its storage/context crates.
#[derive(Deserialize, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
    #[serde(default)]
    pub fqn: String,
    #[serde(default)]
    pub signature: Option<String>,
}

/// Mirrors `roots_context::FeatureContextPacket`'s fields actually consumed by canopy —
/// `goal`/`keywords`/`relationships` aren't used here, so they're simply not deserialized
/// (serde ignores unrecognized-by-us JSON fields by default).
#[derive(Deserialize, Default)]
pub struct FeatureContextPacket {
    #[serde(default)]
    pub symbols: Vec<SymbolInfo>,
    #[serde(default)]
    pub facts: Vec<String>,
}

/// Runs `roots <args>` and parses stdout as JSON. Returns None on any failure — binary missing,
/// no index yet, non-zero exit, or malformed JSON — since every caller already treats "no Roots"
/// as a legitimate fallback-to-raw-files case, not an error to propagate.
fn run_roots_json<T: serde::de::DeserializeOwned>(args: &[&str]) -> Option<T> {
    if !std::path::Path::new(INDEX_PATH).exists() || !binary_available() {
        return None;
    }
    let output = std::process::Command::new("roots").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

fn dump_symbols() -> Option<Vec<SymbolInfo>> {
    run_roots_json(&["dump"])
}

/// Ensures `.roots/` is initialized and the index is current.
/// Delegates to the `roots` binary for actual parsing and indexing.
/// Silently no-ops if the binary is not installed.
pub fn ensure_indexed() {
    if !binary_available() {
        return;
    }
    if !std::path::Path::new(INDEX_PATH).exists() {
        let _ = std::process::Command::new("roots")
            .arg("init")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    let _ = std::process::Command::new("roots")
        .arg("index")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Returns a feature context packet for the given goal, or None when no index exists.
pub fn get_feature_context(goal: &str) -> Option<FeatureContextPacket> {
    run_roots_json(&["context", "--feature", goal])
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
    let all = dump_symbols()?;
    let mut surfaces: Vec<String> = Vec::new();

    for &class_name in class_names {
        let candidates: Vec<&SymbolInfo> = all.iter().filter(|s| s.name == class_name).collect();
        // Prefer symbols whose file lives under this service.
        let class_sym = candidates.iter().copied()
            .find(|s| s.file.starts_with(service_dir) && matches!(s.kind.as_str(), "class" | "interface"))
            .or_else(|| candidates.iter().copied().find(|s| matches!(s.kind.as_str(), "class" | "interface")))?;

        let file_syms: Vec<&SymbolInfo> = all.iter().filter(|s| s.file == class_sym.file).collect();
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

/// Returns a compact TypeScript export surface for the given service-relative file paths.
///
/// For each file the surface lists its exported interfaces, functions, and classes
/// (with method signatures where available):
///   // src/api/ProductApi.ts
///   export interface Product
///   export function registerProduct(productData: any)
///
/// Returns None when the index is unavailable or none of the files have indexed symbols
/// (caller should fall back to reading the files directly).
pub fn get_ts_module_surface(rel_paths: &[String], service_dir: &str) -> Option<String> {
    if rel_paths.is_empty() { return None; }
    let all = dump_symbols()?;
    let mut parts: Vec<String> = Vec::new();

    for rel in rel_paths {
        let ws_path = format!("{}/{}", service_dir, rel);
        let syms: Vec<&SymbolInfo> = all.iter().filter(|s| s.file == ws_path).collect();
        if syms.is_empty() { continue; }

        let mut lines = vec![format!("// {}", rel)];
        for sym in syms.iter().filter(|s| s.kind != "method") {
            match sym.kind.as_str() {
                "interface" | "enum" => {
                    lines.push(format!("export {} {}", sym.kind, sym.name));
                }
                "class" => {
                    // Methods of this class have FQN: {file}#{ClassName}#{methodName}
                    let class_prefix = format!("{}#{}", ws_path, sym.name);
                    let methods: Vec<String> = syms.iter()
                        .filter(|s| s.kind == "method" && s.fqn.starts_with(&class_prefix))
                        .filter_map(|s| s.signature.as_deref()
                            .map(|sig| format!("  {}{}", s.name, sig)))
                        .collect();
                    if methods.is_empty() {
                        lines.push(format!("export class {}", sym.name));
                    } else {
                        lines.push(format!("export class {} {{", sym.name));
                        lines.extend(methods);
                        lines.push("}".to_string());
                    }
                }
                "function" => {
                    let sig = sym.signature.as_deref().unwrap_or("()");
                    lines.push(format!("export function {}{}", sym.name, sig));
                }
                _ => {}
            }
        }
        if lines.len() > 1 {
            parts.push(lines.join("\n"));
        }
    }

    if parts.is_empty() { None } else { Some(parts.join("\n\n")) }
}

/// Looks up every indexed symbol with this exact name across the whole project — e.g.
/// resolving which file exports `createProduct` before writing an import for it. Returns None
/// when the index is unavailable or nothing matches. Backs the `find_symbol` tool exposed to
/// the fix loop (see `dispatch_find_symbol`).
pub fn find_symbol(name: &str) -> Option<Vec<SymbolInfo>> {
    let rows: Vec<SymbolInfo> = run_roots_json(&["symbol", name])?;
    if rows.is_empty() { None } else { Some(rows) }
}

/// Executes one `find_symbol` tool call against the real project's Roots index. `from_file` is
/// the project-relative path of the file making the lookup (known to canopy-cli already — the
/// model never has to supply it) — used to compute the exact relative import specifier the
/// answer is formatted with.
pub fn dispatch_find_symbol(call: &ToolCall, from_file: &str) -> String {
    let Some(name) = call.arguments.get("name").and_then(|v| v.as_str()) else {
        return "error: missing required \"name\" argument".to_string();
    };
    match find_symbol(name) {
        Some(rows) => rows.iter()
            .map(|r| format_symbol_match(r, from_file))
            .collect::<Vec<_>>()
            .join("\n"),
        None => format!("no symbol named \"{name}\" found in the index"),
    }
}

/// Formats one matched symbol as a self-contained answer: kind, defining file and line, the
/// exact relative import specifier `from_file` should use, and — since a TS/TSX caller needs to
/// know — whether it's a type-only construct (`import type`) or a value (`import`). Computing
/// this here means the model never has to count `../` levels or reason about `isolatedModules`
/// itself; the tool's answer already states the form to use.
pub fn format_symbol_match(row: &SymbolInfo, from_file: &str) -> String {
    let specifier = relative_import_specifier(from_file, &row.file);
    let import_form = if row.kind == "interface" {
        "type-only — use `import type`"
    } else {
        "value — use a regular `import`"
    };
    format!(
        "{} {} — defined in {} (line {}); import via '{specifier}' ({import_form})",
        row.kind, row.name, row.file, row.line
    )
}

/// Computes the relative import specifier a file at `from_file` would use to import from
/// `to_file` — e.g. `("services/product/src/services/ProductService.ts",
/// "services/product/src/models/Product.ts")` → `"../models/Product"`. Both paths are project-
/// relative, the same coordinate system the Roots index already stores `file` in. Strips the
/// extension since TS/JS import specifiers never include one.
fn relative_import_specifier(from_file: &str, to_file: &str) -> String {
    let from_dir = std::path::Path::new(from_file).parent().unwrap_or_else(|| std::path::Path::new(""));
    let to_no_ext = std::path::Path::new(to_file).with_extension("");

    let from_parts: Vec<&std::ffi::OsStr> = from_dir.iter().collect();
    let to_parts: Vec<&std::ffi::OsStr> = to_no_ext.iter().collect();
    let common = from_parts.iter().zip(to_parts.iter()).take_while(|(a, b)| a == b).count();

    let mut parts: Vec<String> = (common..from_parts.len()).map(|_| "..".to_string()).collect();
    parts.extend(to_parts[common..].iter().map(|p| p.to_string_lossy().to_string()));

    let joined = parts.join("/");
    if joined.starts_with('.') { joined } else { format!("./{joined}") }
}

#[cfg(test)]
mod relative_import_specifier_tests {
    use super::relative_import_specifier;

    #[test]
    fn sibling_directories_under_src() {
        assert_eq!(
            relative_import_specifier(
                "services/product/src/services/ProductService.ts",
                "services/product/src/models/Product.ts",
            ),
            "../models/Product"
        );
    }

    #[test]
    fn same_directory() {
        assert_eq!(
            relative_import_specifier(
                "services/product/src/models/Product.ts",
                "services/product/src/models/helpers.ts",
            ),
            "./helpers"
        );
    }

    #[test]
    fn nested_deeper_target() {
        assert_eq!(
            relative_import_specifier(
                "services/product/src/routes/products.ts",
                "services/product/src/services/product/ProductService.ts",
            ),
            "../services/product/ProductService"
        );
    }
}

/// Finds how a just-generated test file actually calls the subject under test — e.g.
/// `service.registerProduct(productData)` — and returns it as a ready-to-inject call-shape
/// snippet (`"registerProduct(productData)"`), or `None` if it can't be determined
/// deterministically (no `new <class_name>(...)` found, or the calls found disagree with each
/// other on method name or argument count).
///
/// This doesn't touch the Roots index at all — it's a one-off parse of in-memory source that
/// doesn't exist on disk yet (or was just written but not yet reindexed), not a query against
/// previously-indexed symbols. That's deliberate: waiting for a reindex here would race against
/// the very call site this is meant to inform (stub generation, which runs before the file is
/// ever indexed). It's also why this is the one place canopy still links a roots-* crate
/// (`roots-parser`) directly rather than shelling out — there's no live Roots index or CLI
/// command involved, just a plain tree-sitter parsing utility applied to unsaved content (see
/// CLAUDE.md's Roots section for why this is a deliberate, narrow exception).
///
/// Disagreement across call sites (rather than picking one arbitrarily) is itself useful
/// information withheld here on purpose — silently trusting an inconsistent test would be worse
/// than falling back to the existing self-check instruction in the stub prompt.
pub fn find_test_call_shape(test_content: &str, class_name: &str) -> Option<String> {
    let calls = roots_parser::find_subject_calls(test_content, class_name);
    let first = calls.first()?;
    let consistent = calls.iter()
        .all(|c| c.method_name == first.method_name && c.argument_texts.len() == first.argument_texts.len());
    if !consistent {
        return None;
    }
    Some(format!("{}({})", first.method_name, first.argument_texts.join(", ")))
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
    let _ = std::process::Command::new("roots")
        .arg("index")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
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
