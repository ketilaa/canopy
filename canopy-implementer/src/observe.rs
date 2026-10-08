//! Typed run events and the observers that consume them.
//!
//! Emitting an event is the only way anything about a run is reported. The trace file, the
//! console, metrics and tests are all observers of the same stream, so what a test asserts is
//! exactly what a later analysis reads.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::llm::{Message, Sampling};

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
    pub messages: Vec<Message>,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEvent {
    LlmRequestSent(LlmRequestSent),
    LlmResponseReceived(LlmResponseReceived),
    LlmCallFailed(LlmCallFailed),
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
