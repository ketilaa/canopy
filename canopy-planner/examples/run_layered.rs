//! Runs the layered Planner (steps from the API, rules and cases from the model; see `layered.rs`) and writes one run
//! directory per plan in the layout `score_planner.py` reads: `plan.yaml`, `summary.json`, `events.jsonl`.
//!
//! usage: run_layered [--task order|loans|shipping|stock|tickets] [--runs N] [--url URL] [--model ID] [--out DIR]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use canopy_implementer::llm::{LlmClient, ModelEndpoint, Sampling};
use canopy_implementer::observe::{now_iso, MetricsObserver, Observers, RunEvent, RunObserver, TraceWriter};
use canopy_implementer::plan::Plan;
use canopy_planner::gherkin::Feature;
use canopy_planner::layered::{parse_api, plan_layered, Input};
use canopy_planner::render::render_markdown;
use serde_json::json;

struct Task {
    name: &'static str,
    feature_path: &'static str,
    feature: &'static str,
    /// A hand-written plan of the story, read only for the story text.
    story_plan: &'static str,
    api: &'static str,
}

macro_rules! task {
    ($name:literal) => {
        Task {
            name: $name,
            feature_path: concat!("tasks/", $name, "/feature.feature"),
            feature: include_str!(concat!("../../canopy-implementer/tasks/", $name, "/feature.feature")),
            story_plan: include_str!(concat!("../../canopy-implementer/tasks/", $name, "/plan-signatures.yaml")),
            api: include_str!(concat!("../../canopy-implementer/tasks/", $name, "/api.txt")),
        }
    };
}

fn task(name: &str) -> Result<Task, String> {
    match name {
        "order" => Ok(task!("order")),
        "loans" => Ok(task!("loans")),
        "shipping" => Ok(task!("shipping")),
        "stock" => Ok(task!("stock")),
        "tickets" => Ok(task!("tickets")),
        other => Err(format!("unknown task {other}; known: order, loans, shipping, stock, tickets")),
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
    let mut task_name = "stock".to_string();
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
    let story = Plan::from_yaml(task.story_plan)
        .map_err(|e| e.to_string())?
        .story
        .ok_or("the story plan has no story")?;
    let api = parse_api(task.api)?;
    let batch_id = now_iso()[..19].replace(':', "-");
    let batch_dir = out.join(&batch_id);
    std::fs::create_dir_all(&batch_dir).map_err(|e| e.to_string())?;
    println!("batch {batch_id}: layered, task {}, {runs} run(s), model {model}", task.name);
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
            Sampling { max_tokens: 4096, ..Sampling::qwen3() },
            observers,
        );
        let result = plan_layered(
            &client,
            &Input { id: task.name, story: &story, feature_path: Some(task.feature_path), feature: &feature, api: &api },
        );
        if let Some(plan) = &result.plan {
            valid += 1;
            std::fs::write(run_dir.join("plan.yaml"), plan.to_yaml()).map_err(|e| e.to_string())?;
            std::fs::write(run_dir.join("plan.md"), render_markdown(plan, &feature)).map_err(|e| e.to_string())?;
        }
        let steps = result.plan.as_ref().map_or(0, |p| p.steps.len());
        let summary = json!({
            "run": n,
            "valid": result.plan.is_some(),
            "steps": steps,
            "missing_literals": result.notes.missing_literals,
            "uncovered_scenarios": result.notes.uncovered_scenarios,
            "failed_units": result.notes.failed_units,
            "dropped_cases": result.notes.dropped_cases,
            "metrics": metrics.snapshot(),
        });
        std::fs::write(run_dir.join("summary.json"), serde_json::to_string_pretty(&summary).unwrap())
            .map_err(|e| e.to_string())?;
        println!(
            "  plan: {}; steps: {steps}; missing literals: {:?}; uncovered scenarios: {:?}; dropped cases: {}",
            result.plan.is_some(),
            result.notes.missing_literals,
            result.notes.uncovered_scenarios,
            result.notes.dropped_cases.len()
        );
    }
    println!("\n{valid}/{runs} plans. Artifacts: {}", batch_dir.display());
    Ok(())
}
