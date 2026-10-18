//! Designs the steps of a stored plan with the model: the classes of each step, how a test builds its subject and
//! the cases to check, written into the step's instruction (see `design.rs`). The plan and its repository are
//! read from a planner run directory; `plan.designed.yaml` and `design-notes.txt` are written beside them.
//!
//! usage: design_plan [--task widgets|widgets-create|widgets-invalid|order|loans|shipping|stock|tickets] [--url URL] [--model ID] [--plan FILE] <planner run dir>...
//!        (the plan defaults to plan.rederived.yaml, then plan.yaml)
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use canopy_implementer::classpath::ClassIndex;
use canopy_implementer::llm::{LlmClient, ModelEndpoint, Sampling};
use canopy_implementer::observe::{NoopObserver, RunObserver};
use canopy_implementer::plan::Plan;
use canopy_planner::design::design_plan;
use canopy_planner::facts::RepoFacts;
use canopy_planner::gherkin::Feature;

const FEATURES: [(&str, &str); 8] = [
    (
        "widgets",
        include_str!("../../canopy-implementer/tasks/widgets/feature.feature"),
    ),
    (
        "widgets-create",
        include_str!("../../canopy-implementer/tasks/widgets-create/feature.feature"),
    ),
    (
        "widgets-invalid",
        include_str!("../../canopy-implementer/tasks/widgets-invalid/feature.feature"),
    ),
    (
        "order",
        include_str!("../../canopy-implementer/tasks/order/feature.feature"),
    ),
    (
        "loans",
        include_str!("../../canopy-implementer/tasks/loans/feature.feature"),
    ),
    (
        "shipping",
        include_str!("../../canopy-implementer/tasks/shipping/feature.feature"),
    ),
    (
        "stock",
        include_str!("../../canopy-implementer/tasks/stock/feature.feature"),
    ),
    (
        "tickets",
        include_str!("../../canopy-implementer/tasks/tickets/feature.feature"),
    ),
];

fn main() {
    let mut url = "http://localhost:8080".to_string();
    let mut model = "Qwen/Qwen3-14B-GGUF:Q4_K_M".to_string();
    let mut plan_file: Option<String> = None;
    let mut task = "widgets".to_string();
    let mut dirs = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--url" => url = args.next().expect("--url needs a value"),
            "--model" => model = args.next().expect("--model needs a value"),
            "--plan" => plan_file = Some(args.next().expect("--plan needs a value")),
            "--task" => task = args.next().expect("--task needs a value"),
            other => dirs.push(other.to_string()),
        }
    }
    let observer: Arc<dyn RunObserver> = Arc::new(NoopObserver);
    let client = LlmClient::new(
        ModelEndpoint {
            base_url: url,
            model,
            timeout: Duration::from_secs(600),
        },
        Sampling {
            max_tokens: 2048,
            ..Sampling::qwen3()
        },
        observer,
    );
    for dir in dirs {
        match design(&client, Path::new(&dir), plan_file.as_deref(), &task) {
            Ok(note) => println!("{dir}: {note}"),
            Err(e) => println!("{dir}: not designed: {e}"),
        }
    }
}

fn design(client: &LlmClient, dir: &Path, plan_file: Option<&str>, task: &str) -> Result<String, String> {
    let source = match plan_file {
        Some(name) => dir.join(name),
        None if dir.join("plan.rederived.yaml").exists() => dir.join("plan.rederived.yaml"),
        None => dir.join("plan.yaml"),
    };
    let plan = Plan::from_yaml(&std::fs::read_to_string(&source).map_err(|e| format!("{}: {e}", source.display()))?)
        .map_err(|e| e.to_string())?;
    let root = dir.join("repo");
    let facts = RepoFacts::collect(&root).map_err(|e| e.to_string())?;
    let text = FEATURES
        .iter()
        .find(|(name, _)| *name == task)
        .map(|(_, text)| *text)
        .ok_or_else(|| format!("unknown task {task}"))?;
    let feature = Feature::parse(text).map_err(|e| e.to_string())?;
    let index = ClassIndex::for_repository(&root);
    println!("  classpath resolved: {}", index.resolved());
    let designed = design_plan(client, &plan, &facts, &feature, &root, Some(&index));
    std::fs::write(dir.join("plan.designed.yaml"), designed.plan.to_yaml()).map_err(|e| e.to_string())?;
    let notes: Vec<String> = designed
        .notes
        .iter()
        .map(|n| format!("{}: {}", n.step_id, n.problems.join("; ")))
        .collect();
    std::fs::write(dir.join("design-notes.txt"), notes.join("\n")).map_err(|e| e.to_string())?;
    Ok(format!(
        "{} steps, {} left undesigned{}",
        designed.plan.steps.len(),
        designed.notes.len(),
        if notes.is_empty() {
            String::new()
        } else {
            format!(" ({})", notes.join(" | "))
        }
    ))
}
