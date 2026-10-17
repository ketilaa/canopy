//! A readable transcript of a run, rendered from its event trace: for every step and attempt the complete
//! prompt the model was given, then everything that happened in order (what the model said and called, what
//! each tool returned, what each gate said) and how the attempt ended. Nothing is summarised away: the point
//! is that a person can read exactly what the model saw and did.

use std::fmt::Write;

use serde_json::Value;

/// How many lines of a failed command's raw output are shown. The model never sees the raw output (it sees the
/// harness's feedback, shown in full), so this is context for the reader, and the trace holds the rest.
const COMMAND_EXCERPT_LINES: usize = 40;

/// The transcript of the events of one run, in order.
pub fn render(events: &[Value]) -> String {
    let mut out = String::new();
    for event in events {
        render_event(&mut out, event);
    }
    out
}

/// The transcript of an `events.jsonl` text (one `{"event": {...}}` object per line).
pub fn render_jsonl(text: &str) -> String {
    let events: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|line| line.get("event").cloned())
        .collect();
    render(&events)
}

fn text<'a>(event: &'a Value, key: &str) -> &'a str {
    event.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn fenced(body: &str) -> String {
    format!("```\n{}\n```\n", body.trim_end_matches('\n'))
}

fn render_event(out: &mut String, event: &Value) {
    match text(event, "type") {
        "step_started" => {
            let _ = writeln!(out, "\n# Step `{}`\n", text(event, "step_id"));
        }
        "attempt_started" => {
            let attempt = event.get("attempt").and_then(Value::as_u64).unwrap_or(0);
            let _ = writeln!(out, "\n## Attempt {attempt}\n");
        }
        "llm_request_sent" => render_request(out, event),
        "llm_response_received" => render_response(out, event),
        "llm_call_failed" => {
            let _ = writeln!(out, "**Model call failed:** {}\n", text(event, "message"));
        }
        "tool_called" => {
            let tool = text(event, "tool");
            let outcome = text(event, "outcome");
            let _ = writeln!(out, "**Tool result** `{tool}` ({outcome}):\n");
            out.push_str(&fenced(text(event, "result")));
            out.push('\n');
        }
        "files_written" => {
            let paths: Vec<&str> = event
                .get("paths")
                .and_then(Value::as_array)
                .map(|p| p.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let _ = writeln!(out, "**Files written:** {}\n", paths.join(", "));
        }
        "command_ran" => render_command(out, event),
        "attempt_failed" => {
            let _ = writeln!(
                out,
                "**Attempt failed at `{}`.** Feedback, in full:\n",
                text(event, "stage")
            );
            out.push_str(&fenced(text(event, "feedback")));
            out.push('\n');
        }
        "workspace_restored" => {
            let _ = writeln!(out, "**Workspace restored** to what it was before the attempt.\n");
        }
        "step_finished" => {
            let passed = event.get("passed").and_then(Value::as_bool).unwrap_or(false);
            let attempts = event.get("attempts").and_then(Value::as_u64).unwrap_or(0);
            let _ = writeln!(
                out,
                "\n**Step `{}` {} after {attempts} attempt(s).**\n",
                text(event, "step_id"),
                if passed { "PASSED" } else { "FAILED" }
            );
        }
        _ => {}
    }
}

/// The first request of an attempt carries the complete prompt (system and user message) and is printed in full.
/// Later requests of the same attempt repeat that and add the turns shown by the response and tool events, so
/// they print nothing again.
fn render_request(out: &mut String, event: &Value) {
    let messages = event.get("messages").and_then(Value::as_array).cloned().unwrap_or_default();
    let is_first = messages.iter().all(|m| matches!(text(m, "role"), "system" | "user"));
    if !is_first {
        return;
    }
    let _ = writeln!(out, "### Prompt (call {}, {})\n", event.get("call_id").and_then(Value::as_u64).unwrap_or(0), text(event, "purpose"));
    for message in &messages {
        let _ = writeln!(out, "**{}:**\n", text(message, "role").to_uppercase());
        out.push_str(&fenced(text(message, "content")));
        out.push('\n');
    }
    if let Some(tools) = event.get("tools").and_then(Value::as_array).filter(|t| !t.is_empty()) {
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t.pointer("/function/name").or_else(|| t.get("name")).and_then(Value::as_str))
            .collect();
        let _ = writeln!(out, "Tools offered: {}\n", names.join(", "));
    }
    if let Some(schema) = event.get("schema").filter(|s| !s.is_null()) {
        let _ = writeln!(out, "Response schema `{}`:\n", text(event, "schema_name"));
        out.push_str(&fenced(&serde_json::to_string_pretty(schema).unwrap_or_default()));
        out.push('\n');
    }
}

fn render_response(out: &mut String, event: &Value) {
    let _ = writeln!(
        out,
        "**MODEL** (call {}, finish `{}`, {} prompt + {} completion tokens, {} ms):\n",
        event.get("call_id").and_then(Value::as_u64).unwrap_or(0),
        text(event, "finish_reason"),
        event.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0),
        event.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0),
        event.get("elapsed_ms").and_then(Value::as_u64).unwrap_or(0),
    );
    let content = text(event, "content");
    if !content.is_empty() {
        out.push_str(&fenced(content));
        out.push('\n');
    }
    for call in event.get("tool_calls").and_then(Value::as_array).into_iter().flatten() {
        render_tool_call(out, text(call, "name"), text(call, "arguments"));
    }
}

/// A tool call as the model made it. `submit_files` is shown as the files it carries, in full.
fn render_tool_call(out: &mut String, name: &str, arguments: &str) {
    let _ = writeln!(out, "Calls `{name}`:\n");
    let parsed = serde_json::from_str::<Value>(arguments).ok();
    let files = parsed.as_ref().and_then(|a| a.get("files")).and_then(Value::as_array);
    match (name, files) {
        ("submit_files", Some(files)) if !files.is_empty() => {
            for file in files {
                let _ = writeln!(out, "`{}`:\n", text(file, "path"));
                out.push_str(&fenced(text(file, "content")));
                out.push('\n');
            }
        }
        _ => {
            out.push_str(&fenced(arguments));
            out.push('\n');
        }
    }
}

fn render_command(out: &mut String, event: &Value) {
    let success = event.get("success").and_then(Value::as_bool).unwrap_or(false);
    let _ = writeln!(
        out,
        "**Gate** `{}` ({}): {}\n",
        text(event, "command"),
        text(event, "kind"),
        if success { "ok" } else { "FAILED" }
    );
    if !success {
        let output = text(event, "output");
        let lines: Vec<&str> = output.lines().collect();
        let shown = lines.iter().take(COMMAND_EXCERPT_LINES).copied().collect::<Vec<_>>().join("\n");
        out.push_str(&fenced(&shown));
        if lines.len() > COMMAND_EXCERPT_LINES {
            let _ = writeln!(out, "({} more lines in the trace)\n", lines.len() - COMMAND_EXCERPT_LINES);
        }
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(call_id: u64, messages: Value) -> Value {
        json!({"type":"llm_request_sent","call_id":call_id,"purpose":"navigate","messages":messages,"tools":[{"type":"function","function":{"name":"glob"}}],"schema_name":"","schema":null})
    }

    fn first_prompt() -> Value {
        json!([{"role":"system","content":"SYSTEM RULES"},{"role":"user","content":"## Task\nDo the thing."}])
    }

    #[test]
    fn the_first_request_of_an_attempt_prints_the_whole_prompt_and_later_ones_do_not_repeat_it() {
        let later = json!([
            {"role":"system","content":"SYSTEM RULES"},
            {"role":"user","content":"## Task\nDo the thing."},
            {"role":"assistant","content":null,"tool_calls":[]},
            {"role":"tool","content":"result","tool_call_id":"1"}
        ]);
        let text = render(&[
            json!({"type":"attempt_started","step_id":"s","attempt":1}),
            request(1, first_prompt()),
            request(2, later),
        ]);
        assert_eq!(text.matches("SYSTEM RULES").count(), 1, "{text}");
        assert!(text.contains("## Task\nDo the thing."), "{text}");
        assert!(text.contains("Tools offered: glob"), "{text}");
        assert!(text.contains("## Attempt 1"), "{text}");
    }

    #[test]
    fn a_new_attempt_prints_its_own_complete_prompt_again() {
        let text = render(&[
            json!({"type":"attempt_started","step_id":"s","attempt":1}),
            request(1, first_prompt()),
            json!({"type":"attempt_started","step_id":"s","attempt":2}),
            request(3, first_prompt()),
        ]);
        assert_eq!(text.matches("SYSTEM RULES").count(), 2, "{text}");
    }

    #[test]
    fn what_the_model_said_and_called_is_shown_with_submitted_files_in_full() {
        let args = json!({"files":[{"path":"src/main/java/A.java","content":"class A {}\n"}]}).to_string();
        let text = render(&[json!({
            "type":"llm_response_received","call_id":2,"finish_reason":"tool_calls","content":"Thinking aloud.",
            "tool_calls":[{"id":"1","name":"submit_files","arguments":args}],
            "prompt_tokens":10,"completion_tokens":5,"elapsed_ms":1200
        })]);
        for needle in ["**MODEL** (call 2", "Thinking aloud.", "Calls `submit_files`", "`src/main/java/A.java`", "class A {}"] {
            assert!(text.contains(needle), "{needle}: {text}");
        }
    }

    #[test]
    fn a_tool_result_a_gate_verdict_and_the_failure_feedback_are_all_shown() {
        let output = (1..=60).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
        let text = render(&[
            json!({"type":"tool_called","tool":"glob","outcome":"ok","result":"No files match `src/**/*.java`."}),
            json!({"type":"command_ran","kind":"compile","command":"mvn -B test-compile","success":false,"output":output}),
            json!({"type":"attempt_failed","step_id":"s","attempt":1,"stage":"compile","feedback":"A.java:3: cannot find symbol"}),
            json!({"type":"workspace_restored"}),
            json!({"type":"step_finished","step_id":"s","passed":false,"attempts":3,"elapsed_ms":1}),
        ]);
        for needle in [
            "No files match `src/**/*.java`.",
            "**Gate** `mvn -B test-compile` (compile): FAILED",
            "line 40",
            "(20 more lines in the trace)",
            "Attempt failed at `compile`",
            "A.java:3: cannot find symbol",
            "Workspace restored",
            "FAILED after 3 attempt(s)",
        ] {
            assert!(text.contains(needle), "{needle}: {text}");
        }
        assert!(!text.contains("line 41"), "{text}");
    }

    #[test]
    fn a_passing_gate_is_one_line_and_jsonl_input_is_read_line_by_line() {
        let line = |event: Value| json!({"run_id":"r","seq":1,"event":event}).to_string();
        let jsonl = [
            line(json!({"type":"command_ran","kind":"test","command":"mvn -B test","success":true,"output":"BIG OUTPUT"})),
            "not json".to_string(),
            line(json!({"type":"step_finished","step_id":"s","passed":true,"attempts":1,"elapsed_ms":1})),
        ]
        .join("\n");
        let text = render_jsonl(&jsonl);
        assert!(text.contains("**Gate** `mvn -B test` (test): ok"), "{text}");
        assert!(!text.contains("BIG OUTPUT"), "{text}");
        assert!(text.contains("PASSED after 1 attempt(s)"), "{text}");
    }
}
