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
/// without needing a live server.
pub(crate) fn parse_openai_message(message: &serde_json::Value) -> Result<ToolTurn, String> {
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
            Ok(ToolTurn::FinalText(content))
        }
    }
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

    #[test]
    fn parses_final_text_when_no_tool_calls() {
        let msg = serde_json::json!({"role": "assistant", "content": "the answer is 42"});
        match parse_openai_message(&msg).unwrap() {
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
        match parse_openai_message(&msg).unwrap() {
            ToolTurn::ToolCalls(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].name, "read_file");
                assert_eq!(calls[0].arguments["path"], "shipping.ts");
            }
            ToolTurn::FinalText(_) => panic!("expected ToolCalls"),
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
