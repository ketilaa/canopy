use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use roots_core::Language;
use roots_parser::extract;

use crate::db::{open_store, resolve_workspace};
use crate::discover::{collect_source_files, discover_projects};
use crate::output;

pub fn run(workspace: Option<&str>, path: &str) -> Result<(), String> {
    let workspace_id = resolve_workspace(workspace)?;

    let root = Path::new(path)
        .canonicalize()
        .map_err(|e| format!("cannot access path {path}: {e}"))?;

    let store = open_store()?;
    let timestamp = iso_now();

    let projects = discover_projects(&root);
    if projects.is_empty() {
        output::json(&serde_json::json!({
            "projects": 0,
            "files": 0,
            "symbols": 0,
            "message": "no projects found"
        }));
        return Ok(());
    }

    let mut total_files = 0i64;
    let mut total_symbols = 0i64;
    let mut total_relationships = 0i64;

    store.begin_transaction().map_err(|e| e.to_string())?;

    let result: Result<(), String> = (|| {
        for project in &projects {
            let project_id = store
                .upsert_project(&workspace_id, &project.name, &project.path, &project.language)
                .map_err(|e| e.to_string())?;

            let source_files = collect_source_files(project);

            for file_path in &source_files {
                let relative = file_path
                    .strip_prefix(&root)
                    .unwrap_or(file_path)
                    .to_string_lossy()
                    .to_string();

                // Dispatch by file extension so Java files in a Kotlin/Gradle project
                // are parsed by the Java extractor, not the Kotlin one.
                let file_language = file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .and_then(Language::from_extension)
                    .unwrap_or_else(|| project.language.clone());

                let file_id = store
                    .upsert_file(&workspace_id, project_id, &relative, &file_language, &timestamp)
                    .map_err(|e| e.to_string())?;

                store.delete_symbols_for_file(file_id).map_err(|e| e.to_string())?;
                store.delete_relationships_for_file(&workspace_id, &relative).map_err(|e| e.to_string())?;

                let source = match std::fs::read(file_path) {
                    Ok(b) => b,
                    Err(_) => continue,
                };

                let parse_output = match extract(&source, &file_language, &relative, &project.name, &workspace_id) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                total_symbols += parse_output.symbols.len() as i64;
                total_relationships += parse_output.relationships.len() as i64;
                store
                    .insert_symbols(&workspace_id, project_id, file_id, &parse_output.symbols)
                    .map_err(|e| e.to_string())?;
                store
                    .insert_relationships(&workspace_id, &relative, &parse_output.relationships)
                    .map_err(|e| e.to_string())?;

                total_files += 1i64;
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            store.commit_transaction().map_err(|e| e.to_string())?;
            output::json(&serde_json::json!({
                "workspace": workspace_id,
                "projects": projects.len(),
                "files": total_files,
                "symbols": total_symbols,
                "relationships": total_relationships
            }));
            Ok(())
        }
        Err(e) => {
            let _ = store.rollback_transaction();
            Err(e)
        }
    }
}

fn iso_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = secs_to_parts(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn secs_to_parts(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let mins = secs / 60;
    let mi = mins % 60;
    let hrs = mins / 60;
    let h = hrs % 24;
    let days = hrs / 24;

    let mut year = 1970u64;
    let mut remaining = days;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if remaining < dy {
            break;
        }
        remaining -= dy;
        year += 1;
    }
    let leap = is_leap(year);
    let months = [31u64, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    for dm in &months {
        if remaining < *dm {
            break;
        }
        remaining -= dm;
        month += 1;
    }
    (year, month, remaining + 1, h, mi, s)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}
