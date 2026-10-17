//! Replays stored retry prompts in variants against the model, to see how what a retry shows about the failure
//! changes the answer. Measurement only: nothing here changes the Implementer.
//!
//! Variants (all keep the system rules and the rest of the user message as recorded):
//!   current  the retry prompt exactly as it was sent: the previous answer, then the error
//!   fresh    the previous answer left out; the model is told only what failed
//!   located  the previous answer kept, plus the source lines the error points at (`## Where it failed`)
//!
//! Each answer is classified (no submission, identical to the failed answer, changed) and, when changed, built:
//! the repository at the run's tag plus the files of the steps that had already passed plus the new answer, then
//! `mvn test-compile` and `mvn test`. The import audit and the other submit refusals are NOT applied, so this is
//! the model's raw answer to the retry prompt. glob and read_file are carried out against the rebuilt repository
//! (the Implementer's limits: 16 turns, 12 tool calls); the two system rules about reading are left out, as a retry that shows files has them.
//!
//! Usage: retry_replay --runs <dir> [--runs <dir> ...] [--cases N] [--samples K] [--stage test|compile|both]
//!        [--variants current,fresh,located] [--seed S] [--url URL] [--model ID] --out <dir> [--no-build] [--show]
//! `<dir>` holds batches (`<batch>/run-NN/events.jsonl` and `<batch>/batch.json`).
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use canopy_implementer::build::{Build, CommandBuild};
use canopy_implementer::llm::{ChatMessage, LlmClient, ModelEndpoint, Sampling, ToolRequest};
use canopy_implementer::observe::NoopObserver;
use canopy_implementer::replay::{same_files, with_failure_locations, without_previous_answer, Files};
use canopy_implementer::tools::ToolBox;
use serde_json::{json, Value};

struct Case {
    id: String,
    stage: String,
    feedback: String,
    system: String,
    user: String,
    previous: Files,
    prior: Files,
    repo: String,
    tag: String,
}

struct Config {
    runs: Vec<PathBuf>,
    cases: usize,
    samples: usize,
    stage: String,
    variants: Vec<String>,
    seed: u64,
    url: String,
    model: String,
    out: PathBuf,
    build: bool,
    show: bool,
}

fn parse_args() -> Result<Config, String> {
    let mut c = Config {
        runs: vec![],
        cases: 12,
        samples: 2,
        stage: "both".into(),
        variants: vec!["current".into(), "fresh".into(), "located".into()],
        seed: 1,
        url: "http://localhost:8080".into(),
        model: "Qwen/Qwen3-14B-GGUF:Q4_K_M".into(),
        out: PathBuf::new(),
        build: true,
        show: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = |n: &str| args.next().ok_or_else(|| format!("{n} needs a value"));
        match flag.as_str() {
            "--runs" => c.runs.push(value("--runs")?.into()),
            "--cases" => c.cases = value("--cases")?.parse().map_err(|e| format!("--cases: {e}"))?,
            "--samples" => c.samples = value("--samples")?.parse().map_err(|e| format!("--samples: {e}"))?,
            "--stage" => c.stage = value("--stage")?,
            "--variants" => c.variants = value("--variants")?.split(',').map(str::to_string).collect(),
            "--seed" => c.seed = value("--seed")?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--url" => c.url = value("--url")?,
            "--model" => c.model = value("--model")?,
            "--out" => c.out = value("--out")?.into(),
            "--no-build" => c.build = false,
            "--show" => c.show = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    if c.runs.is_empty() || c.out.as_os_str().is_empty() {
        return Err("usage: retry_replay --runs <dir> [--runs <dir>] --out <dir> [--cases N] [--samples K] [--stage test|compile|both] [--variants current,fresh,located] [--seed S] [--url URL] [--model ID] [--no-build] [--show]".into());
    }
    Ok(c)
}

fn files_of(arguments: &str) -> Option<Files> {
    let v: Value = serde_json::from_str(arguments).ok()?;
    let list = v.get("files")?.as_array()?;
    let files: Files = list
        .iter()
        .filter_map(|f| Some((f.get("path")?.as_str()?.to_string(), f.get("content")?.as_str()?.to_string())))
        .collect();
    (!files.is_empty()).then_some(files)
}

fn load_cases(dirs: &[PathBuf]) -> Vec<Case> {
    let mut cases = Vec::new();
    for dir in dirs {
        let mut batches: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten().collect();
        batches.sort_by_key(|e| e.file_name());
        for batch in batches {
            let meta: Value = std::fs::read_to_string(batch.path().join("batch.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or(Value::Null);
            let (Some(repo), Some(tag)) = (meta["repo"].as_str(), meta["repository_tag"].as_str()) else { continue };
            let mut runs: Vec<_> = std::fs::read_dir(batch.path()).into_iter().flatten().flatten().collect();
            runs.sort_by_key(|e| e.file_name());
            for run in runs {
                let Ok(text) = std::fs::read_to_string(run.path().join("events.jsonl")) else { continue };
                let label = format!("{}/{}", batch.file_name().to_string_lossy(), run.file_name().to_string_lossy());
                let (mut step, mut attempt) = (String::new(), 0u64);
                let mut submits: HashMap<(String, u64), Files> = HashMap::new();
                let mut failures: HashMap<(String, u64), (String, String)> = HashMap::new();
                let mut passed: Vec<Files> = Vec::new();
                for line in text.lines() {
                    let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
                    let ev = &v["event"];
                    match ev["type"].as_str() {
                        Some("step_started") => step = ev["step_id"].as_str().unwrap_or("").into(),
                        Some("attempt_started") => attempt = ev["attempt"].as_u64().unwrap_or(0),
                        Some("tool_called") if ev["tool"] == "submit_files" && ev["outcome"] == "ok" => {
                            if let Some(f) = files_of(ev["arguments"].as_str().unwrap_or("")) {
                                submits.insert((step.clone(), attempt), f);
                            }
                        }
                        Some("attempt_failed") => {
                            failures.insert(
                                (step.clone(), ev["attempt"].as_u64().unwrap_or(0)),
                                (ev["stage"].as_str().unwrap_or("").into(), ev["feedback"].as_str().unwrap_or("").into()),
                            );
                        }
                        Some("step_finished") if ev["passed"] == true => {
                            if let Some(f) = submits.iter().filter(|((s, _), _)| *s == step).max_by_key(|((_, a), _)| *a) {
                                passed.push(f.1.clone());
                            }
                        }
                        Some("llm_request_sent") if ev["purpose"].as_str().is_some_and(|p| p.contains("retry")) => {
                            let messages = ev["messages"].as_array().cloned().unwrap_or_default();
                            if messages.len() != 2 || attempt < 2 {
                                continue;
                            }
                            let Some((stage, feedback)) = failures.get(&(step.clone(), attempt - 1)).cloned() else { continue };
                            if stage != "compile" && stage != "test" {
                                continue;
                            }
                            let Some(previous) = submits.get(&(step.clone(), attempt - 1)).cloned() else { continue };
                            let user = messages[1]["content"].as_str().unwrap_or("").to_string();
                            if without_previous_answer(&user).is_none() || !user.contains(&feedback) {
                                continue;
                            }
                            cases.push(Case {
                                id: format!("{label}/{step}#{attempt}"),
                                stage,
                                feedback,
                                system: messages[0]["content"].as_str().unwrap_or("").to_string(),
                                user,
                                previous,
                                prior: passed.iter().flatten().cloned().collect(),
                                repo: repo.to_string(),
                                tag: tag.to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    cases
}

/// A small deterministic shuffle, so a seed picks the same cases every time.
fn shuffle<T>(items: &mut Vec<T>, seed: u64) {
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    for i in (1..items.len()).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        items.swap(i, (state >> 33) as usize % (i + 1));
    }
}

/// The Implementer's own limits: 16 model turns and 12 tool calls in an attempt; asking about a file the step creates is free.
const MAX_TURNS: usize = 16;
const MAX_TOOL_CALLS: usize = 12;

/// The paths listed under `## Files to create` in a user prompt.
fn files_to_create(user: &str) -> Vec<String> {
    let Some(at) = user.find("\n## Files to create\n") else { return vec![] };
    user[at + 21..].lines().take_while(|l| l.starts_with("- ")).map(|l| l[2..].trim().to_string()).collect()
}

/// The system rules as a retry that shows files has them: without the two rules about finding and reading files.
fn system_for_replay(system: &str) -> String {
    system
        .lines()
        .filter(|l| !l.starts_with("ALWAYS use glob and read_file") && !l.starts_with("ALWAYS read an existing file with read_file"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn variant_user(variant: &str, case: &Case) -> Option<String> {
    match variant {
        "current" => Some(case.user.clone()),
        "fresh" => without_previous_answer(&case.user),
        "located" => with_failure_locations(&case.user, &case.feedback, &case.previous),
        _ => None,
    }
}

fn workspace(case: &Case, answer: &Files) -> Result<tempfile::TempDir, String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("dogfooding").join(&case.repo);
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let archive = Command::new("git").arg("-C").arg(&repo).args(["archive", &case.tag]).output().map_err(|e| e.to_string())?;
    if !archive.status.success() {
        return Err(format!("git archive {} failed", case.tag));
    }
    let mut tar = Command::new("tar")
        .args(["-x", "-C"])
        .arg(dir.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    std::io::Write::write_all(tar.stdin.as_mut().unwrap(), &archive.stdout).map_err(|e| e.to_string())?;
    tar.wait().map_err(|e| e.to_string())?;
    for (path, content) in case.prior.iter().chain(answer.iter()) {
        let target = dir.path().join(path);
        std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(target, content).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

fn main() {
    let config = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let mut cases = load_cases(&config.runs);
    println!("{} retry prompts after a compile or test failure found", cases.len());
    cases.retain(|c| config.stage == "both" || c.stage == config.stage);
    shuffle(&mut cases, config.seed);
    let mut chosen: Vec<Case> = Vec::new();
    for stage in ["test", "compile"] {
        let want = if config.stage == "both" { config.cases / 2 + config.cases % 2 * (stage == "test") as usize } else { config.cases };
        chosen.extend(cases.iter().filter(|c| c.stage == stage).take(want).map(|c| Case { id: c.id.clone(), stage: c.stage.clone(), feedback: c.feedback.clone(), system: c.system.clone(), user: c.user.clone(), previous: c.previous.clone(), prior: c.prior.clone(), repo: c.repo.clone(), tag: c.tag.clone() }));
    }
    println!(
        "replaying {} cases ({} test, {} compile) x {} variants x {} samples",
        chosen.len(),
        chosen.iter().filter(|c| c.stage == "test").count(),
        chosen.iter().filter(|c| c.stage == "compile").count(),
        config.variants.len(),
        config.samples
    );
    std::fs::create_dir_all(&config.out).expect("create --out");
    let client = LlmClient::new(
        ModelEndpoint { base_url: config.url.clone(), model: config.model.clone(), timeout: Duration::from_secs(300) },
        Sampling::qwen3(),
        Arc::new(NoopObserver),
    );
    let build = CommandBuild::maven(Duration::from_secs(240), Arc::new(NoopObserver));
    let mut results = std::fs::File::create(config.out.join("results.jsonl")).expect("results file");
    let mut first = true;
    for (index, case) in chosen.iter().enumerate() {
        for variant in &config.variants {
            let Some(user) = variant_user(variant, case) else { continue };
            for sample in 0..config.samples {
                let system = system_for_replay(&case.system);
                let mut transcript = format!("# {} [{}] variant {variant}, sample {sample}\n\n## SYSTEM\n```\n{system}\n```\n\n## USER\n```\n{user}\n```\n\n", case.id, case.stage);
                let ws = match workspace(case, &vec![]) {
                    Ok(ws) => ws,
                    Err(e) => {
                        println!("  case {index:02} {variant} #{sample}: workspace error: {e}");
                        continue;
                    }
                };
                let toolbox = ToolBox::new(ws.path());
                let mut messages = vec![ChatMessage::System(system.clone()), ChatMessage::User(user.clone())];
                let mut answer: Option<Files> = None;
                let mut outcome = String::from("no_submit (turn budget)");
                let mut tool_uses: Vec<String> = Vec::new();
                let create = files_to_create(&user);
                let mut budget_spent = false;
                for turn_no in 1..=MAX_TURNS {
                    let request = ToolRequest {
                        purpose: format!("replay-{variant}"),
                        messages: messages.clone(),
                        tools: ToolBox::specs(),
                        thinking: false,
                    };
                    let turn = match client.complete_with_tools(&request) {
                        Err(e) => {
                            outcome = format!("error: {e}");
                            break;
                        }
                        Ok(t) => t,
                    };
                    transcript.push_str(&format!("## MODEL turn {turn_no} ({} prompt + {} completion tokens, finish {})\n{}\n", turn.prompt_tokens, turn.completion_tokens, turn.finish_reason, turn.content));
                    if turn.finish_reason == "length" {
                        outcome = "no_submit (truncated)".into();
                        break;
                    }
                    if turn.tool_calls.is_empty() {
                        outcome = "no_submit (prose)".into();
                        break;
                    }
                    messages.push(ChatMessage::Assistant { content: turn.content.clone(), tool_calls: turn.tool_calls.clone() });
                    let mut submitted = false;
                    for call in &turn.tool_calls {
                        if call.name == "submit_files" {
                            transcript.push_str(&format!("```json\n{}\n```\n", call.arguments));
                            match files_of(&call.arguments) {
                                Some(files) => {
                                    answer = Some(files);
                                    outcome = "submitted".into();
                                }
                                None => outcome = "unparseable".into(),
                            }
                            submitted = true;
                            break;
                        }
                        let target = serde_json::from_str::<Value>(&call.arguments).ok().and_then(|a| {
                            a.get(if call.name == "glob" { "pattern" } else { "path" }).and_then(Value::as_str).map(str::to_string)
                        });
                        let probe = target.filter(|t| create.contains(t));
                        let text = match &probe {
                            Some(path) => format!("`{path}` is a file this step creates. It does not exist yet; write it in submit_files."),
                            None => {
                                tool_uses.push(call.name.clone());
                                if tool_uses.len() > MAX_TOOL_CALLS {
                                    budget_spent = true;
                                    break;
                                }
                                toolbox.call(&call.name, &call.arguments).text
                            }
                        };
                        transcript.push_str(&format!("tool {} {} -> {}\n", call.name, call.arguments, text.lines().next().unwrap_or("")));
                        messages.push(ChatMessage::Tool { call_id: call.id.clone(), content: text });
                    }
                    if submitted {
                        break;
                    }
                    if budget_spent {
                        outcome = "no_submit (tool budget)".into();
                        break;
                    }
                }
                if !tool_uses.is_empty() {
                    transcript.push_str(&format!("\n(tool calls before the answer: {})\n", tool_uses.join(", ")));
                }
                let (class, built) = match &answer {
                    None => (outcome.clone(), "n/a".to_string()),
                    Some(files) if same_files(files, &case.previous) => ("identical".into(), "known to fail (identical)".into()),
                    Some(files) if config.build => {
                        for (path, content) in files {
                            let target = ws.path().join(path);
                            std::fs::create_dir_all(target.parent().unwrap()).ok();
                            std::fs::write(target, content).ok();
                        }
                        let built = if !build.compile(ws.path()).success {
                            "compile_fail".to_string()
                        } else if build.test(ws.path()).success {
                            "pass".to_string()
                        } else {
                            "test_fail".to_string()
                        };
                        ("changed".into(), built)
                    }
                    Some(_) => ("changed".into(), "not built".into()),
                };
                transcript.push_str(&format!("\n## RESULT\nclass: {class}\nbuild: {built}\n"));
                let name = format!("{index:02}-{variant}-{sample}.md");
                std::fs::write(config.out.join(&name), &transcript).ok();
                writeln!(
                    results,
                    "{}",
                    json!({"case": case.id, "stage": case.stage, "variant": variant, "sample": sample, "class": class, "build": built, "file": name})
                )
                .ok();
                println!("  case {index:02} {} {variant} #{sample}: {class}, build {built}", case.stage);
                if config.show && first {
                    println!("{transcript}");
                }
            }
        }
        first = false;
    }
    summarize(&config.out.join("results.jsonl"), &config.variants);
}

use std::io::Write;

fn summarize(path: &Path, variants: &[String]) {
    let rows: Vec<Value> = std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    println!("\nvariant   stage    n  no_answer  identical  changed  passed");
    for stage in ["all", "test", "compile"] {
        for variant in variants {
            let rs: Vec<&Value> = rows.iter().filter(|r| r["variant"] == *variant && (stage == "all" || r["stage"] == stage)).collect();
            let n = rs.len();
            let identical = rs.iter().filter(|r| r["class"] == "identical").count();
            let changed = rs.iter().filter(|r| r["class"] == "changed").count();
            let passed = rs.iter().filter(|r| r["build"] == "pass").count();
            println!("{variant:<9} {stage:<8} {n:>2}  {:>9}  {identical:>9}  {changed:>7}  {passed:>6}", n - identical - changed);
        }
    }
}
