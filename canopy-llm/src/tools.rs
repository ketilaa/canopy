use serde::Deserialize;

/// Describes one tool the model may call, in the OpenAI/Anthropic function-calling shape.
/// `parameters` is a JSON Schema object (e.g. `{"type": "object", "properties": {...}, "required": [...]}`).
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// One tool call the model asked to make.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// One entry in a tool-enabled conversation. Deliberately minimal — just enough to run a
/// bounded call/respond loop, not a general chat abstraction.
pub enum ChatMessage {
    User(String),
    /// The model's own prior turn — either a final text answer, or a request to call tools
    /// (mutually exclusive in the OpenAI/Anthropic wire format: a turn is one or the other).
    Assistant { content: Option<String>, tool_calls: Vec<ToolCall> },
    /// The result of executing one tool call, correlated back by `tool_call_id`.
    Tool { tool_call_id: String, content: String },
}

/// What one round of `complete_with_tools` produced.
pub enum ToolTurn {
    /// The model wants to call one or more tools before answering.
    ToolCalls(Vec<ToolCall>),
    /// The model produced a final answer with no further tool calls.
    FinalText(String),
}

#[derive(Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiFunctionCall,
}

/// Parses an OpenAI-compatible `choices[0].message` object into a `ToolTurn`. Shared by
/// `LlmClient::complete_with_tools` and available standalone for testing against fixture JSON
/// without needing a live server. `known_tools` is used only for the plain-text fallback (see
/// `fallback_parse_tool_call`) — the structured `tool_calls` path trusts the server's own parse.
pub(crate) fn parse_openai_message(message: &serde_json::Value, known_tools: &[ToolSpec]) -> Result<ToolTurn, String> {
    let raw_calls = message.get("tool_calls").and_then(|v| v.as_array());
    match raw_calls {
        Some(calls) if !calls.is_empty() => {
            let mut parsed = Vec::with_capacity(calls.len());
            for call in calls {
                let call: OpenAiToolCall = serde_json::from_value(call.clone())
                    .map_err(|e| format!("malformed tool_call: {e}"))?;
                let arguments: serde_json::Value = serde_json::from_str(&call.function.arguments)
                    .unwrap_or(serde_json::Value::String(call.function.arguments.clone()));
                parsed.push(ToolCall { id: call.id, name: call.function.name, arguments });
            }
            Ok(ToolTurn::ToolCalls(parsed))
        }
        _ => {
            let content = message.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
            match fallback_parse_tool_call(&content, known_tools) {
                Some(call) => Ok(ToolTurn::ToolCalls(vec![call])),
                None => Ok(ToolTurn::FinalText(content)),
            }
        }
    }
}

/// Extracts every balanced top-level `{...}` substring from `content`, string-literal-aware so
/// a brace inside a JSON string value doesn't miscount depth. This project's actual local setup
/// (Qwen2.5-Coder via llama-server) has been observed, over real runs, emitting a tool call as:
/// the correct `<tool_call>` tag, the wrong `<tools>` tag (echoing the tag used to PRESENT tools
/// to it), no wrapper at all, or — the shape that first slipped past a single-candidate parse —
/// the tool's own spec echoed inside `<tools>` followed by the real call as a second, unwrapped
/// object later in the same response. Scanning for every top-level object rather than assuming
/// one wrapper or the whole trimmed string is one JSON value handles all of these uniformly.
fn extract_json_objects(content: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    let mut in_string = false;
    let mut escaped = false;

    for (i, c) in content.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        objects.push(&content[s..=i]);
                    }
                    start = None;
                }
            }
            _ => {}
        }
    }
    objects
}

/// Tries to recognize a tool call in plain-text `content` when the model didn't use the
/// structured `tool_calls` response field — tagged, bare, or buried among other JSON objects in
/// the same response (also observed). Only returns `Some` for the first candidate object that
/// names one of `known_tools` with an `arguments` value present, so ordinary prose or an echoed
/// tool spec that happens to contain `{...}` isn't misread as a tool call.
fn fallback_parse_tool_call(content: &str, known_tools: &[ToolSpec]) -> Option<ToolCall> {
    for candidate in extract_json_objects(content) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) else { continue };
        let Some(name) = value.get("name").and_then(|n| n.as_str()) else { continue };
        if !known_tools.iter().any(|t| t.name == name) {
            continue;
        }
        let Some(arguments) = value.get("arguments").cloned() else { continue };
        return Some(ToolCall { id: format!("fallback-{name}"), name: name.to_string(), arguments });
    }
    None
}

/// Builds the OpenAI-compatible `messages` array entry for one `ChatMessage`.
pub(crate) fn message_to_json(msg: &ChatMessage) -> serde_json::Value {
    match msg {
        ChatMessage::User(text) => serde_json::json!({"role": "user", "content": text}),
        ChatMessage::Assistant { content, tool_calls } => {
            let mut obj = serde_json::json!({"role": "assistant"});
            if let Some(c) = content {
                obj["content"] = serde_json::Value::String(c.clone());
            }
            if !tool_calls.is_empty() {
                obj["tool_calls"] = serde_json::Value::Array(tool_calls.iter().map(|tc| {
                    serde_json::json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": serde_json::to_string(&tc.arguments).unwrap_or_default(),
                        }
                    })
                }).collect());
            }
            obj
        }
        ChatMessage::Tool { tool_call_id, content } => serde_json::json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": content,
        }),
    }
}

/// The `find_symbol` tool spec, offered to the fix loop by `canopy-cli` — a Roots-backed
/// symbol lookup so the model can resolve a missing import by looking it up instead of
/// guessing. The actual lookup logic lives in canopy-cli (canopy-llm has no Roots dependency);
/// this is only the wire-format description of the capability.
pub fn find_symbol_tool_spec() -> ToolSpec {
    ToolSpec {
        name: "find_symbol".to_string(),
        description: "ALWAYS prefer this over read_file for a missing name or import error. Look up where a symbol (function, class, interface) is defined in this project by its exact name. Returns its kind, defining file and line, the exact relative import specifier to use, and whether it needs `import type` — or reports that it wasn't found.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "The exact symbol name to look up, e.g. \"createWidget\""}
            },
            "required": ["name"]
        }),
    }
}

/// The `read_file` tool spec, offered to the fix loop alongside `find_symbol` — lets the model
/// pull the content of any project file it decides it needs, instead of every potentially
/// relevant file being pushed into the prompt upfront whether or not this attempt actually
/// needs it. The actual file access lives in canopy-cli (sandboxed to the project root); this
/// is only the wire-format description of the capability.
pub fn read_file_tool_spec() -> ToolSpec {
    ToolSpec {
        name: "read_file".to_string(),
        description: "LAST RESORT — only call this when you need to see a file's actual implementation, several symbols in it at once, or something find_symbol can't answer (it only resolves a single symbol's location/import). For a missing name or import error, ALWAYS try find_symbol first. Reads the full content of a file in this project by its project-relative path.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Project-relative file path, e.g. \"services/widget/src/models/Widget.ts\""}
            },
            "required": ["path"]
        }),
    }
}

/// Builds the "Tools available" prompt section from whichever tools are actually being offered
/// in this call — empty when `tools` is empty. Shared by every prompt-building function that
/// may or may not have tool access (fix, test-gen, stub-gen): the hint text must only appear
/// when the tool it names is actually callable in that specific call, never unconditionally in
/// shared skill text — a call with no tool access can't act on being told to use one (confirmed:
/// this exact mistake once left "ALWAYS call find_symbol" sitting in tool-less test-gen prompts).
pub(crate) fn tools_hint_section(tools: &[ToolSpec]) -> String {
    let mut hints = Vec::new();
    if tools.iter().any(|t| t.name == "find_symbol") {
        hints.push("- find_symbol: resolves a symbol's exact import specifier and whether it's type-only (`import type` vs `import`) — ALWAYS use it instead of re-deriving a path by hand or guessing.");
    }
    if tools.iter().any(|t| t.name == "read_file") {
        hints.push("- read_file: reads any project file's full content — last resort, only when find_symbol can't answer what you need.");
    }
    if hints.is_empty() {
        String::new()
    } else {
        format!("\nTools available — ALWAYS prefer them over guessing:\n{}\n", hints.join("\n"))
    }
}

/// Builds the OpenAI-compatible `tools` array entry for one `ToolSpec`.
pub(crate) fn tool_to_json(tool: &ToolSpec) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_file_tool() -> ToolSpec {
        ToolSpec {
            name: "read_file".to_string(),
            description: "reads a file".to_string(),
            parameters: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }
    }

    #[test]
    fn parses_final_text_when_no_tool_calls_and_no_fallback_match() {
        let msg = serde_json::json!({"role": "assistant", "content": "the answer is 42"});
        match parse_openai_message(&msg, &[read_file_tool()]).unwrap() {
            ToolTurn::FinalText(t) => assert_eq!(t, "the answer is 42"),
            ToolTurn::ToolCalls(_) => panic!("expected FinalText"),
        }
    }

    #[test]
    fn parses_tool_calls_with_json_arguments() {
        let msg = serde_json::json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "read_file", "arguments": "{\"path\": \"shipping.ts\"}"}
            }]
        });
        match parse_openai_message(&msg, &[read_file_tool()]).unwrap() {
            ToolTurn::ToolCalls(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "read_file");
                assert_eq!(calls[0].arguments["path"], "shipping.ts");
            }
            ToolTurn::FinalText(_) => panic!("expected ToolCalls"),
        }
    }

    // The following four mirror the exact shapes observed over 5 real runs against this
    // project's actual local setup (Qwen2.5-Coder via llama-server) when tool_calls came back
    // empty and the model expressed its call as plain content instead.

    #[test]
    fn fallback_parses_tools_wrapper_single_line() {
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "<tools>\n{\"name\": \"read_file\", \"arguments\": {\"path\": \"shipping.ts\"}}\n</tools>"
        });
        match parse_openai_message(&msg, &[read_file_tool()]).unwrap() {
            ToolTurn::ToolCalls(calls) => {
                assert_eq!(calls[0].name, "read_file");
                assert_eq!(calls[0].arguments["path"], "shipping.ts");
            }
            ToolTurn::FinalText(t) => panic!("expected ToolCalls, got FinalText: {t}"),
        }
    }

    #[test]
    fn fallback_parses_tools_wrapper_multiline_indented() {
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "<tools>\n{\n  \"name\": \"read_file\",\n  \"arguments\": {\n    \"path\": \"shipping.ts\"\n  }\n}\n</tools>"
        });
        match parse_openai_message(&msg, &[read_file_tool()]).unwrap() {
            ToolTurn::ToolCalls(calls) => assert_eq!(calls[0].name, "read_file"),
            ToolTurn::FinalText(t) => panic!("expected ToolCalls, got FinalText: {t}"),
        }
    }

    #[test]
    fn fallback_parses_bare_json_with_no_wrapper() {
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "{\"name\": \"read_file\", \"arguments\": {\"path\": \"shipping.ts\"}}"
        });
        match parse_openai_message(&msg, &[read_file_tool()]).unwrap() {
            ToolTurn::ToolCalls(calls) => assert_eq!(calls[0].name, "read_file"),
            ToolTurn::FinalText(t) => panic!("expected ToolCalls, got FinalText: {t}"),
        }
    }

    #[test]
    fn fallback_parses_correct_tool_call_tag_too() {
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "<tool_call>\n{\"name\": \"read_file\", \"arguments\": {\"path\": \"shipping.ts\"}}\n</tool_call>"
        });
        match parse_openai_message(&msg, &[read_file_tool()]).unwrap() {
            ToolTurn::ToolCalls(calls) => assert_eq!(calls[0].name, "read_file"),
            ToolTurn::FinalText(t) => panic!("expected ToolCalls, got FinalText: {t}"),
        }
    }

    #[test]
    fn fallback_parses_real_call_after_an_echoed_tool_spec() {
        // Observed live: the model echoes the tool's own spec (no "arguments" key, so it's
        // correctly skipped) inside <tools>, then appends the real call as a second, unwrapped
        // JSON object later in the same response.
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "<tools>\n{\"type\": \"function\", \"function\": {\"name\": \"read_file\", \"description\": \"reads a file\", \"parameters\": {\"type\": \"object\"}}}\n</tools>\n{\"name\": \"read_file\", \"arguments\": {\"path\": \"shipping.ts\"}}"
        });
        match parse_openai_message(&msg, &[read_file_tool()]).unwrap() {
            ToolTurn::ToolCalls(calls) => {
                assert_eq!(calls[0].name, "read_file");
                assert_eq!(calls[0].arguments["path"], "shipping.ts");
            }
            ToolTurn::FinalText(t) => panic!("expected ToolCalls, got FinalText: {t}"),
        }
    }

    #[test]
    fn fallback_does_not_misfire_on_unrelated_json_prose() {
        // A tool NAME must match one we actually offered — otherwise this would misread any
        // JSON-shaped example in an explanation as a real tool call.
        let msg = serde_json::json!({
            "role": "assistant",
            "content": "Here's an example config: {\"name\": \"some_config_key\", \"arguments\": {\"x\": 1}}"
        });
        match parse_openai_message(&msg, &[read_file_tool()]).unwrap() {
            ToolTurn::FinalText(_) => {}
            ToolTurn::ToolCalls(_) => panic!("should not have matched an unknown tool name"),
        }
    }

    #[test]
    fn message_to_json_roundtrips_tool_call() {
        let msg = ChatMessage::Assistant {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "shipping.ts"}),
            }],
        };
        let json = message_to_json(&msg);
        assert_eq!(json["tool_calls"][0]["function"]["name"], "read_file");
        assert_eq!(json["tool_calls"][0]["id"], "call_1");
    }
}
