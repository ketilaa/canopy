//! Reads stored batches (Planner or Implementer) and groups what went wrong into recurring classes, so the
//! dominant remaining cost is visible instead of re-derived by hand from each run.
//!
//! usage: analyze_runs [--json] <batch dir>...
//!        analyze_runs --step <run dir> <step id>
//!
//! `--step` prints the causal packet of one step of one stored run: what it was told and allowed, what the
//! earlier steps left, what the model did and what the harness said. Read it before naming an origin.
//!
//! A Planner batch is a directory of `run-NN/summary.json` with `attempt_problems`; an Implementer batch is
//! one whose runs have an `events.jsonl`. Planner runs that were later run by the Implementer carry an
//! `implementer-runs.jsonl` next to their plan; those results are joined in.

use std::path::{Path, PathBuf};

use canopy_implementer::report::{class_of, FailureClasses, RunReport, StepPacket};
use serde_json::{json, Value};

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn run_dirs(batch: &Path) -> Vec<PathBuf> {
    let mut runs: Vec<PathBuf> = std::fs::read_dir(batch)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    runs.sort();
    runs
}

fn print_step_packet(run: &Path, step_id: &str) {
    let text = std::fs::read_to_string(run.join("events.jsonl")).unwrap_or_else(|e| {
        eprintln!("cannot read {}/events.jsonl: {e}", run.display());
        std::process::exit(1)
    });
    let events: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .map(|v| v.get("event").cloned().unwrap_or(v))
        .collect();
    match StepPacket::from_events(&events, step_id) {
        Some(packet) => println!("{}", packet.render()),
        None => {
            eprintln!("step {step_id} did not run in {}", run.display());
            std::process::exit(1)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, run, step] = args.as_slice() {
        if flag == "--step" {
            print_step_packet(Path::new(run), step);
            return;
        }
    }
    let mut json_output = false;
    let mut batches: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--json" => json_output = true,
            "--help" | "-h" => {
                eprintln!(
                    "usage: analyze_runs [--json] <batch dir>...\n       analyze_runs --step <run dir> <step id>"
                );
                return;
            }
            other => batches.push(other.into()),
        }
    }
    let mut classes = FailureClasses::default();
    let mut lines: Vec<String> = Vec::new();
    let (mut planner_runs, mut planner_invalid) = (0, 0);
    let (mut implementer_runs, mut implementer_stopped) = (0, 0);

    for batch in &batches {
        let meta = read_json(&batch.join("batch.json")).unwrap_or(Value::Null);
        lines.push(format!(
            "\n== {} (harness {}{}, task {})",
            batch.display(),
            meta["harness_commit"].as_str().unwrap_or("?"),
            if meta["harness_dirty"].as_bool() == Some(true) {
                ", dirty"
            } else {
                ""
            },
            meta["task"].as_str().unwrap_or("?")
        ));
        for run in run_dirs(batch) {
            let name = run.file_name().unwrap().to_string_lossy().to_string();
            let summary = read_json(&run.join("summary.json")).unwrap_or(Value::Null);
            if summary.get("attempt_problems").is_some() {
                planner_runs += 1;
                let valid = summary["valid"].as_bool() == Some(true);
                if !valid {
                    planner_invalid += 1;
                }
                // What was still wrong at the end: the classes of the last attempt's problems, once each.
                let last = summary["attempt_problems"]
                    .as_array()
                    .and_then(|a| a.last())
                    .cloned()
                    .unwrap_or(Value::Null);
                let mut seen = std::collections::BTreeSet::new();
                for problem in last.as_array().unwrap_or(&vec![]) {
                    let label = match problem["rule"].as_str() {
                        Some(rule) => format!("rule {rule}"),
                        None => class_of(problem["text"].as_str().unwrap_or("")),
                    };
                    if seen.insert(label.clone()) {
                        classes.add("planner", &label);
                    }
                }
                // Recorded, not counted as a failure: a valid plan whose instruction leaves a listed class unnamed.
                let noted = summary["diagnostics"].as_array().map_or(0, Vec::len);
                if noted > 0 {
                    classes.add("planner-note", "a listed class is not named in the instruction");
                }
                let joined: Vec<Value> = std::fs::read_to_string(run.join("implementer-runs.jsonl"))
                    .map(|t| t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
                    .unwrap_or_default();
                let mut line = format!(
                    "  {name} planner: valid={valid} attempts={} steps={} notes={noted}",
                    summary["attempts"], summary["steps"]
                );
                for j in &joined {
                    implementer_runs += 1;
                    line.push_str(&format!(
                        "\n      -> implementer {}: {}/{} steps, stopped at {}",
                        j["implementer_batch"].as_str().unwrap_or("?"),
                        j["steps_passed"],
                        j["steps_total"],
                        j["failed_step"].as_str().unwrap_or("-")
                    ));
                    if let Some(stage) = j["stop_reason"]["stage"].as_str() {
                        implementer_stopped += 1;
                        let label = format!(
                            "stop {stage}: {}",
                            class_of(j["stop_reason"]["feedback"].as_str().unwrap_or(""))
                        );
                        classes.add("implementer", &label);
                    }
                    for (reason, count) in j["refusals"].as_object().into_iter().flatten() {
                        line.push_str(&format!("\n      refused {count}x: {reason}"));
                        classes.add("implementer", &format!("refusal: {reason}"));
                    }
                }
                lines.push(line);
            } else if run.join("events.jsonl").exists() {
                let report = std::fs::read_to_string(run.join("events.jsonl"))
                    .map(|t| RunReport::from_jsonl(&t))
                    .unwrap_or_default();
                implementer_runs += 1;
                let mut line = format!(
                    "  {name} implementer: {}/{} steps",
                    report.steps_passed, report.steps_total
                );
                if let Some(failed) = report.steps.iter().find(|s| !s.passed) {
                    implementer_stopped += 1;
                    line.push_str(&format!(", stopped at {}", failed.step_id));
                    if let Some(stop) = &failed.stop_reason {
                        let label = format!("stop {}: {}", stop.stage, class_of(&stop.feedback));
                        line.push_str(&format!(" ({label})"));
                        classes.add("implementer", &label);
                    }
                    for (reason, count) in &failed.refusals {
                        line.push_str(&format!("\n      refused {count}x: {reason}"));
                        classes.add("implementer", &format!("refusal: {reason}"));
                    }
                }
                lines.push(line);
            }
        }
    }

    let ranked = classes.ranked();
    if json_output {
        let value = json!({
            "planner": { "runs": planner_runs, "invalid": planner_invalid },
            "implementer": { "runs": implementer_runs, "stopped": implementer_stopped },
            "ranked": ranked.iter().map(|(s, l, n)| json!({"source": s, "class": l, "count": n})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
        return;
    }
    for line in &lines {
        println!("{line}");
    }
    println!("\n== failure classes, most frequent first (a class counts once per run)");
    for (source, label, count) in ranked.iter().take(15) {
        println!("  {count:>3}  {source:<11} {label}");
    }
    println!(
        "\n== attribution: planner {planner_invalid}/{planner_runs} plans invalid at the end; implementer {implementer_stopped}/{implementer_runs} runs stopped before the last step"
    );
    match ranked.first() {
        Some((source, label, count)) if *count > 1 => {
            println!(
                "== dominant: {source}: {label} ({count} of {} counted)",
                classes.total()
            );
        }
        Some(_) => println!("== dominant: none yet, every class occurs once; too few runs to rank"),
        None => println!("== dominant: nothing failed in these runs"),
    }
}
