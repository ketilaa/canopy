//! Replays the import audit over the submissions of past runs that the harness accepted and the build passed.
//! Any file the audit would have refused is a false positive: that file compiled.
//!
//! Usage: cargo run -p canopy-implementer --example audit_replay -- <runs dir> [<runs dir> ...]
use std::collections::HashMap;

use canopy_implementer::java;

type Submitted = Vec<(String, String)>;

fn main() {
    let mut files_checked = 0;
    let mut flagged: Vec<(String, String, Vec<String>)> = Vec::new();
    let (mut compile_failures, mut missing_class_failures, mut caught) = (0, 0, 0);
    for dir in std::env::args().skip(1) {
        let runs: Vec<_> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .flat_map(|batch| std::fs::read_dir(batch.path()).into_iter().flatten().flatten())
            .collect();
        for run in runs {
            let events = run.path().join("events.jsonl");
            let Ok(text) = std::fs::read_to_string(&events) else { continue };
            let mut step = String::new();
            let mut last_submit: HashMap<String, Submitted> = HashMap::new();
            let mut passed_steps: Vec<String> = Vec::new();
            let mut pending: HashMap<String, Submitted> = HashMap::new();
            let mut failures: Vec<(String, Submitted, String)> = Vec::new();
            for line in text.lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
                let ev = &v["event"];
                match ev["type"].as_str() {
                    Some("step_started") => step = ev["step_id"].as_str().unwrap_or("").to_string(),
                    Some("tool_called") if ev["tool"] == "submit_files" && ev["outcome"] == "ok" => {
                        if let Ok(args) = serde_json::from_str::<serde_json::Value>(ev["arguments"].as_str().unwrap_or("")) {
                            let files: Vec<(String, String)> = args["files"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .map(|f| {
                                            (
                                                f["path"].as_str().unwrap_or("").to_string(),
                                                f["content"].as_str().unwrap_or("").to_string(),
                                            )
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            pending.insert(step.clone(), files.clone());
                            last_submit.insert(step.clone(), files);
                        }
                    }
                    Some("attempt_failed") if ev["stage"] == "compile" => {
                        let feedback = ev["feedback"].as_str().unwrap_or("").to_string();
                        failures.push((step.clone(), pending.get(&step).cloned().unwrap_or_default(), feedback));
                    }
                    Some("step_finished") if ev["passed"] == true => passed_steps.push(ev["step_id"].as_str().unwrap_or("").to_string()),
                    _ => {}
                }
            }
            // The classes this run created, so the index matches what was on disk.
            let mut index: HashMap<String, Vec<String>> = HashMap::new();
            for step_id in &passed_steps {
                for (path, _) in last_submit.get(step_id).into_iter().flatten() {
                    if let (Some(q), Some(stem)) = (java::qualified_name(path), std::path::Path::new(path).file_stem()) {
                        index.entry(stem.to_string_lossy().into_owned()).or_default().push(q);
                    }
                }
            }
            for (_, files, feedback) in &failures {
                compile_failures += 1;
                let wanted: Vec<&str> = feedback
                    .lines()
                    .filter_map(|l| l.trim().strip_prefix("symbol:").and_then(|s| s.trim().strip_prefix("class ")))
                    .map(str::trim)
                    .collect();
                if wanted.is_empty() {
                    continue;
                }
                missing_class_failures += 1;
                let mut all = index.clone();
                for (path, _) in files {
                    if let (Some(q), Some(stem)) = (java::qualified_name(path), std::path::Path::new(path).file_stem()) {
                        let e = all.entry(stem.to_string_lossy().into_owned()).or_default();
                        if !e.contains(&q) {
                            e.push(q);
                        }
                    }
                }
                let reported: Vec<String> = files.iter().flat_map(|(p, c)| java::missing_imports(p, c, &all)).collect();
                if wanted.iter().all(|w| reported.iter().any(|r| r.ends_with(&format!(".{w}")))) {
                    caught += 1;
                }
            }
            for step_id in &passed_steps {
                for (path, content) in last_submit.get(step_id).into_iter().flatten() {
                    files_checked += 1;
                    let missing = java::missing_imports(path, content, &index);
                    if !missing.is_empty() {
                        flagged.push((run.path().display().to_string(), path.clone(), missing));
                    }
                }
            }
        }
    }
    println!("files of passed steps checked: {files_checked}; flagged: {}", flagged.len());
    println!(
        "compile failures: {compile_failures}; of those a missing class: {missing_class_failures}; the audit names every missing class in: {caught}"
    );
    for (run, path, missing) in flagged {
        println!("{run}: {path}: {missing:?}");
    }
}
