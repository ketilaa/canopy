//! Runs the Greeter step against the dogfooding project, one or more times, and records everything.
//!
//!   cargo run -p canopy-implementer --example run_step -- [--runs 1] [--attempts 3] [--tag baseline-00-scaffold]
//!
//! Options: --repo <path>  --tag <git tag>  --runs <n>  --attempts <n>  --url <base url>
//!          --model <id>   --out <dir>
//!
//! Each run starts from a clean checkout of the tag, writes a full event trace, and is scored by a
//! hidden acceptance test the model never sees. Output goes to `<out>/<batch>/run-NN/`:
//! `events.jsonl` (every event), `summary.json`, `changes.patch`. The batch directory also holds
//! `batch.json` (what was run, under which settings) and `results.csv`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use canopy_implementer::build::{Build, CommandBuild};
use canopy_implementer::implementer::{Implementer, StepOutcome, StepRequest};
use canopy_implementer::llm::{LlmClient, ModelEndpoint, Sampling};
use canopy_implementer::observe::{now_iso, Metrics, MetricsObserver, Observers, RunEvent, RunObserver, TraceWriter};
use serde_json::json;

/// A step to run, the state to start it from, and how its result is scored.
struct Task {
    name: &'static str,
    default_tag: &'static str,
    instruction: &'static str,
    context: &'static str,
    /// Existing files the step reads or changes; the model is shown their current content.
    files: &'static [&'static str],
    /// Copied in after the run, never shown to the model. Scores what the model's own tests cannot.
    hidden_test: &'static str,
    /// Test methods present in the starting state that must still exist afterwards.
    preserved_tests: &'static [&'static str],
}

const HIDDEN_TEST_PATH: &str = "src/test/java/com/example/demo/GreeterHiddenTest.java";

const CREATE: Task = Task {
    name: "create",
    default_tag: "baseline-00-scaffold",
    instruction: "Create a `Greeter` class in package `com.example.demo` with a method \
`String greet(String name)` that returns `Hello, <name>!`, and throws `IllegalArgumentException` if the name is \
blank. Create a `GreeterTest` class that tests both behaviours.",
    context: "Root package: com.example.demo\n\
Main sources: src/main/java/com/example/demo/\n\
Test sources: src/test/java/com/example/demo/\n\
Build: Maven, Java 21, JUnit 5 (spring-boot-starter-test is on the classpath)\n\
Existing files: src/main/java/com/example/demo/DemoApplication.java",
    files: &[],
    hidden_test: r#"package com.example.demo;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.Test;

class GreeterHiddenTest {
    @Test
    void greetsByName() {
        assertEquals("Hello, Ada!", new Greeter().greet("Ada"));
    }

    @Test
    void greetsAnotherName() {
        assertEquals("Hello, Grace!", new Greeter().greet("Grace"));
    }

    @Test
    void rejectsAnEmptyName() {
        assertThrows(IllegalArgumentException.class, () -> new Greeter().greet(""));
    }

    @Test
    void rejectsABlankName() {
        assertThrows(IllegalArgumentException.class, () -> new Greeter().greet("   "));
    }
}
"#,
    preserved_tests: &[],
};

const TRIM: Task = Task {
    name: "trim",
    default_tag: "candidate-01-greeter",
    instruction: "Change `Greeter.greet` so that leading and trailing whitespace is removed from the name before it \
is used: `greet(\"  Ada \")` must return `Hello, Ada!`. A blank name must still throw `IllegalArgumentException`. \
Keep all existing behaviour and existing tests. Add a test for the new behaviour to `GreeterTest`.",
    context: "Root package: com.example.demo\n\
Main sources: src/main/java/com/example/demo/\n\
Test sources: src/test/java/com/example/demo/\n\
Build: Maven, Java 21, JUnit 5 (spring-boot-starter-test is on the classpath)\n\
Existing files: src/main/java/com/example/demo/DemoApplication.java, \
src/main/java/com/example/demo/Greeter.java, src/test/java/com/example/demo/GreeterTest.java",
    files: &[
        "src/main/java/com/example/demo/Greeter.java",
        "src/test/java/com/example/demo/GreeterTest.java",
    ],
    hidden_test: r#"package com.example.demo;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.Test;

class GreeterHiddenTest {
    @Test
    void greetsByName() {
        assertEquals("Hello, Ada!", new Greeter().greet("Ada"));
    }

    @Test
    void trimsSurroundingWhitespace() {
        assertEquals("Hello, Ada!", new Greeter().greet("  Ada "));
    }

    @Test
    void trimsTabsAndNewlinesToo() {
        assertEquals("Hello, Grace!", new Greeter().greet("\tGrace\n"));
    }

    @Test
    void stillRejectsAnEmptyName() {
        assertThrows(IllegalArgumentException.class, () -> new Greeter().greet(""));
    }

    @Test
    void stillRejectsABlankName() {
        assertThrows(IllegalArgumentException.class, () -> new Greeter().greet("   "));
    }
}
"#,
    preserved_tests: &["testGreetWithValidName", "testGreetWithBlankName"],
};

fn task(name: &str) -> Result<Task, String> {
    match name {
        "create" => Ok(CREATE),
        "trim" => Ok(TRIM),
        other => Err(format!("unknown task {other}; known: create, trim")),
    }
}

const USAGE: &str =
    "usage: run_step [--task create|trim] [--runs N] [--attempts N] [--tag TAG] [--repo PATH] [--url URL] [--model ID] [--out DIR]";

struct Config {
    task: String,
    repo: PathBuf,
    tag: String,
    runs: u32,
    attempts: u32,
    url: String,
    model: String,
    out: PathBuf,
}

fn parse_args() -> Result<Config, String> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut config = Config {
        task: "create".into(),
        repo: crate_dir.parent().unwrap().join("dogfooding"),
        tag: String::new(),
        runs: 1,
        attempts: 3,
        url: "http://localhost:8080".into(),
        model: "Qwen/Qwen3-14B-GGUF:Q4_K_M".into(),
        out: crate_dir.join("runs"),
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--help" || flag == "-h" {
            return Err(USAGE.into());
        }
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--task" => config.task = value("--task")?,
            "--repo" => config.repo = value("--repo")?.into(),
            "--tag" => config.tag = value("--tag")?,
            "--runs" => config.runs = value("--runs")?.parse().map_err(|e| format!("--runs: {e}"))?,
            "--attempts" => config.attempts = value("--attempts")?.parse().map_err(|e| format!("--attempts: {e}"))?,
            "--url" => config.url = value("--url")?,
            "--model" => config.model = value("--model")?,
            "--out" => config.out = value("--out")?.into(),
            other => return Err(format!("unknown option {other}")),
        }
    }
    if config.tag.is_empty() {
        config.tag = task(&config.task)?.default_tag.to_string();
    }
    Ok(config)
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn reset(repo: &Path, tag: &str) -> Result<(), String> {
    git(repo, &["checkout", "-q", "-f", "--detach", tag])?;
    // Without -x: the Maven target/ directory survives, so builds stay fast.
    git(repo, &["clean", "-fdq"])?;
    Ok(())
}

/// Prints the events worth watching as they happen.
struct Console;

impl RunObserver for Console {
    fn on(&self, event: &RunEvent) {
        let first_lines = |text: &str| text.lines().take(2).collect::<Vec<_>>().join(" | ");
        match event {
            RunEvent::LlmRequestSent(r) => println!("  -> llm call {} ({})", r.call_id, r.purpose),
            RunEvent::LlmResponseReceived(r) => println!(
                "  <- llm call {}: {} in / {} out tokens, {:.1}s, finish={}",
                r.call_id,
                r.prompt_tokens,
                r.completion_tokens,
                r.elapsed_ms as f64 / 1000.0,
                r.finish_reason
            ),
            RunEvent::LlmCallFailed(f) => println!("  <- llm call {} FAILED {:?}: {}", f.call_id, f.kind, f.message),
            RunEvent::FilesWritten(w) => println!("  wrote {}", w.paths.join(", ")),
            RunEvent::CommandRan(c) => println!(
                "  mvn {:?}: {} in {:.1}s",
                c.kind,
                if c.success { "ok" } else { "FAILED" },
                c.elapsed_ms as f64 / 1000.0
            ),
            RunEvent::AttemptFailed(f) => println!(
                "  attempt {} failed at {:?}: {}",
                f.attempt,
                f.stage,
                first_lines(&f.feedback)
            ),
            RunEvent::WorkspaceRestored(r) => println!("  restored {} file(s)", r.paths.len()),
            RunEvent::StepFinished(f) => {
                println!(
                    "  step {}: {} after {} attempt(s), {:.1}s",
                    f.step_id,
                    if f.passed { "PASSED" } else { "FAILED" },
                    f.attempts,
                    f.elapsed_ms as f64 / 1000.0
                )
            }
            RunEvent::ToolCalled(t) => println!(
                "  tool {} {} -> {:?} ({} chars)",
                t.tool,
                t.arguments.chars().take(80).collect::<String>(),
                t.outcome,
                t.result.len()
            ),
            RunEvent::StepStarted(_) | RunEvent::AttemptStarted(_) => {}
        }
    }
}

fn count_own_tests(repo: &Path) -> usize {
    fn walk(dir: &Path, total: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, total);
            } else if path.extension().is_some_and(|e| e == "java") {
                *total += std::fs::read_to_string(&path)
                    .unwrap_or_default()
                    .matches("@Test")
                    .count();
            }
        }
    }
    let mut total = 0;
    walk(&repo.join("src/test/java"), &mut total);
    total
}

/// Everything under src/test/java, concatenated.
fn test_sources(repo: &Path) -> String {
    fn walk(dir: &Path, out: &mut String) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "java") {
                out.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    let mut out = String::new();
    walk(&repo.join("src/test/java"), &mut out);
    out
}

struct RunResult {
    n: u32,
    passed: bool,
    attempts: u32,
    acceptance: &'static str,
    own_tests: usize,
    tests_added: usize,
    kept_original: bool,
    scored: bool,
    metrics: Metrics,
    step_ms: u64,
}

fn run_one(config: &Config, task: &Task, batch_dir: &Path, batch_id: &str, n: u32) -> Result<RunResult, String> {
    let run_dir = batch_dir.join(format!("run-{n:02}"));
    reset(&config.repo, &config.tag)?;
    let tests_before = count_own_tests(&config.repo);
    println!("\n== run {n}/{} ==", config.runs);

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
            timeout: Duration::from_secs(300),
        },
        Sampling::qwen3(),
        observers.clone(),
    );
    let build = CommandBuild::maven(Duration::from_secs(240), observers.clone());
    let implementer = Implementer::new(Box::new(client), Box::new(build), observers.clone(), config.attempts);

    let step = StepRequest {
        id: format!("greeter-{}", task.name),
        instruction: task.instruction.into(),
        context: task.context.into(),
        files: task.files.iter().map(|f| f.to_string()).collect(),
        kind: Default::default(),
    };
    let started = Instant::now();
    let outcome = implementer.run_step(&config.repo, &step);
    let step_ms = started.elapsed().as_millis() as u64;
    let metrics_after_step = metrics.snapshot();

    let (passed, attempts) = match &outcome {
        StepOutcome::Passed { attempts } => (true, *attempts),
        StepOutcome::Failed { attempts, .. } => (false, *attempts),
    };

    // What the run left behind, before the hidden test is added.
    let own_tests = count_own_tests(&config.repo);
    let tests_added = own_tests.saturating_sub(tests_before);
    let sources = test_sources(&config.repo);
    let kept_original = task
        .preserved_tests
        .iter()
        .all(|name| sources.contains(&format!("{name}(")));
    git(&config.repo, &["add", "-A"])?;
    let patch = git(&config.repo, &["diff", "--cached", "HEAD"])?;
    let files_changed: Vec<String> = git(&config.repo, &["diff", "--cached", "--name-only", "HEAD"])?
        .lines()
        .map(String::from)
        .collect();
    git(&config.repo, &["reset", "-q"])?;
    std::fs::write(run_dir.join("changes.patch"), &patch).map_err(|e| e.to_string())?;

    // Score with the hidden acceptance test, only when the loop claims success.
    let (acceptance, acceptance_digest) = if passed {
        let path = config.repo.join(HIDDEN_TEST_PATH);
        std::fs::write(&path, task.hidden_test).map_err(|e| e.to_string())?;
        let outcome = CommandBuild::maven(Duration::from_secs(240), observers.clone()).test(&config.repo);
        if outcome.success {
            ("passed", String::new())
        } else {
            ("failed", outcome.digest)
        }
    } else {
        ("not_run", String::new())
    };
    let scored = acceptance == "passed" && kept_original && tests_added >= 1;
    println!(
        "  hidden acceptance: {acceptance}; original tests kept: {kept_original}; tests added: {tests_added}; scored: {scored}"
    );

    let summary = json!({
        "run": n,
        "passed": passed,
        "attempts": attempts,
        "last_failure": match &outcome { StepOutcome::Failed { last_failure, .. } => json!(last_failure), _ => json!(null) },
        "step_ms": step_ms,
        "acceptance": acceptance,
        "acceptance_digest": acceptance_digest,
        "own_tests": own_tests,
        "tests_before": tests_before,
        "tests_added": tests_added,
        "original_tests_kept": kept_original,
        "scored_pass": scored,
        "files_changed": files_changed,
        "metrics": metrics_after_step,
    });
    std::fs::write(
        run_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    Ok(RunResult {
        n,
        passed,
        attempts,
        acceptance,
        own_tests,
        tests_added,
        kept_original,
        scored,
        metrics: metrics_after_step,
        step_ms,
    })
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
    if !config.repo.join(".git").exists() {
        return Err(format!("{} is not a git repository", config.repo.display()));
    }
    let tag_commit = git(
        &config.repo,
        &["rev-parse", "--short", &format!("{}^{{commit}}", config.tag)],
    )?;

    let batch_id = now_iso()[..19].replace(':', "-");
    let batch_dir = config.out.join(&batch_id);
    std::fs::create_dir_all(&batch_dir).map_err(|e| e.to_string())?;

    let canopy_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let harness_commit = git(canopy_root, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let harness_dirty = !git(canopy_root, &["status", "--porcelain", "--", "canopy-implementer"])
        .unwrap_or_default()
        .trim()
        .is_empty();
    // Best effort: what the server says it is running with.
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
        "repository_tag": config.tag,
        "repository_commit": tag_commit.trim(),
        "model_requested": config.model,
        "url": config.url,
        "sampling": Sampling::qwen3(),
        "max_attempts": config.attempts,
        "runs": config.runs,
        "task": task.name,
        "instruction": task.instruction,
        "context": task.context,
        "files_shown": task.files,
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
        "batch {batch_id}: task {}, {} run(s) from {} ({}), model {}",
        task.name,
        config.runs,
        config.tag,
        tag_commit.trim(),
        config.model
    );

    let mut results = Vec::new();
    for n in 1..=config.runs {
        results.push(run_one(&config, &task, &batch_dir, &batch_id, n)?);
    }

    let mut csv = String::from(
        "run,passed,attempts,acceptance,kept_original,tests_added,scored,own_tests,llm_calls,prompt_tokens,completion_tokens,llm_s,build_s,step_s\n",
    );
    println!(
        "\n{:>3} {:>7} {:>8} {:>10} {:>5} {:>6} {:>6} {:>6} {:>9} {:>7} {:>7}",
        "run", "passed", "attempts", "acceptance", "kept", "+tests", "scored", "calls", "tokens", "llm_s", "step_s"
    );
    for r in &results {
        let m = &r.metrics;
        println!(
            "{:>3} {:>7} {:>8} {:>10} {:>5} {:>6} {:>6} {:>6} {:>4}/{:<4} {:>7.1} {:>7.1}",
            r.n,
            r.passed,
            r.attempts,
            r.acceptance,
            r.kept_original,
            r.tests_added,
            r.scored,
            m.llm_calls,
            m.prompt_tokens,
            m.completion_tokens,
            m.llm_ms as f64 / 1000.0,
            r.step_ms as f64 / 1000.0
        );
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{:.1},{:.1},{:.1}\n",
            r.n,
            r.passed,
            r.attempts,
            r.acceptance,
            r.kept_original,
            r.tests_added,
            r.scored,
            r.own_tests,
            m.llm_calls,
            m.prompt_tokens,
            m.completion_tokens,
            m.llm_ms as f64 / 1000.0,
            m.build_ms as f64 / 1000.0,
            r.step_ms as f64 / 1000.0
        ));
    }
    std::fs::write(batch_dir.join("results.csv"), csv).map_err(|e| e.to_string())?;
    let scored = results.iter().filter(|r| r.scored).count();
    println!(
        "\n{scored}/{} runs scored: hidden acceptance passed, original tests kept, a test added. Trace and summaries: {}",
        results.len(),
        batch_dir.display()
    );
    Ok(())
}
