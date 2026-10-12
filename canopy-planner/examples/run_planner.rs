//! Runs the Planner against a dogfooding repository and records everything: the facts, every attempt,
//! the verifier's problems, the plan, and how it compares with the hand-written one.
//!
//! The repository is exported from its tag with `git archive` into the run directory, so the
//! dogfooding repository itself is never touched. To score a generated plan end to end, hand it to the
//! Implementer's runner: `cargo run -p canopy-implementer --example run_step -- --task widgets-planned
//! --plan <run dir>/plan.yaml`.
//!
//! usage: run_planner [--task widgets|widgets-create|widgets-invalid] [--runs N] [--tag TAG] [--url URL] [--model ID] [--out DIR]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use canopy_implementer::llm::{LlmClient, ModelEndpoint, Sampling};
use canopy_implementer::observe::{now_iso, MetricsObserver, Observers, RunEvent, RunObserver, TraceWriter};
use canopy_implementer::plan::Plan;
use canopy_implementer::report::content_id;
use canopy_planner::facts::RepoFacts;
use canopy_planner::gherkin::Feature;
use canopy_planner::planner::{Outcome, Planner, Problem, Request};
use canopy_planner::render::{compare_created_files, render_markdown};
use canopy_planner::verify::Rule;
use serde_json::json;

struct Task {
    name: &'static str,
    repo: &'static str,
    tag: &'static str,
    feature_path: &'static str,
    feature: &'static str,
    /// The story the plan implements.
    story: &'static str,
    /// The hand-written plan the draft is compared with, when there is one.
    golden: Option<&'static str>,
}

const WIDGETS: Task = Task {
    name: "widgets",
    repo: "java-widgets",
    tag: "baseline-00-widgets",
    feature_path: "tasks/widgets/feature.feature",
    feature: include_str!("../../canopy-implementer/tasks/widgets/feature.feature"),
    story: "A client can create a widget with POST /widgets, giving a name. The widget gets a generated id. A name that is blank, missing, or longer than 50 characters is rejected.",
    golden: Some(include_str!("../../canopy-implementer/tasks/widgets/plan.yaml")),
};

/// The success path of the widgets story, on the repository as it is at the contract tag.
const WIDGETS_CREATE: Task = Task {
    name: "widgets-create",
    repo: "java-widgets",
    tag: "baseline-06-widgets-contract",
    feature_path: "tasks/widgets-create/feature.feature",
    feature: include_str!("../../canopy-implementer/tasks/widgets-create/feature.feature"),
    story: "A client can create a widget with POST /widgets, giving a name. The widget gets a generated id, which is a UUID.",
    golden: None,
};

/// The invalid-name paths, on a repository where the success path is already built (pass --tag).
const WIDGETS_INVALID: Task = Task {
    name: "widgets-invalid",
    repo: "java-widgets",
    tag: "baseline-06-widgets-contract",
    feature_path: "tasks/widgets-invalid/feature.feature",
    feature: include_str!("../../canopy-implementer/tasks/widgets-invalid/feature.feature"),
    story: "A widget name that is blank, missing, or longer than 50 characters is rejected with status 400. A name of exactly 50 characters is accepted.",
    golden: None,
};

struct Config {
    task: String,
    runs: u32,
    tag: Option<String>,
    url: String,
    model: String,
    out: PathBuf,
}

const USAGE: &str = "usage: run_planner [--task widgets|widgets-create|widgets-invalid] [--runs N] [--tag TAG] [--url URL] [--model ID] [--out DIR]";

fn parse_args() -> Result<Config, String> {
    let mut config = Config {
        task: "widgets".into(),
        runs: 1,
        tag: None,
        url: "http://localhost:8080".into(),
        model: "Qwen/Qwen3-14B-GGUF:Q4_K_M".into(),
        out: Path::new(env!("CARGO_MANIFEST_DIR")).join("runs"),
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--help" | "-h" => return Err(USAGE.into()),
            "--task" => config.task = value("--task")?,
            "--runs" => config.runs = value("--runs")?.parse().map_err(|e| format!("--runs: {e}"))?,
            "--tag" => config.tag = Some(value("--tag")?),
            "--url" => config.url = value("--url")?,
            "--model" => config.model = value("--model")?,
            "--out" => config.out = value("--out")?.into(),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    Ok(config)
}

fn task(name: &str) -> Result<&'static Task, String> {
    match name {
        "widgets" => Ok(&WIDGETS),
        "widgets-create" => Ok(&WIDGETS_CREATE),
        "widgets-invalid" => Ok(&WIDGETS_INVALID),
        other => Err(format!(
            "unknown task {other}; known: widgets, widgets-create, widgets-invalid"
        )),
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    match output.status.success() {
        true => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        false => Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

/// The repository as it is at `tag`, exported to `dest`. The repository itself is not touched.
fn export(repo: &Path, tag: &str, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let archive = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["archive", "--format=tar", tag])
        .output()
        .map_err(|e| e.to_string())?;
    if !archive.status.success() {
        return Err(format!(
            "git archive {tag}: {}",
            String::from_utf8_lossy(&archive.stderr)
        ));
    }
    let mut tar = Command::new("tar")
        .args(["-x", "-C"])
        .arg(dest)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut tar.stdin.take().unwrap(), &archive.stdout).map_err(|e| e.to_string())?;
    tar.wait()
        .map_err(|e| e.to_string())?
        .success()
        .then_some(())
        .ok_or_else(|| "tar failed".to_string())
}

/// Prints each model call as it starts and ends.
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
            RunEvent::LlmCallFailed(f) => println!("  !! {}: {:?} {}", f.purpose, f.kind, f.message),
            _ => {}
        }
    }
}

fn rule_name(rule: Rule) -> &'static str {
    match rule {
        Rule::Repository => "repository",
        Rule::Coverage => "coverage",
        Rule::Location => "location",
        Rule::MissingTest => "missing_test",
        Rule::StepSize => "step_size",
        Rule::Redundant => "redundant",
    }
}

fn problem_json(problem: &Problem) -> serde_json::Value {
    match problem {
        Problem::Model(text) => json!({"kind": "model", "text": text}),
        Problem::Unparseable(text) => json!({"kind": "unparseable", "text": text}),
        Problem::Rule(issue) => {
            json!({"kind": "rule", "rule": rule_name(issue.rule), "step": issue.step, "text": issue.to_string()})
        }
    }
}

struct RunResult {
    n: u32,
    valid: bool,
    attempts: usize,
    first_draft_valid: bool,
    rules: Vec<&'static str>,
    steps: usize,
    files_match_golden: Option<bool>,
    metrics: canopy_implementer::observe::Metrics,
}

fn run_one(
    config: &Config,
    task: &Task,
    repo: &Path,
    tag: &str,
    batch_dir: &Path,
    batch_id: &str,
    n: u32,
) -> Result<RunResult, String> {
    let run_dir = batch_dir.join(format!("run-{n:02}"));
    let checkout = run_dir.join("repo");
    export(repo, tag, &checkout)?;
    println!("\n== run {n}/{} ==", config.runs);

    let facts = RepoFacts::collect(&checkout).map_err(|e| e.to_string())?;
    std::fs::write(run_dir.join("facts.txt"), facts.render()).map_err(|e| e.to_string())?;
    // The repository's own decisions: which contract (by content id), and where the repository disagrees with it.
    let contract_id = std::fs::read_to_string(checkout.join(canopy_planner::contract::CONTRACT_PATH))
        .ok()
        .map(|text| content_id(&text));
    let drift = facts.drift(&checkout);
    println!(
        "  contract: {}; drift: {}",
        contract_id.as_deref().unwrap_or("none"),
        if drift.is_empty() {
            "none".to_string()
        } else {
            drift.join("; ")
        }
    );
    let feature = Feature::parse(task.feature).map_err(|e| e.to_string())?;
    let golden = task
        .golden
        .map(|g| Plan::from_yaml(g).map_err(|e| e.to_string()))
        .transpose()?;
    let story = task.story.to_string();

    let metrics = Arc::new(MetricsObserver::new());
    let trace = TraceWriter::create(&run_dir.join("events.jsonl"), &format!("{batch_id}/run-{n:02}"))
        .map_err(|e| e.to_string())?;
    let observers: Arc<Observers> = Arc::new(
        Observers::new()
            .with(Arc::new(trace))
            .with(metrics.clone())
            .with(Arc::new(Console)),
    );
    let client = LlmClient::new(
        ModelEndpoint {
            base_url: config.url.clone(),
            model: config.model.clone(),
            timeout: Duration::from_secs(600),
        },
        sampling(),
        observers.clone(),
    );
    let planner = Planner::new(Box::new(client));
    let outcome: Outcome = planner.plan(&Request {
        id: task.name,
        story: &story,
        feature_path: Some(task.feature_path),
        feature: &feature,
        facts: &facts,
        root: &checkout,
    });

    for (index, attempt) in outcome.attempts.iter().enumerate() {
        let number = index + 1;
        if let Some(raw) = &attempt.raw {
            std::fs::write(run_dir.join(format!("attempt-{number}.json")), raw).map_err(|e| e.to_string())?;
        }
        let problems: Vec<_> = attempt.problems.iter().map(problem_json).collect();
        std::fs::write(
            run_dir.join(format!("attempt-{number}.problems.json")),
            serde_json::to_string_pretty(&problems).unwrap(),
        )
        .map_err(|e| e.to_string())?;
    }
    let mut diff_json = serde_json::Value::Null;
    let mut files_match_golden = None;
    if let Some(plan) = &outcome.plan {
        let yaml = plan.to_yaml();
        std::fs::write(run_dir.join("plan.yaml"), &yaml).map_err(|e| e.to_string())?;
        // What a later Implementer run needs to say "this is that plan", and who wrote each field.
        let meta = json!({
            "planner_batch": batch_id,
            "planner_run": n,
            "plan_content_id": content_id(&yaml),
            "contract_id": contract_id,
            "contract_drift": drift,
            "diagnostics": outcome.diagnostics().iter().map(|d| d.to_string()).collect::<Vec<_>>(),
            "provenance": canopy_planner::draft::provenance(&feature.scenario_ids(), &facts.root_package),
        });
        std::fs::write(
            run_dir.join("plan.meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(run_dir.join("plan.md"), render_markdown(plan, &feature)).map_err(|e| e.to_string())?;
        // What each step was routed: its layers (from its classes), and the contract rules it was given.
        let routing: Vec<serde_json::Value> = plan
            .steps
            .iter()
            .map(|step| {
                let classes: Vec<String> = step
                    .scope
                    .create
                    .iter()
                    .chain(&step.scope.modify)
                    .filter_map(|p| canopy_implementer::java::qualified_name_of_source(p))
                    .collect();
                let layers = facts.layers_of_classes(&checkout, &classes);
                let rules = facts
                    .contract
                    .as_ref()
                    .map(|c| c.rule_ids_for(canopy_planner::contract::Consumer::Implementer, &layers))
                    .unwrap_or_default();
                json!({
                    "step": step.id,
                    "layers": layers,
                    "rules": rules,
                    "context_chars": step.context.as_deref().map_or(0, str::len),
                })
            })
            .collect();
        std::fs::write(
            run_dir.join("routing.json"),
            serde_json::to_string_pretty(&routing).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        if let Some(golden) = &golden {
            let diff = compare_created_files(plan, golden);
            files_match_golden = Some(diff.matches());
            diff_json = json!({"matches": diff.matches(), "missing": diff.missing, "extra": diff.extra});
        }
    }
    let rules: Vec<&'static str> = outcome
        .attempts
        .iter()
        .flat_map(|a| &a.problems)
        .filter_map(|p| match p {
            Problem::Rule(issue) => Some(rule_name(issue.rule)),
            _ => None,
        })
        .collect();
    let first_draft_valid = outcome.attempts.first().is_some_and(|a| a.problems.is_empty());
    let snapshot = metrics.snapshot();
    let steps = outcome.plan.as_ref().map_or(0, |p| p.steps.len());
    let diagnostics: Vec<String> = outcome.diagnostics().iter().map(|d| d.to_string()).collect();
    let summary = json!({
        "diagnostics": diagnostics,
        "run": n,
        "valid": outcome.is_valid(),
        "attempts": outcome.attempts.len(),
        "first_draft_valid": first_draft_valid,
        "steps": steps,
        "attempt_problems": outcome.attempts.iter().map(|a| a.problems.iter().map(problem_json).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "created_files_vs_golden": diff_json,
        "metrics": snapshot,
    });
    std::fs::write(
        run_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "  valid: {}; attempts: {}; first draft valid: {first_draft_valid}; steps: {steps}; created files match the hand-written plan: {}",
        outcome.is_valid(),
        outcome.attempts.len(),
        files_match_golden.map_or("n/a".to_string(), |m| m.to_string())
    );
    for diagnostic in outcome.diagnostics() {
        println!("    diagnostic: {diagnostic}");
    }
    for problem in outcome.attempts.iter().flat_map(|a| &a.problems) {
        println!("    problem: {problem}");
    }
    Ok(RunResult {
        n,
        valid: outcome.is_valid(),
        attempts: outcome.attempts.len(),
        first_draft_valid,
        rules,
        steps,
        files_match_golden,
        metrics: snapshot,
    })
}

/// The Implementer's sampling, with room for a whole plan in one answer.
fn sampling() -> Sampling {
    Sampling {
        max_tokens: 4096,
        ..Sampling::qwen3()
    }
}

fn main() {
    if let Err(message) = real_main() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let config = parse_args()?;
    let task = task(&config.task)?;
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = crate_dir.parent().unwrap().join("dogfooding").join(task.repo);
    if !repo.join(".git").exists() {
        return Err(format!("{} is not a git repository", repo.display()));
    }
    let tag = config.tag.clone().unwrap_or_else(|| task.tag.to_string());
    let tag_commit = git(&repo, &["rev-parse", "--short", &format!("{tag}^{{commit}}")])?;

    let batch_id = now_iso()[..19].replace(':', "-");
    let batch_dir = config.out.join(&batch_id);
    std::fs::create_dir_all(&batch_dir).map_err(|e| e.to_string())?;
    let canopy_root = crate_dir.parent().unwrap();
    let harness_commit = git(canopy_root, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let harness_dirty = !git(
        canopy_root,
        &["status", "--porcelain", "--", "canopy-planner", "canopy-implementer"],
    )
    .unwrap_or_default()
    .trim()
    .is_empty();
    let props: serde_json::Value = ureq::get(&format!("{}/props", config.url))
        .timeout(Duration::from_secs(5))
        .call()
        .ok()
        .and_then(|r| r.into_json().ok())
        .unwrap_or(json!(null));
    let batch = json!({
        "batch": batch_id,
        "started_at": now_iso(),
        "harness_commit": harness_commit.trim(),
        "harness_dirty": harness_dirty,
        "task": task.name,
        "repository_tag": tag,
        "repository_commit": tag_commit.trim(),
        "model_requested": config.model,
        "url": config.url,
        "sampling": sampling(),
        "runs": config.runs,
        "server": {
            "build_info": props["build_info"],
            "model_path": props["model_path"],
            "n_ctx": props["default_generation_settings"]["n_ctx"],
            "total_slots": props["total_slots"],
        },
    });
    std::fs::write(
        batch_dir.join("batch.json"),
        serde_json::to_string_pretty(&batch).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "batch {batch_id}: task {}, {} run(s) from {tag} ({}), model {}",
        task.name,
        config.runs,
        tag_commit.trim(),
        config.model
    );

    let mut results = Vec::new();
    for n in 1..=config.runs {
        results.push(run_one(&config, task, &repo, &tag, &batch_dir, &batch_id, n)?);
    }

    let mut csv = String::from("run,valid,attempts,first_draft_valid,steps,files_match_golden,rules,llm_calls,prompt_tokens,completion_tokens,llm_s\n");
    for r in &results {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{:.1}\n",
            r.n,
            r.valid,
            r.attempts,
            r.first_draft_valid,
            r.steps,
            r.files_match_golden.map_or("n/a".to_string(), |m| m.to_string()),
            r.rules.join("+"),
            r.metrics.llm_calls,
            r.metrics.prompt_tokens,
            r.metrics.completion_tokens,
            r.metrics.llm_ms as f64 / 1000.0
        ));
    }
    std::fs::write(batch_dir.join("results.csv"), &csv).map_err(|e| e.to_string())?;
    println!("\n{csv}");
    let valid = results.iter().filter(|r| r.valid).count();
    let first = results.iter().filter(|r| r.first_draft_valid).count();
    println!(
        "{valid}/{} plans valid ({first} on the first draft). Artifacts: {}",
        results.len(),
        batch_dir.display()
    );
    Ok(())
}
