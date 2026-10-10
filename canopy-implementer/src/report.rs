//! What a run did, step by step, worked out from its event stream: which steps ran, how each ended, what
//! the harness refused and why, and where a failed step stopped.
//!
//! One pure function over events, so the same report comes from a live run (events serialised as they
//! happen) and from a stored `events.jsonl`. It reads the events as JSON, so old traces keep working when
//! event types gain fields.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

/// How much of a failed attempt's feedback a report keeps.
const FEEDBACK_HEAD: usize = 300;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RunReport {
    pub plan_id: Option<String>,
    pub steps_total: usize,
    pub steps_passed: usize,
    pub failed_step: Option<String>,
    /// The steps that ran, in order.
    pub steps: Vec<StepReport>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct StepReport {
    pub step_id: String,
    pub passed: bool,
    pub attempts: u32,
    pub elapsed_ms: u64,
    pub tool_calls: u32,
    /// The standing context the step was given.
    pub context: Option<String>,
    /// What the harness refused, by reason with the names taken out, and how often.
    pub refusals: BTreeMap<String, u32>,
    pub failures: Vec<AttemptFailure>,
    /// Where a failed step stopped: its last failed attempt.
    pub stop_reason: Option<AttemptFailure>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AttemptFailure {
    pub attempt: u32,
    pub stage: String,
    /// The head of what the next attempt was told, cut with an ellipsis.
    pub feedback: String,
}

impl RunReport {
    pub fn from_jsonl(text: &str) -> Self {
        let events: Vec<Value> = text
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .map(|v| v.get("event").cloned().unwrap_or(v))
            .collect();
        Self::from_events(&events)
    }

    pub fn from_events(events: &[Value]) -> Self {
        let mut report = RunReport::default();
        let mut finished_by_plan = false;
        for event in events {
            let text = |key: &str| event.get(key).and_then(Value::as_str).map(str::to_string);
            let number = |key: &str| event.get(key).and_then(Value::as_u64).unwrap_or(0);
            match event.get("type").and_then(Value::as_str) {
                Some("plan_started") => {
                    report.plan_id = text("plan_id");
                    report.steps_total = event.get("step_ids").and_then(Value::as_array).map_or(0, Vec::len);
                }
                Some("plan_finished") => {
                    finished_by_plan = true;
                    report.steps_total = number("steps_total") as usize;
                    report.steps_passed = number("steps_passed") as usize;
                    report.failed_step = text("failed_step");
                }
                Some("step_started") => {
                    if let Some(id) = text("step_id") {
                        report.steps.push(StepReport {
                            step_id: id,
                            context: text("context"),
                            ..StepReport::default()
                        });
                    }
                }
                Some("tool_called") => {
                    if let Some(step) = report.step_mut(text("step_id")) {
                        step.tool_calls += 1;
                        if event.get("outcome").and_then(Value::as_str) == Some("refused") {
                            let reason = refusal_reason(&text("result").unwrap_or_default());
                            *step.refusals.entry(reason).or_insert(0) += 1;
                        }
                    }
                }
                Some("attempt_failed") => {
                    let failure = AttemptFailure {
                        attempt: number("attempt") as u32,
                        stage: text("stage").unwrap_or_default(),
                        feedback: head(&text("feedback").unwrap_or_default()),
                    };
                    if let Some(step) = report.step_mut(text("step_id")) {
                        step.failures.push(failure);
                    }
                }
                Some("step_finished") => {
                    if let Some(step) = report.step_mut(text("step_id")) {
                        step.passed = event.get("passed").and_then(Value::as_bool).unwrap_or(false);
                        step.attempts = number("attempts") as u32;
                        step.elapsed_ms = number("elapsed_ms");
                        if !step.passed {
                            step.stop_reason = step.failures.last().cloned();
                        }
                    }
                }
                _ => {}
            }
        }
        if !finished_by_plan {
            report.steps_total = report.steps_total.max(report.steps.len());
            report.steps_passed = report.steps.iter().filter(|s| s.passed).count();
            report.failed_step = report.steps.iter().find(|s| !s.passed).map(|s| s.step_id.clone());
        }
        report
    }

    fn step_mut(&mut self, id: Option<String>) -> Option<&mut StepReport> {
        let id = id?;
        self.steps.iter_mut().rev().find(|s| s.step_id == id)
    }
}

/// A refusal's text with the names taken out (backticked names and `.java` paths), so refusals that differ
/// only in which class they mention count as one reason.
fn refusal_reason(text: &str) -> String {
    let mut out = String::new();
    let mut in_ticks = false;
    for c in text.chars() {
        match c {
            '`' if !in_ticks => {
                in_ticks = true;
                out.push('X');
            }
            '`' => in_ticks = false,
            _ if in_ticks => {}
            _ => out.push(c),
        }
    }
    out.split_whitespace()
        .map(
            |word| match word.trim_end_matches(['.', ',', ':', ';']).ends_with(".java") {
                true => "X".to_string() + &word[word.trim_end_matches(['.', ',', ':', ';']).len()..],
                false => word.to_string(),
            },
        )
        .collect::<Vec<_>>()
        .join(" ")
}

fn head(feedback: &str) -> String {
    let one_line = feedback.split('\n').collect::<Vec<_>>().join(" | ");
    match one_line.chars().count() > FEEDBACK_HEAD {
        true => one_line.chars().take(FEEDBACK_HEAD).collect::<String>() + "…",
        false => one_line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn events() -> Vec<serde_json::Value> {
        vec![
            json!({"type": "plan_started", "plan_id": "p", "step_ids": ["a", "b", "c"]}),
            json!({"type": "step_started", "step_id": "a", "instruction": "x", "context": "facts"}),
            json!({"type": "tool_called", "step_id": "a", "attempt": 1, "tool": "read_file", "outcome": "ok", "result": "..."}),
            json!({"type": "tool_called", "step_id": "a", "attempt": 1, "tool": "submit_files", "outcome": "refused",
                   "result": "Not submitted. No changed test mentions `WidgetController`. Add a test."}),
            json!({"type": "tool_called", "step_id": "a", "attempt": 1, "tool": "submit_files", "outcome": "refused",
                   "result": "Not submitted. No changed test mentions `WidgetService`. Add a test."}),
            json!({"type": "attempt_failed", "step_id": "a", "attempt": 1, "stage": "compile",
                   "feedback": "src/main/java/p/A.java:3:27: package jakarta.persistence does not exist"}),
            json!({"type": "step_finished", "step_id": "a", "passed": true, "attempts": 2, "elapsed_ms": 4000}),
            json!({"type": "step_started", "step_id": "b", "instruction": "y", "context": "facts"}),
            json!({"type": "tool_called", "step_id": "b", "attempt": 1, "tool": "glob", "outcome": "ok", "result": "..."}),
            json!({"type": "attempt_failed", "step_id": "b", "attempt": 1, "stage": "budget", "feedback": "You made exactly the same submit_files call 3 times without making progress."}),
            json!({"type": "attempt_failed", "step_id": "b", "attempt": 2, "stage": "budget", "feedback": "You made exactly the same submit_files call 3 times without making progress."}),
            json!({"type": "step_finished", "step_id": "b", "passed": false, "attempts": 2, "elapsed_ms": 9000}),
            json!({"type": "plan_finished", "plan_id": "p", "passed": false, "steps_passed": 1, "steps_total": 3, "failed_step": "b", "issues": [], "elapsed_ms": 13000}),
        ]
    }

    #[test]
    fn a_run_reports_each_step_with_its_outcome_attempts_and_tool_calls_in_plan_order() {
        let report = RunReport::from_events(&events());
        assert_eq!(report.plan_id.as_deref(), Some("p"));
        assert_eq!(report.steps_total, 3);
        assert_eq!(report.steps_passed, 1);
        assert_eq!(report.failed_step.as_deref(), Some("b"));
        let ids: Vec<&str> = report.steps.iter().map(|s| s.step_id.as_str()).collect();
        assert_eq!(ids, ["a", "b"], "only steps that ran have a report");
        let a = &report.steps[0];
        assert!(a.passed);
        assert_eq!(a.attempts, 2);
        assert_eq!(a.elapsed_ms, 4000);
        assert_eq!(a.tool_calls, 3);
        assert_eq!(a.context.as_deref(), Some("facts"));
    }

    #[test]
    fn refusals_are_counted_by_reason_with_the_names_taken_out() {
        let a = &RunReport::from_events(&events()).steps[0];
        assert_eq!(
            a.refusals.len(),
            1,
            "two refusals that differ only in a class name are one reason"
        );
        let (reason, count) = a.refusals.iter().next().unwrap();
        assert_eq!(reason, "Not submitted. No changed test mentions X. Add a test.");
        assert_eq!(*count, 2);
    }

    #[test]
    fn a_step_keeps_each_failed_attempt_with_its_stage_and_the_head_of_its_feedback() {
        let report = RunReport::from_events(&events());
        let a = &report.steps[0].failures;
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].attempt, 1);
        assert_eq!(a[0].stage, "compile");
        assert!(
            a[0].feedback.contains("jakarta.persistence does not exist"),
            "{:?}",
            a[0]
        );
    }

    #[test]
    fn a_failed_step_names_its_stop_reason_from_the_last_failed_attempt() {
        let report = RunReport::from_events(&events());
        let stop = report.steps[1].stop_reason.as_ref().expect("the step failed");
        assert_eq!(stop.stage, "budget");
        assert!(
            stop.feedback.starts_with("You made exactly the same submit_files call"),
            "{stop:?}"
        );
        assert!(
            report.steps[0].stop_reason.is_none(),
            "a step that passed has no stop reason"
        );
    }

    #[test]
    fn long_feedback_is_cut_and_the_cut_is_visible() {
        let long = "x".repeat(1000);
        let events = vec![
            json!({"type": "step_started", "step_id": "a", "instruction": "x", "context": ""}),
            json!({"type": "attempt_failed", "step_id": "a", "attempt": 1, "stage": "test", "feedback": long}),
            json!({"type": "step_finished", "step_id": "a", "passed": false, "attempts": 1, "elapsed_ms": 1}),
        ];
        let failure = &RunReport::from_events(&events).steps[0].failures[0];
        assert!(
            failure.feedback.len() < 400 && failure.feedback.ends_with('…'),
            "{}",
            failure.feedback.len()
        );
    }

    #[test]
    fn a_stored_trace_is_read_line_by_line_through_its_envelope_and_unreadable_lines_are_skipped() {
        let trace = events()
            .into_iter()
            .enumerate()
            .map(|(i, e)| json!({"run_id": "r", "seq": i, "ts": "t", "event": e}).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let from_file = RunReport::from_jsonl(&format!("not json\n{trace}\n\n"));
        assert_eq!(from_file, RunReport::from_events(&events()));
    }

    #[test]
    fn a_trace_with_no_steps_gives_an_empty_report() {
        let report = RunReport::from_jsonl("");
        assert!(report.steps.is_empty() && report.plan_id.is_none() && report.failed_step.is_none());
    }

    #[test]
    fn the_report_is_json_for_a_summary_file() {
        let value = serde_json::to_value(RunReport::from_events(&events())).unwrap();
        assert_eq!(value["steps"][1]["stop_reason"]["stage"], "budget");
        assert_eq!(value["steps_passed"], 1);
    }
}
