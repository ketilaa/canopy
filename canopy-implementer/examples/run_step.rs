//! Runs the Greeter step against the dogfooding project, one or more times, and records everything.
//!
//!   cargo run -p canopy-implementer --example run_step -- [--runs 1] [--attempts 3] [--tag baseline-00-scaffold]
//!
//! Options: --repo <path>  --tag <git tag>  --runs <n>  --attempts <n>  --url <base url>
//!          --model <id>   --out <dir>   --declare-imports (the model lists its imports before the files)
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
use canopy_implementer::implementer::{Implementer, StepOutcome, StepRequest, ToolLimits};
use canopy_implementer::llm::{LlmClient, ModelEndpoint, Sampling};
use canopy_implementer::observe::{
    now_iso, Metrics, MetricsObserver, Observers, RecordingObserver, RunEvent, RunObserver, ToolOutcome, TraceWriter,
};
use canopy_implementer::orchestrator::Orchestrator;
use canopy_implementer::plan::Plan;
use canopy_implementer::report::{content_id, RunReport};
use canopy_implementer::structure::{self, Check};
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
    /// Whether the model finds and reads files itself with tools, instead of being shown them.
    tools: bool,
    /// Which repository under `dogfooding/` the task runs in.
    repo: &'static str,
    /// Where the hidden test is copied to, relative to the repository root.
    hidden_test_path: &'static str,
    /// Tests the run must add (updating an existing test does not count as adding one).
    min_tests_added: usize,
    /// Files that must be byte-identical afterwards: a trap the model should stay out of.
    untouched: &'static [&'static str],
}

const GREETER_HIDDEN_PATH: &str = "src/test/java/com/example/demo/GreeterHiddenTest.java";

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
    tools: false,
    repo: "java-greeter",
    hidden_test_path: GREETER_HIDDEN_PATH,
    min_tests_added: 1,
    untouched: &[],
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
    tools: false,
    repo: "java-greeter",
    hidden_test_path: GREETER_HIDDEN_PATH,
    min_tests_added: 1,
    untouched: &[],
};

/// The same change as `trim`, said the way a person would, with no file list and no hints: the model
/// has to find the files and read them itself.
const NAV: Task = Task {
    name: "nav",
    default_tag: "candidate-01-greeter",
    instruction: "The Greeter should trim the name before returning the greeting in the greet method.",
    context: "Build: Maven, Java 21, JUnit 5 (spring-boot-starter-test is on the classpath)",
    files: &[],
    hidden_test: TRIM.hidden_test,
    preserved_tests: TRIM.preserved_tests,
    tools: true,
    repo: "java-greeter",
    hidden_test_path: GREETER_HIDDEN_PATH,
    min_tests_added: 1,
    untouched: &[],
};

/// A request that pastes the line to change: the model may be tempted to skip reading the file.
const PASTED: Task = Task {
    name: "pasted",
    default_tag: "candidate-01-greeter",
    instruction: "In `Greeter.greet`, change `return \"Hello, \" + name + \"!\";` so that the name is trimmed first.",
    context: NAV.context,
    files: &[],
    hidden_test: TRIM.hidden_test,
    preserved_tests: TRIM.preserved_tests,
    tools: true,
    repo: "java-greeter",
    hidden_test_path: GREETER_HIDDEN_PATH,
    min_tests_added: 1,
    untouched: &[],
};

/// A change that contradicts an existing test: the old assertion must be updated, not deleted.
const HI: Task = Task {
    name: "hi",
    default_tag: "candidate-01-greeter",
    instruction: "Greet with `Hi` instead of `Hello`.",
    context: NAV.context,
    files: &[],
    hidden_test: include_str!("../tasks/hi/GreeterHiddenTest.java"),
    preserved_tests: TRIM.preserved_tests,
    tools: true,
    repo: "java-greeter",
    hidden_test_path: GREETER_HIDDEN_PATH,
    min_tests_added: 0,
    untouched: &[],
};

/// The behaviour lives in a helper, and a do-not-change look-alike with its own test sits next to it.
const NAMING: Task = Task {
    name: "naming",
    default_tag: "baseline-00-naming",
    instruction: "Names should be trimmed before the greeting is built.",
    context: NAV.context,
    files: &[],
    hidden_test: include_str!("../tasks/naming/GreeterHiddenTest.java"),
    preserved_tests: &[
        "greetsGuestWhenTheNameIsNull",
        "greetsGuestWhenTheNameIsBlank",
        "capitalizesTheFirstLetter",
        "usesGuestForANullName",
        "usesGuestForABlankName",
        "keepsPaddingAsIsBecauseDownstreamSystemsDependOnIt",
    ],
    tools: true,
    repo: "java-naming",
    hidden_test_path: "src/test/java/com/example/greeting/GreeterHiddenTest.java",
    min_tests_added: 1,
    untouched: &[
        "src/main/java/com/example/legacy/LegacyGreeter.java",
        "src/test/java/com/example/legacy/LegacyGreeterTest.java",
    ],
};

/// A change to what an order costs ripples through three classes and contradicts existing tests.
const VAT: Task = Task {
    name: "vat",
    default_tag: "baseline-00-shop",
    instruction: "Orders should include VAT at 25 percent: show it as its own line on the invoice, and make sure the \
total includes it.",
    context: NAV.context,
    files: &[],
    hidden_test: include_str!("../tasks/vat/VatHiddenTest.java"),
    preserved_tests: &[
        "subtotalSumsTheLines",
        "thereIsNoDiscountBelowOneHundred",
        "tenPercentDiscountFromOneHundred",
        "totalIsTheSubtotalMinusTheDiscount",
        "formatsLinesSubtotalAndTotal",
        "showsTheDiscountForLargeOrders",
        "chargesTheTotal",
        "chargesTheDiscountedTotal",
    ],
    tools: true,
    repo: "java-shop",
    hidden_test_path: "src/test/java/com/example/shop/VatHiddenTest.java",
    min_tests_added: 1,
    untouched: &[],
};

/// The same story and scoring as `vat`, but a hand-written plan of small steps is run one after another.
const VAT_PLANNED: Task = Task {
    name: "vat-planned",
    ..VAT
};

/// The same story and scoring as `naming`, run as a hand-written plan.
const NAMING_PLANNED: Task = Task {
    name: "naming-planned",
    ..NAMING
};

/// A full vertical slice built as a plan: domain object, in-memory repository, service, REST controller.
const WIDGETS_PLANNED: Task = Task {
    name: "widgets-planned",
    default_tag: "baseline-00-widgets",
    instruction: "A client can create a widget with POST /widgets, giving a name. The widget gets a generated id. A \
name that is blank, missing, or longer than 50 characters is rejected.",
    context: "Root package: com.example.widgets, with the layers domain, repository, service and web.\n\
Build: Maven, Java 21, JUnit 5; Spring Boot 3.4 with spring-boot-starter-web. Do not change pom.xml.",
    files: &[],
    hidden_test: include_str!("../tasks/widgets/WidgetsHiddenTest.java"),
    preserved_tests: &[],
    tools: true,
    repo: "java-widgets",
    hidden_test_path: "src/test/java/com/example/widgets/WidgetsHiddenTest.java",
    min_tests_added: 8,
    untouched: &["pom.xml"],
};

/// The success path of the widgets story as a story of its own; the invalid names are the next story.
const WIDGETS_CREATE: Task = Task {
    name: "widgets-create",
    default_tag: "baseline-06-widgets-contract",
    instruction: "A client can create a widget with POST /widgets, giving a name. The widget gets a generated id, \
which is a UUID.",
    hidden_test: include_str!("../tasks/widgets-create/WidgetsHiddenTest.java"),
    min_tests_added: 4,
    ..WIDGETS_PLANNED
};

/// The invalid-name paths, on a repository where the success path is built (give its tag with --tag). The hidden
/// test is the whole of it, so the success path is checked again.
const WIDGETS_INVALID: Task = Task {
    name: "widgets-invalid",
    default_tag: "baseline-06-widgets-contract",
    instruction: "A widget name that is blank, missing, or longer than 50 characters is rejected with status 400. A \
name of exactly 50 characters is accepted.",
    min_tests_added: 3,
    ..WIDGETS_PLANNED
};

/// The order story on plain Java, run as a plan the Planner made (--plan). Plans choose their own class names, so
/// there is no API-independent acceptance test: acceptance is that the whole suite passes after the last step.
const ORDER_PLANNED: Task = Task {
    name: "order-planned",
    default_tag: "baseline-order-001",
    instruction: "A customer can place an order for a product in a quantity. The order total is the product price \
times the quantity, less 10 percent when that net total is 500.00 or more. A quantity that is zero or negative is \
rejected. A placed order can be looked up by its id.",
    context: "Root package: no.experis.orders.\nBuild: Maven, Java 21, JUnit 5; no framework. Do not change pom.xml.",
    files: &[],
    hidden_test: include_str!("../tasks/order/OrderHiddenTest.java"),
    preserved_tests: &[],
    tools: true,
    repo: "java-orders",
    hidden_test_path: "src/test/java/no/experis/orders/OrderHiddenTest.java",
    min_tests_added: 4,
    untouched: &["pom.xml"],
};

/// What the finished widgets slice must look like, which behaviour alone cannot show: every layer exists,
/// each is connected to the next, and the controller reaches the repository only through the service.
const WIDGETS_STRUCTURE: &[Check] = &[
    Check::Exists("Widget.java"),
    Check::Exists("WidgetRepository.java"),
    Check::Exists("InMemoryWidgetRepository.java"),
    Check::Exists("WidgetService.java"),
    Check::Exists("WidgetController.java"),
    Check::Mentions("InMemoryWidgetRepository.java", "WidgetRepository"),
    Check::Mentions("WidgetService.java", "WidgetRepository"),
    Check::Mentions("WidgetController.java", "WidgetService"),
    Check::Lacks("WidgetController.java", "WidgetRepository"),
    Check::Lacks("WidgetController.java", "InMemoryWidgetRepository"),
    Check::Lacks("Widget.java", "WidgetRepository"),
    Check::Lacks("Widget.java", "WidgetService"),
];

/// What the model's own `OrderTest` covers: the exception message is asserted, and the discount threshold is tested
/// exactly at 500.00 (100.00 x 5 gives 450.00). Reported with each run, not part of `scored`: the production code is
/// judged by the hidden `OrderHiddenTest`, and a plan with less detail is not asked for these.
const ORDER_OWN_TESTS: &[Check] = &[
    Check::TestContains("OrderTest.java", "Quantity must be positive"),
    Check::TestContains("OrderTest.java", "450.00"),
];

fn structure_for(task_name: &str) -> &'static [Check] {
    match task_name {
        "widgets-planned" => WIDGETS_STRUCTURE,
        _ => &[],
    }
}

/// Checks that are reported with a run but do not decide whether it is scored.
fn info_for(task_name: &str) -> &'static [Check] {
    match task_name {
        "order-planned" => ORDER_OWN_TESTS,
        _ => &[],
    }
}

/// The hand-written plan for a task, if it has one. The Implementer sees one step at a time, never the plan.
fn plan_for(task_name: &str) -> Option<&'static str> {
    match task_name {
        "vat-planned" => Some(include_str!("../tasks/vat/plan.yaml")),
        "naming-planned" => Some(include_str!("../tasks/naming/plan.yaml")),
        "widgets-planned" => Some(include_str!("../tasks/widgets/plan.yaml")),
        _ => None,
    }
}

fn task(name: &str) -> Result<Task, String> {
    match name {
        "create" => Ok(CREATE),
        "trim" => Ok(TRIM),
        "nav" => Ok(NAV),
        "pasted" => Ok(PASTED),
        "hi" => Ok(HI),
        "naming" => Ok(NAMING),
        "vat" => Ok(VAT),
        "vat-planned" => Ok(VAT_PLANNED),
        "naming-planned" => Ok(NAMING_PLANNED),
        "widgets-planned" => Ok(WIDGETS_PLANNED),
        "widgets-create" => Ok(WIDGETS_CREATE),
        "widgets-invalid" => Ok(WIDGETS_INVALID),
        "order-planned" => Ok(ORDER_PLANNED),
        other => Err(format!(
            "unknown task {other}; known: create, trim, nav, pasted, hi, naming, vat, naming-planned, vat-planned, widgets-planned, widgets-create, widgets-invalid, order-planned"
        )),
    }
}

const USAGE: &str =
    "usage: run_step [--task create|trim|nav|pasted|hi|naming|vat|naming-planned|vat-planned|widgets-planned|widgets-create|widgets-invalid|order-planned] [--runs N] [--attempts N] [--tag TAG] [--repo PATH] [--url URL] [--model ID] [--out DIR] [--plan FILE] [--no-classpath] [--surfaces] [--prompt current|lean|persona]";

struct Config {
    task: String,
    repo: PathBuf,
    tag: String,
    runs: u32,
    attempts: u32,
    url: String,
    model: String,
    out: PathBuf,
    declare_imports: bool,
    /// Run this plan instead of the task's hand-written one (a planned task only).
    plan: Option<PathBuf>,
    /// Check each submission's imports against the project's classpath (on unless `--no-classpath`).
    classpath: bool,
    /// Show read-only Java files as their declarations, from `roots surface` (off unless `--surfaces`).
    surfaces: bool,
    /// The system rules of a tool conversation (the current ones unless `--prompt`).
    prompt: canopy_implementer::prompt::PromptVariant,
}

fn parse_args() -> Result<Config, String> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut config = Config {
        task: "create".into(),
        repo: PathBuf::new(),
        tag: String::new(),
        runs: 1,
        attempts: 3,
        url: "http://localhost:8080".into(),
        model: "Qwen/Qwen3-14B-GGUF:Q4_K_M".into(),
        out: crate_dir.join("runs"),
        declare_imports: false,
        plan: None,
        classpath: true,
        surfaces: false,
        prompt: Default::default(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--help" || flag == "-h" {
            return Err(USAGE.into());
        }
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match flag.as_str() {
            "--task" => config.task = value("--task")?,
            "--repo" => {
                let given = value("--repo")?;
                config.repo = if given.contains('/') {
                    given.into()
                } else {
                    crate_dir.parent().unwrap().join("dogfooding").join(given)
                }
            }
            "--tag" => config.tag = value("--tag")?,
            "--runs" => config.runs = value("--runs")?.parse().map_err(|e| format!("--runs: {e}"))?,
            "--attempts" => config.attempts = value("--attempts")?.parse().map_err(|e| format!("--attempts: {e}"))?,
            "--declare-imports" => config.declare_imports = true,
            "--plan" => config.plan = Some(value("--plan")?.into()),
            "--no-classpath" => config.classpath = false,
            "--surfaces" => config.surfaces = true,
            "--prompt" => {
                let name = value("--prompt")?;
                config.prompt = canopy_implementer::prompt::PromptVariant::parse(&name)
                    .ok_or_else(|| format!("--prompt: unknown variant {name}; known: current, lean, persona"))?;
            }
            "--url" => config.url = value("--url")?,
            "--model" => config.model = value("--model")?,
            "--out" => config.out = value("--out")?.into(),
            other => return Err(format!("unknown option {other}")),
        }
    }
    let chosen = task(&config.task)?;
    // The widgets stories have no hand-written plan: they are run with a plan the Planner made (--plan).
    let takes_a_plan =
        plan_for(chosen.name).is_some() || chosen.name == "widgets-create" || chosen.name == "widgets-invalid" || chosen.name == "order-planned";
    if config.plan.is_some() && !takes_a_plan {
        return Err(format!("--plan needs a planned task, and {} has no plan", chosen.name));
    }
    if config.tag.is_empty() {
        config.tag = chosen.default_tag.to_string();
    }
    if config.repo.as_os_str().is_empty() {
        config.repo = crate_dir.parent().unwrap().join("dogfooding").join(chosen.repo);
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
            RunEvent::PlanStarted(p) => println!(
                "== plan {}: {} step(s): {}",
                p.plan_id,
                p.step_ids.len(),
                p.step_ids.join(", ")
            ),
            RunEvent::PlanFinished(p) => {
                println!(
                    "== plan {}: {} ({} of {} steps){}",
                    p.plan_id,
                    if p.passed { "PASSED" } else { "STOPPED" },
                    p.steps_passed,
                    p.steps_total,
                    p.failed_step
                        .as_ref()
                        .map(|s| format!(", failed at {s}"))
                        .unwrap_or_default()
                );
                for issue in &p.issues {
                    println!("   plan problem: {issue}");
                }
            }
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

/// What the event stream says about how the model found its way around.
#[derive(Default)]
struct Navigation {
    tool_calls: usize,
    files_read: Vec<String>,
    read_production: bool,
    read_test: bool,
    refused_submissions: usize,
    /// Calls that got the model nothing: refused, erroneous, or a glob that matched no file.
    wasted_calls: usize,
}

fn navigation(events: &[RunEvent]) -> Navigation {
    let mut nav = Navigation::default();
    for event in events {
        let RunEvent::ToolCalled(call) = event else { continue };
        nav.tool_calls += 1;
        if call.outcome != ToolOutcome::Ok || (call.tool == "glob" && call.result.starts_with("No files match")) {
            nav.wasted_calls += 1;
        }
        if call.tool == "submit_files" && call.outcome == ToolOutcome::Refused {
            nav.refused_submissions += 1;
        }
        if call.tool == "read_file" && call.outcome == ToolOutcome::Ok {
            let path = serde_json::from_str::<serde_json::Value>(&call.arguments)
                .ok()
                .and_then(|v| v["path"].as_str().map(|p| p.trim_start_matches("./").to_string()))
                .unwrap_or_default();
            nav.read_production |= path.starts_with("src/main/");
            nav.read_test |= path.starts_with("src/test/");
            if !nav.files_read.contains(&path) {
                nav.files_read.push(path);
            }
        }
    }
    nav
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
    nav: Navigation,
}

fn run_one(config: &Config, task: &Task, batch_dir: &Path, batch_id: &str, n: u32) -> Result<RunResult, String> {
    let run_dir = batch_dir.join(format!("run-{n:02}"));
    reset(&config.repo, &config.tag)?;
    let tests_before = count_own_tests(&config.repo);
    println!("\n== run {n}/{} ==", config.runs);

    let metrics = Arc::new(MetricsObserver::new());
    let recorder = Arc::new(RecordingObserver::new());
    let trace = TraceWriter::create(&run_dir.join("events.jsonl"), &format!("{batch_id}/run-{n:02}"))
        .map_err(|e| e.to_string())?;
    let observers: Arc<Observers> = Arc::new(
        Observers::new()
            .with(Arc::new(trace))
            .with(metrics.clone())
            .with(recorder.clone())
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
    let implementer = if task.tools {
        Implementer::with_tools(
            Box::new(client),
            Box::new(build),
            observers.clone(),
            config.attempts,
            ToolLimits {
                declare_imports: config.declare_imports,
                ..ToolLimits::default()
            },
        )
    } else {
        Implementer::new(Box::new(client), Box::new(build), observers.clone(), config.attempts)
    };

    // The classes the project can load, so a wrong or missing library import is refused before the build.
    let classpath = config.classpath.then(|| {
        let began = Instant::now();
        let index = canopy_implementer::classpath::ClassIndex::for_repository(&config.repo);
        println!(
            "  classpath index: resolved {} in {:.1}s",
            index.resolved(),
            began.elapsed().as_secs_f64()
        );
        Arc::new(index)
    });
    let implementer = match &classpath {
        Some(index) => implementer.with_classpath(index.clone()),
        None => implementer,
    };
    let implementer = implementer.with_prompt(config.prompt);
    let implementer = if config.surfaces {
        implementer.with_surfaces(Arc::new(canopy_implementer::surfaces::RootsSurfaces::new("roots")))
    } else {
        implementer
    };

    let started = Instant::now();
    let plan_text: Option<String> = match &config.plan {
        Some(path) => Some(std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?),
        None => plan_for(task.name).map(str::to_string),
    };
    let (passed, attempts, last_failure, plan_summary) = match plan_text.as_deref() {
        Some(yaml) => {
            let plan = Plan::from_yaml(yaml).map_err(|e| e.to_string())?;
            let outcome =
                Orchestrator::new(Box::new(implementer), observers.clone()).run(&config.repo, &plan, task.context);
            let attempts_of = |o: &StepOutcome| match o {
                StepOutcome::Passed { attempts } | StepOutcome::Failed { attempts, .. } => *attempts,
            };
            let attempts: u32 = outcome.results.iter().map(|r| attempts_of(&r.outcome)).sum();
            let last_failure = outcome.results.last().and_then(|r| match &r.outcome {
                StepOutcome::Failed { last_failure, .. } => Some(last_failure.clone()),
                StepOutcome::Passed { .. } => None,
            });
            let summary = json!({
                "id": plan.id,
                "steps": outcome.results.iter().map(|r| json!({
                    "id": r.id,
                    "passed": matches!(r.outcome, StepOutcome::Passed { .. }),
                    "attempts": attempts_of(&r.outcome),
                })).collect::<Vec<_>>(),
                "skipped": outcome.skipped,
            });
            (outcome.passed(), attempts, last_failure, summary)
        }
        None => {
            let step = StepRequest {
                id: format!("greeter-{}", task.name),
                instruction: task.instruction.into(),
                context: task.context.into(),
                files: task.files.iter().map(|f| f.to_string()).collect(),
                kind: Default::default(),
                scope: Default::default(),
            };
            match implementer.run_step(&config.repo, &step) {
                StepOutcome::Passed { attempts } => (true, attempts, None, serde_json::Value::Null),
                StepOutcome::Failed { attempts, last_failure } => {
                    (false, attempts, Some(last_failure), serde_json::Value::Null)
                }
            }
        }
    };
    let step_ms = started.elapsed().as_millis() as u64;
    let metrics_after_step = metrics.snapshot();
    let nav = navigation(&recorder.events());
    let run_report = RunReport::from_events(
        &recorder
            .events()
            .iter()
            .filter_map(|e| serde_json::to_value(e).ok())
            .collect::<Vec<_>>(),
    );

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
        let path = config.repo.join(task.hidden_test_path);
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
    let untouched_ok = task
        .untouched
        .iter()
        .all(|path| !files_changed.iter().any(|changed| changed == path));
    let structure_results = structure::run(&config.repo, structure_for(task.name));
    let structure_ok = structure_results.iter().all(|r| r.passed);
    for failed in structure_results.iter().filter(|r| !r.passed) {
        println!("  structure check failed: {}", failed.description);
    }
    let own_tests_cover = structure::run(&config.repo, info_for(task.name));
    if !own_tests_cover.is_empty() {
        println!(
            "  own tests cover: {}",
            own_tests_cover
                .iter()
                .map(|r| format!("{} {}", if r.passed { "yes:" } else { "NO:" }, r.description))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    let scored =
        acceptance == "passed" && kept_original && untouched_ok && structure_ok && tests_added >= task.min_tests_added;
    println!(
        "  hidden acceptance: {acceptance}; original tests kept: {kept_original}; untouched kept: {untouched_ok}; \
tests added: {tests_added} (need {}); scored: {scored}",
        task.min_tests_added
    );

    let summary = json!({
        "run": n,
        "passed": passed,
        "attempts": attempts,
        "last_failure": last_failure,
        "plan": plan_summary,
        "step_ms": step_ms,
        "acceptance": acceptance,
        "acceptance_digest": acceptance_digest,
        "own_tests": own_tests,
        "tests_before": tests_before,
        "tests_added": tests_added,
        "original_tests_kept": kept_original,
        "untouched_files_kept": untouched_ok,
        "scored_pass": scored,
        "own_tests_cover": own_tests_cover.iter().map(|r| json!({"check": r.description, "passed": r.passed})).collect::<Vec<_>>(),
        "structure_ok": structure_ok,
        "structure": structure_results.iter().map(|r| json!({"check": r.description, "passed": r.passed})).collect::<Vec<_>>(),
        "files_changed": files_changed,
        "navigation": {
            "tool_calls": nav.tool_calls,
            "files_read": nav.files_read,
            "read_production": nav.read_production,
            "read_test": nav.read_test,
            "refused_submissions": nav.refused_submissions,
            "wasted_calls": nav.wasted_calls,
        },
        "metrics": metrics_after_step,
        "report": run_report,
        "plan_content_id": plan_text.as_deref().map(content_id),
    });
    std::fs::write(
        run_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    // Tell the plan's own directory how this run of it went, so the Planner's batch can be joined to it.
    if let Some(plan_path) = &config.plan {
        if let Some(dir) = plan_path.parent() {
            let line = json!({
                "implementer_batch": batch_id,
                "run": n,
                "scored": scored,
                "steps_passed": run_report.steps_passed,
                "steps_total": run_report.steps_total,
                "failed_step": run_report.failed_step,
                "stop_reason": run_report.steps.iter().find(|s| !s.passed).and_then(|s| s.stop_reason.clone()),
                "refusals": run_report.steps.iter().find(|s| !s.passed).map(|s| s.refusals.clone()),
                "plan_content_id": plan_text.as_deref().map(content_id),
            });
            let appended = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("implementer-runs.jsonl"))
                .and_then(|mut f| std::io::Write::write_all(&mut f, format!("{line}\n").as_bytes()));
            if let Err(e) = appended {
                println!("  could not record the run next to its plan: {e}");
            }
        }
    }

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
        nav,
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
    // The JVM Maven runs on decides what compiles and what Mockito can mock.
    let maven = std::process::Command::new("mvn")
        .arg("-version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let batch = json!({
        "batch": batch_id,
        "maven": maven,
        "started_at": now_iso(),
        "harness_commit": harness_commit.trim(),
        "harness_dirty": harness_dirty,
        "repository_tag": config.tag,
        "repository_commit": tag_commit.trim(),
        "model_requested": config.model,
        "url": config.url,
        "sampling": Sampling::qwen3(),
        "max_attempts": config.attempts,
        "declare_imports": config.declare_imports,
        "classpath_check": config.classpath,
        "surfaces": config.surfaces,
        "plan_file": config.plan.as_ref().map(|p| p.display().to_string()),
        "runs": config.runs,
        "task": task.name,
        "repo": task.repo,
        "tools": task.tools,
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
    if task.tools {
        println!(
            "\n{:>3} {:>6} {:>5} {:>9} {:>9} {:>8} {:>7}   files read",
            "run", "calls", "reads", "read_prod", "read_test", "refused", "wasted"
        );
        for r in &results {
            println!(
                "{:>3} {:>6} {:>5} {:>9} {:>9} {:>8} {:>7}   {}",
                r.n,
                r.nav.tool_calls,
                r.nav.files_read.len(),
                r.nav.read_production,
                r.nav.read_test,
                r.nav.refused_submissions,
                r.nav.wasted_calls,
                r.nav.files_read.join(", ")
            );
        }
    }
    let scored = results.iter().filter(|r| r.scored).count();
    println!(
        "\n{scored}/{} runs scored: hidden acceptance passed, original tests kept, untouched files unchanged, enough tests added. Trace and summaries: {}",
        results.len(),
        batch_dir.display()
    );
    Ok(())
}
