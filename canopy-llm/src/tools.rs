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

/// Tag pairs a tool call might be wrapped in, tried in order. `<tool_call>` is what this
/// project's actual local setup (Qwen2.5-Coder via llama-server, --jinja on by default) tells
/// the model to use for its own response. `<tools>` is confirmed, over 5 real runs against that
/// exact setup, to be what the model actually emits instead every time — it consistently echoes
/// the tag used to PRESENT the available tools to it rather than the different tag it's told to
/// respond with. This is a narrow, understood model formatting slip, not a capability gap (the
/// tool name and arguments were correct in all 5 runs) — worth normalizing here rather than
/// chasing with more prompt text aimed at a local model that isn't reading its own system
/// prompt's chat template instructions carefully enough to reproduce one exact tag name.
const TOOL_CALL_WRAPPERS: [(&str, &str); 2] = [
    ("<tool_call>", "</tool_call>"),
    ("<tools>", "</tools>"),
];

/// Tries to recognize a tool call in plain-text `content` when the model didn't use the
/// structured `tool_calls` response field — wrapped in a recognized tag pair, or as a bare JSON
/// object (also observed). Only returns `Some` when the parsed object names one of `known_tools`
/// with an `arguments` value present, so ordinary prose that happens to contain `{...}` isn't
/// misread as a tool call.
fn fallback_parse_tool_call(content: &str, known_tools: &[ToolSpec]) -> Option<ToolCall> {
    let trimmed = content.trim();
    let wrapped = TOOL_CALL_WRAPPERS.iter().find_map(|(open, close)| {
        let start = trimmed.find(open)? + open.len();
        let end = trimmed[start..].find(close)? + start;
        Some(trimmed[start..end].trim())
    });

    for candidate in wrapped.into_iter().chain(std::iter::once(trimmed)) {
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
