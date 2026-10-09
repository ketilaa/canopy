//! Typed run events and the observers that consume them.
//!
//! Emitting an event is the only way anything about a run is reported. The trace file, the
//! console, metrics and tests are all observers of the same stream, so what a test asserts is
//! exactly what a later analysis reads.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::llm::{Sampling, ToolCall};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// Could not reach the server, or the connection broke.
    Transport,
    /// The request exceeded the configured timeout.
    Timeout,
    /// The server answered with a non-success HTTP status.
    HttpStatus,
    /// The server answered successfully but the body is not a usable completion.
    BadShape,
}

/// What the server itself reports about where the time went (llama-server `timings`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ServerTimings {
    pub prompt_ms: f64,
    pub predicted_ms: f64,
    pub prompt_per_second: f64,
    pub predicted_per_second: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LlmRequestSent {
    pub call_id: u64,
    pub purpose: String,
    pub model: String,
    /// The messages exactly as sent on the wire, including tool calls and tool results.
    pub messages: Vec<serde_json::Value>,
    /// Names of the tools offered on this call (empty for schema-constrained calls).
    pub tools: Vec<String>,
    /// Empty and null when the call is not schema-constrained.
    pub schema_name: String,
    pub schema: serde_json::Value,
    pub sampling: Sampling,
    pub thinking: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LlmResponseReceived {
    pub call_id: u64,
    pub purpose: String,
    /// The model id the server reports, which may differ from the one requested.
    pub model: String,
    pub finish_reason: String,
    pub content: String,
    /// The tool calls the model asked for, if any.
    pub tool_calls: Vec<ToolCall>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: Option<u64>,
    /// Wall-clock time of the whole call, measured by the client.
    pub elapsed_ms: u64,
    pub server: Option<ServerTimings>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LlmCallFailed {
    pub call_id: u64,
    pub purpose: String,
    pub kind: FailureKind,
    pub status: Option<u16>,
    pub message: String,
    /// The raw response body when there was one.
    pub body: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildKind {
    /// Compiles main and test sources; runs no tests.
    Compile,
    /// Runs the test suite (which compiles first).
    Test,
}

/// A build command the harness ran. The model never decides whether this happens.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandRan {
    pub kind: BuildKind,
    pub command: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub elapsed_ms: u64,
    /// Full combined output. The digest shown to the model is derived from this.
    pub output: String,
}

/// The stage of an attempt at which it failed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The model call itself failed.
    Llm,
    /// The model's answer was cut off at the token limit.
    Truncated,
    /// The answer did not match the schema.
    Parse,
    /// A path in the answer was unsafe, or writing failed.
    Write,
    Compile,
    Test,
    /// The model used up its tool calls or turns without submitting.
    Budget,
    /// The model kept answering in prose instead of calling a tool.
    Protocol,
    /// The model repeated the same tool call until the harness ended the attempt.
    Stuck,
    /// Every submitted file was identical to what is already there: nothing would change.
    NoChange,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepStarted {
    pub step_id: String,
    pub instruction: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttemptStarted {
    pub step_id: String,
    pub attempt: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FilesWritten {
    pub step_id: String,
    pub attempt: u32,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttemptFailed {
    pub step_id: String,
    pub attempt: u32,
    pub stage: Stage,
    /// What the next attempt is told went wrong.
    pub feedback: String,
}

/// The workspace was put back exactly as it was before the attempt wrote anything.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkspaceRestored {
    pub step_id: String,
    pub attempt: u32,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepFinished {
    pub step_id: String,
    pub passed: bool,
    pub attempts: u32,
    pub elapsed_ms: u64,
}

/// A plan began: the steps it will run, in order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanStarted {
    pub plan_id: String,
    pub step_ids: Vec<String>,
}

/// A plan ended, either having run every step or having stopped at the first one that failed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanFinished {
    pub plan_id: String,
    pub passed: bool,
    pub steps_passed: usize,
    pub steps_total: usize,
    pub failed_step: Option<String>,
    /// Problems found with the plan before any step ran.
    pub issues: Vec<String>,
    pub elapsed_ms: u64,
}

/// A tool call the model made, and what the harness answered.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolCalled {
    pub step_id: String,
    pub attempt: u32,
    pub tool: String,
    /// The raw JSON arguments as the model wrote them.
    pub arguments: String,
    pub outcome: ToolOutcome,
    /// Exactly what the model was told.
    pub result: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    LlmRequestSent(LlmRequestSent),
    LlmResponseReceived(LlmResponseReceived),
    LlmCallFailed(LlmCallFailed),
    ToolCalled(ToolCalled),
    PlanStarted(PlanStarted),
    PlanFinished(PlanFinished),
    CommandRan(CommandRan),
    StepStarted(StepStarted),
    AttemptStarted(AttemptStarted),
    FilesWritten(FilesWritten),
    AttemptFailed(AttemptFailed),
    WorkspaceRestored(WorkspaceRestored),
    StepFinished(StepFinished),
}

pub trait RunObserver: Send + Sync {
    fn on(&self, event: &RunEvent);
}

/// Ignores everything. The default when nobody is watching.
pub struct NoopObserver;

impl RunObserver for NoopObserver {
    fn on(&self, _event: &RunEvent) {}
}

/// Delivers every event to every observer, in order.
#[derive(Default)]
pub struct Observers {
    observers: Vec<Arc<dyn RunObserver>>,
}

impl Observers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, observer: Arc<dyn RunObserver>) -> Self {
        self.observers.push(observer);
        self
    }
}

impl RunObserver for Observers {
    fn on(&self, event: &RunEvent) {
        for observer in &self.observers {
            observer.on(event);
        }
    }
}

/// Keeps every event in memory. For tests and for in-process analysis.
#[derive(Default)]
pub struct RecordingObserver {
    events: Mutex<Vec<RunEvent>>,
}

impl RecordingObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<RunEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl RunObserver for RecordingObserver {
    fn on(&self, event: &RunEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

/// Writes each event as one JSON line: `{"run_id","seq","ts","event":{"type":...}}`.
/// Flushed per line, so a crashed run still leaves its trace. Write errors are swallowed:
/// observing a run must never be able to fail it.
pub struct TraceWriter {
    run_id: String,
    inner: Mutex<TraceInner>,
}

struct TraceInner {
    file: std::fs::File,
    seq: u64,
}

#[derive(Serialize)]
struct Envelope<'a> {
    run_id: &'a str,
    seq: u64,
    ts: String,
    event: &'a RunEvent,
}

impl TraceWriter {
    pub fn create(path: &Path, run_id: &str) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            run_id: run_id.to_string(),
            inner: Mutex::new(TraceInner { file, seq: 0 }),
        })
    }
}

impl RunObserver for TraceWriter {
    fn on(&self, event: &RunEvent) {
        use std::io::Write;
        let Ok(mut inner) = self.inner.lock() else { return };
        inner.seq += 1;
        let envelope = Envelope {
            run_id: &self.run_id,
            seq: inner.seq,
            ts: now_iso(),
            event,
        };
        if let Ok(mut line) = serde_json::to_string(&envelope) {
            line.push('\n');
            let _ = inner.file.write_all(line.as_bytes());
            let _ = inner.file.flush();
        }
    }
}

/// UTC timestamp as `YYYY-MM-DDThh:mm:ss.mmmZ`.
pub fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since_epoch.as_secs();
    let millis = since_epoch.subsec_millis();
    let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    // civil_from_days, after Howard Hinnant
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{millis:03}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(id: u64) -> RunEvent {
        RunEvent::LlmCallFailed(LlmCallFailed {
            call_id: id,
            purpose: "p".into(),
            kind: FailureKind::Timeout,
            status: None,
            message: "m".into(),
            body: None,
            elapsed_ms: 5,
        })
    }

    #[test]
    fn fan_out_delivers_every_event_to_every_observer_in_order() {
        let a = Arc::new(RecordingObserver::new());
        let b = Arc::new(RecordingObserver::new());
        let all = Observers::new().with(a.clone()).with(b.clone());
        all.on(&failed(1));
        all.on(&failed(2));
        assert_eq!(a.events(), vec![failed(1), failed(2)]);
        assert_eq!(b.events(), vec![failed(1), failed(2)]);
    }

    #[test]
    fn events_serialise_with_a_type_tag() {
        let json = serde_json::to_value(failed(7)).unwrap();
        assert_eq!(json["type"], "llm_call_failed");
        assert_eq!(json["call_id"], 7);
        assert_eq!(json["kind"], "timeout");
    }

    #[test]
    fn trace_writer_writes_one_enveloped_line_per_event_with_increasing_seq() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run").join("events.jsonl");
        let writer = TraceWriter::create(&path, "run-42").unwrap();
        writer.on(&failed(1));
        writer.on(&failed(2));

        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<serde_json::Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["run_id"], "run-42");
        assert_eq!(lines[0]["seq"], 1);
        assert_eq!(lines[1]["seq"], 2);
        assert_eq!(lines[0]["event"]["type"], "llm_call_failed");
        assert_eq!(lines[1]["event"]["call_id"], 2);
        assert!(lines[0]["ts"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn trace_is_readable_while_the_writer_is_still_alive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let writer = TraceWriter::create(&path, "r").unwrap();
        writer.on(&failed(1));
        // No drop, no explicit flush: a crashed run must still have left its line.
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
        drop(writer);
    }

    #[test]
    fn now_iso_has_millisecond_utc_format() {
        let ts = now_iso();
        assert_eq!(ts.len(), 24, "{ts}");
        assert_eq!(&ts[10..11], "T");
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[19..20], ".");
    }
}

/// Totals derived from the event stream: what a run cost and where its failures were.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Metrics {
    pub llm_calls: u64,
    pub llm_failures: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: u64,
    /// Wall-clock time spent waiting for the model, including failed calls.
    pub llm_ms: u64,
    pub compile_runs: u64,
    pub test_runs: u64,
    /// Wall-clock time spent in the build tool.
    pub build_ms: u64,
    pub attempts: u64,
    /// Failed attempts by the stage they failed at (`compile`, `test`, `parse`, ...).
    pub failed_attempts_by_stage: std::collections::BTreeMap<String, u64>,
    pub steps_passed: u64,
    pub steps_failed: u64,
    /// Tool calls by tool name (`glob`, `read_file`, `submit_files`).
    pub tool_calls_by_name: std::collections::BTreeMap<String, u64>,
    /// Tool calls the harness declined on policy.
    pub tool_refusals: u64,
    /// Tool calls that were themselves wrong: bad arguments, unknown tool, missing file.
    pub tool_errors: u64,
}

/// Accumulates [`Metrics`] from the events it sees. Cheap to run next to any other observer.
#[derive(Default)]
pub struct MetricsObserver {
    metrics: Mutex<Metrics>,
}

impl MetricsObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Metrics {
        self.metrics.lock().unwrap().clone()
    }
}

impl RunObserver for MetricsObserver {
    fn on(&self, event: &RunEvent) {
        let mut m = self.metrics.lock().unwrap();
        match event {
            RunEvent::LlmRequestSent(_) => m.llm_calls += 1,
            RunEvent::LlmResponseReceived(r) => {
                m.prompt_tokens += r.prompt_tokens;
                m.completion_tokens += r.completion_tokens;
                m.cached_tokens += r.cached_tokens.unwrap_or(0);
                m.llm_ms += r.elapsed_ms;
            }
            RunEvent::LlmCallFailed(f) => {
                m.llm_failures += 1;
                m.llm_ms += f.elapsed_ms;
            }
            RunEvent::CommandRan(c) => {
                match c.kind {
                    BuildKind::Compile => m.compile_runs += 1,
                    BuildKind::Test => m.test_runs += 1,
                }
                m.build_ms += c.elapsed_ms;
            }
            RunEvent::AttemptStarted(_) => m.attempts += 1,
            RunEvent::AttemptFailed(f) => {
                let stage = serde_json::to_value(f.stage)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string));
                *m.failed_attempts_by_stage.entry(stage.unwrap_or_default()).or_insert(0) += 1;
            }
            RunEvent::StepFinished(f) => {
                if f.passed {
                    m.steps_passed += 1;
                } else {
                    m.steps_failed += 1;
                }
            }
            RunEvent::ToolCalled(t) => {
                *m.tool_calls_by_name.entry(t.tool.clone()).or_insert(0) += 1;
                match t.outcome {
                    ToolOutcome::Ok => {}
                    ToolOutcome::Refused => m.tool_refusals += 1,
                    ToolOutcome::Error => m.tool_errors += 1,
                }
            }
            RunEvent::FilesWritten(_)
            | RunEvent::WorkspaceRestored(_)
            | RunEvent::StepStarted(_)
            | RunEvent::PlanStarted(_)
            | RunEvent::PlanFinished(_) => {}
        }
    }
}

#[cfg(test)]
mod metrics_tests {
    use super::*;

    fn response(prompt: u64, completion: u64, cached: Option<u64>, ms: u64) -> RunEvent {
        RunEvent::LlmResponseReceived(LlmResponseReceived {
            call_id: 1,
            purpose: "p".into(),
            model: "m".into(),
            finish_reason: "stop".into(),
            content: String::new(),
            tool_calls: vec![],
            prompt_tokens: prompt,
            completion_tokens: completion,
            cached_tokens: cached,
            elapsed_ms: ms,
            server: None,
        })
    }

    fn command(kind: BuildKind, ms: u64) -> RunEvent {
        RunEvent::CommandRan(CommandRan {
            kind,
            command: "mvn".into(),
            success: true,
            exit_code: Some(0),
            timed_out: false,
            elapsed_ms: ms,
            output: String::new(),
        })
    }

    #[test]
    fn totals_tokens_and_time_across_calls_including_failed_ones() {
        let metrics = MetricsObserver::new();
        for event in [
            response(100, 50, Some(20), 1000),
            response(200, 70, None, 2000),
            RunEvent::LlmCallFailed(LlmCallFailed {
                call_id: 3,
                purpose: "p".into(),
                kind: FailureKind::Timeout,
                status: None,
                message: "m".into(),
                body: None,
                elapsed_ms: 300,
            }),
        ] {
            metrics.on(&event);
        }
        let m = metrics.snapshot();
        assert_eq!((m.prompt_tokens, m.completion_tokens, m.cached_tokens), (300, 120, 20));
        assert_eq!((m.llm_failures, m.llm_ms), (1, 3300));
    }

    #[test]
    fn counts_builds_by_kind_with_their_time() {
        let metrics = MetricsObserver::new();
        metrics.on(&command(BuildKind::Compile, 1500));
        metrics.on(&command(BuildKind::Compile, 1600));
        metrics.on(&command(BuildKind::Test, 2000));
        let m = metrics.snapshot();
        assert_eq!((m.compile_runs, m.test_runs, m.build_ms), (2, 1, 5100));
    }

    #[test]
    fn counts_attempts_failures_by_stage_and_step_results() {
        let metrics = MetricsObserver::new();
        let failed = |stage| {
            RunEvent::AttemptFailed(AttemptFailed {
                step_id: "s".into(),
                attempt: 1,
                stage,
                feedback: String::new(),
            })
        };
        let finished = |passed| {
            RunEvent::StepFinished(StepFinished {
                step_id: "s".into(),
                passed,
                attempts: 1,
                elapsed_ms: 1,
            })
        };
        for event in [
            RunEvent::AttemptStarted(AttemptStarted {
                step_id: "s".into(),
                attempt: 1,
            }),
            failed(Stage::Compile),
            RunEvent::AttemptStarted(AttemptStarted {
                step_id: "s".into(),
                attempt: 2,
            }),
            failed(Stage::Compile),
            RunEvent::AttemptStarted(AttemptStarted {
                step_id: "s".into(),
                attempt: 3,
            }),
            failed(Stage::Test),
            finished(false),
            finished(true),
        ] {
            metrics.on(&event);
        }
        let m = metrics.snapshot();
        assert_eq!(m.attempts, 3);
        assert_eq!(m.failed_attempts_by_stage.get("compile"), Some(&2));
        assert_eq!(m.failed_attempts_by_stage.get("test"), Some(&1));
        assert_eq!((m.steps_passed, m.steps_failed), (1, 1));
    }

    #[test]
    fn counts_tool_calls_by_name_and_separates_refusals_from_errors() {
        let metrics = MetricsObserver::new();
        let called = |tool: &str, outcome| {
            RunEvent::ToolCalled(ToolCalled {
                step_id: "s".into(),
                attempt: 1,
                tool: tool.into(),
                arguments: "{}".into(),
                outcome,
                result: String::new(),
                elapsed_ms: 1,
            })
        };
        for event in [
            called("glob", ToolOutcome::Ok),
            called("read_file", ToolOutcome::Ok),
            called("read_file", ToolOutcome::Refused),
            called("read_file", ToolOutcome::Error),
            called("submit_files", ToolOutcome::Refused),
        ] {
            metrics.on(&event);
        }
        let m = metrics.snapshot();
        assert_eq!(m.tool_calls_by_name.get("read_file"), Some(&3));
        assert_eq!(m.tool_calls_by_name.get("glob"), Some(&1));
        assert_eq!(m.tool_calls_by_name.get("submit_files"), Some(&1));
        assert_eq!((m.tool_refusals, m.tool_errors), (2, 1));
    }

    #[test]
    fn counts_every_request_sent_as_a_call() {
        let metrics = MetricsObserver::new();
        let sent = RunEvent::LlmRequestSent(LlmRequestSent {
            call_id: 1,
            purpose: "p".into(),
            model: "m".into(),
            messages: vec![],
            tools: vec![],
            schema_name: "s".into(),
            schema: serde_json::json!({}),
            sampling: Sampling::qwen3(),
            thinking: false,
        });
        metrics.on(&sent);
        metrics.on(&sent);
        assert_eq!(metrics.snapshot().llm_calls, 2);
    }
}

/// How a tool call ended, as seen by the model.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolOutcome {
    Ok,
    /// The harness declined on policy: outside the sandbox, too large, binary, not yet read.
    Refused,
    /// The call itself was wrong: bad arguments, an unknown tool, a missing file.
    Error,
}
