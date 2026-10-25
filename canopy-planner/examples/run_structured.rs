//! Phase 2 arm A1 (`docs/design/structured-plan-schema-introduction-plan.md`): drafts each unit's
//! `examples` and `methods[].throws` from the fixed `api`, the fixed `decisions`, and the model, then
//! renders the result down to the shape `run_step` runs. Writes, per run, in the layout
//! `score_structured.py` reads: `plan.structured.yaml` (the model's draft), `plan.yaml` and `plan.md`
//! (rendered by `structured::to_legacy_plan`), `summary.json`, `events.jsonl`.
//!
//! usage: run_structured [--task tickets] [--runs N] [--url URL] [--model ID] [--out DIR]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use canopy_implementer::llm::{LlmClient, ModelEndpoint, Sampling};
use canopy_implementer::observe::{now_iso, MetricsObserver, Observers, RunEvent, RunObserver, TraceWriter};
use canopy_planner::gherkin::Feature;
use canopy_planner::render::render_markdown;
use canopy_planner::structured::{draft_structured, to_legacy_plan, StructuredPlan};
use serde_json::json;

struct Task {
    name: &'static str,
    feature: &'static str,
    structured: &'static str,
}

/// The only structured fixture authored so far (Phase 1's golden tickets plan). A second task is added
/// the same way Phase 3 of the introduction plan asks for — a hand-authored `plan.structured.yaml`
/// next to its `feature.feature` — not by generalising this match arm speculatively first.
fn task(name: &str) -> Result<Task, String> {
    match name {
        "tickets" => Ok(Task {
            name: "tickets",
            feature: include_str!("../../canopy-implementer/tasks/tickets/feature.feature"),
            structured: include_str!("../../canopy-implementer/tasks/tickets/plan.structured.yaml"),
        }),
        other => Err(format!(
            "unknown task {other}; the only structured fixture authored so far is tickets"
        )),
    }
}

struct Console;

impl RunObserver for Console {
    fn on(&self, event: &RunEvent) {
        match event {
            RunEvent::LlmRequestSent(r) => println!("  -> {} (call {})", r.purpose, r.call_id),
            RunEvent::LlmResponseReceived(r) => println!(
                "  <- {}: {} prompt + {} completion tokens, {:.1}s, finish: {}",
                r.purpose,
                r.prompt_tokens,
                r.completion_tokens,
                r.elapsed_ms as f64 / 1000.0,
                r.finish_reason
            ),
            _ => {}
        }
    }
}

fn main() {
    if let Err(message) = real_main() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let mut task_name = "tickets".to_string();
    let mut runs = 1u32;
    let mut url = "http://localhost:8080".to_string();
    let mut model = "Qwen/Qwen3-14B-GGUF:Q4_K_M".to_string();
    let mut out: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("runs");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--task" => task_name = value("--task")?,
            "--runs" => runs = value("--runs")?.parse().map_err(|e| format!("--runs: {e}"))?,
            "--url" => url = value("--url")?,
            "--model" => model = value("--model")?,
            "--out" => out = value("--out")?.into(),
            other => return Err(format!("unknown option {other}")),
        }
    }
    let task = task(&task_name)?;
    let feature = Feature::parse(task.feature).map_err(|e| e.to_string())?;
    let fixed = StructuredPlan::from_yaml(task.structured).map_err(|e| e.to_string())?;
    let batch_id = now_iso()[..19].replace(':', "-");
    let batch_dir = out.join(&batch_id);
    std::fs::create_dir_all(&batch_dir).map_err(|e| e.to_string())?;
    println!("batch {batch_id}: structured draft (A1), task {}, {runs} run(s), model {model}", task.name);
    let mut valid = 0;
    for n in 1..=runs {
        println!("\n== run {n}/{runs} ==");
        let run_dir = batch_dir.join(format!("run-{n:02}"));
        std::fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;
        let metrics = Arc::new(MetricsObserver::new());
        let trace = TraceWriter::create(&run_dir.join("events.jsonl"), &format!("{batch_id}/run-{n:02}"))
            .map_err(|e| e.to_string())?;
        let observers: Arc<Observers> =
            Arc::new(Observers::new().with(Arc::new(trace)).with(metrics.clone()).with(Arc::new(Console)));
        let client = LlmClient::new(
            ModelEndpoint { base_url: url.clone(), model: model.clone(), timeout: Duration::from_secs(600) },
            Sampling { max_tokens: 2048, ..Sampling::qwen3() },
            observers,
        );
        let result = draft_structured(&client, &fixed, &feature);
        let (valid_now, error, legacy_steps) = match &result {
            Ok(drafted) => {
                std::fs::write(run_dir.join("plan.structured.yaml"), serde_yaml::to_string(drafted).unwrap())
                    .map_err(|e| e.to_string())?;
                match to_legacy_plan(drafted) {
                    Ok(plan) => {
                        std::fs::write(run_dir.join("plan.yaml"), plan.to_yaml()).map_err(|e| e.to_string())?;
                        std::fs::write(run_dir.join("plan.md"), render_markdown(&plan, &feature))
                            .map_err(|e| e.to_string())?;
                        (true, None, plan.steps.len())
                    }
                    Err(e) => (false, Some(format!("drafted but did not render: {e}")), 0),
                }
            }
            Err(e) => (false, Some(e.clone()), 0),
        };
        if valid_now {
            valid += 1;
        }
        let summary = json!({
            "run": n,
            "valid": valid_now,
            "error": error,
            "steps": legacy_steps,
            "metrics": metrics.snapshot(),
        });
        std::fs::write(run_dir.join("summary.json"), serde_json::to_string_pretty(&summary).unwrap())
            .map_err(|e| e.to_string())?;
        println!("  drafted: {valid_now}; steps: {legacy_steps}; error: {error:?}");
    }
    println!("\n{valid}/{runs} plans drafted. Artifacts: {}", batch_dir.display());
    Ok(())
}
