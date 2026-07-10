use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_llm::{ChatMessage, ToolCall, ToolSpec, ToolTurn};

const MAX_ITERATIONS: usize = 4;

/// EXPERIMENTAL — answers one question: can the configured local model reliably use a tool at
/// all? Not wired into `implement` or any real pipeline. Builds a tiny, self-contained scratch
/// scenario (a source file with an arbitrary, ungessable signature), gives the model exactly one
/// tool (`read_file`, sandboxed to the scratch directory), and asks a question that can only be
/// answered correctly by actually reading the file — pattern-completion from training data can't
/// get this right, so a correct final answer is real evidence the tool was used, not luck.
///
/// This deliberately does NOT try to be a general agent loop, a new `implement` mode, or
/// anything reusable yet — see CLAUDE.md's Diagnosing Dogfooding Runs / the session notes on
/// why this is being tried before building anything bigger on the assumption that it works.
pub(crate) fn cmd_try_tools(debug: bool) -> Result<()> {
    let client = build_client("developer", debug)?;

    let scratch_dir = std::env::temp_dir().join(format!("canopy-try-tools-{}", std::process::id()));
    std::fs::create_dir_all(&scratch_dir).context("failed to create scratch dir")?;
    // Deliberately arbitrary — a model has no training-data prior for this exact signature, so
    // getting it right is only possible by actually reading the file the tool exposes.
    let scratch_file = "shipping.ts";
    let file_content = "export function calculateShippingFee(\n  distanceKm: number,\n  weightKg: number,\n  isExpressDelivery: boolean,\n  insuredValueUsd?: number\n): number {\n  throw new Error('not implemented');\n}\n";
    std::fs::write(scratch_dir.join(scratch_file), file_content)
        .context("failed to write scratch file")?;

    println!("Scratch scenario: {}", scratch_dir.join(scratch_file).display());
    println!("(the model is never shown this file directly — it must call read_file to see it)\n");

    let read_file_tool = ToolSpec {
        name: "read_file".to_string(),
        description: "Read the contents of a file by its name. Only files in the current working scenario are accessible.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "The file name to read, e.g. \"shipping.ts\""}
            },
            "required": ["path"]
        }),
    };

    let mut messages = vec![ChatMessage::User(format!(
        "There is a TypeScript file named \"{scratch_file}\" that you have not seen. It exports \
         one function, `calculateShippingFee`. Call the `read_file` tool to read \"{scratch_file}\" \
         and find its exact parameter list, then answer with ONLY the parameter names and types, \
         in order, comma-separated — nothing else, no explanation, no code fences."
    ))];

    let mut tool_call_count = 0usize;
    let mut final_answer: Option<String> = None;

    for iteration in 1..=MAX_ITERATIONS {
        println!("--- iteration {iteration}/{MAX_ITERATIONS} ---");
        let turn = client.complete_with_tools(&messages, std::slice::from_ref(&read_file_tool))
            .context("LLM tool-call request failed")?;

        match turn {
            ToolTurn::ToolCalls(calls) => {
                println!("model called {} tool(s):", calls.len());
                messages.push(ChatMessage::Assistant { content: None, tool_calls: calls.clone() });
                for call in &calls {
                    tool_call_count += 1;
                    let result = dispatch_tool(call, &scratch_dir);
                    println!("  {}({}) -> {}", call.name, call.arguments,
                        if result.len() > 80 { format!("{}...", &result[..80]) } else { result.clone() });
                    messages.push(ChatMessage::Tool { tool_call_id: call.id.clone(), content: result });
                }
            }
            ToolTurn::FinalText(text) => {
                println!("model answered directly: {text}");
                final_answer = Some(text);
                break;
            }
        }
    }

    let _ = std::fs::remove_dir_all(&scratch_dir);

    println!("\n=== Report ===");
    println!("tool calls made:   {tool_call_count}");
    match &final_answer {
        Some(answer) => {
            println!("final answer:      {answer}");
            let expected_names = ["distanceKm", "weightKg", "isExpressDelivery", "insuredValueUsd"];
            let matched = expected_names.iter().filter(|n| answer.contains(*n)).count();
            println!("expected params:   {} (distanceKm: number, weightKg: number, isExpressDelivery: boolean, insuredValueUsd?: number)", expected_names.len());
            println!("names matched:     {matched}/{}", expected_names.len());
            if tool_call_count == 0 {
                println!("verdict:           DID NOT call the tool — answer is a guess, not evidence of tool use.");
            } else if matched == expected_names.len() {
                println!("verdict:           called the tool AND got the real signature right.");
            } else {
                println!("verdict:           called the tool but the final answer doesn't fully match — used the tool, but not reliably.");
            }
        }
        None => {
            println!("final answer:      (none — exhausted {MAX_ITERATIONS} iterations without a final answer)");
            println!("verdict:           INCONCLUSIVE — model kept calling tools (or got stuck) past the iteration budget.");
        }
    }

    Ok(())
}

/// Executes one tool call. Only `read_file` exists right now — anything else (a hallucinated
/// tool name) is reported back as an error string, the same way a real tool registry would.
fn dispatch_tool(call: &ToolCall, scratch_dir: &std::path::Path) -> String {
    match call.name.as_str() {
        "read_file" => {
            let Some(path) = call.arguments.get("path").and_then(|v| v.as_str()) else {
                return "error: missing required \"path\" argument".to_string();
            };
            // Sandboxed: reject anything that isn't a plain file name inside scratch_dir —
            // this is a throwaway temp dir either way, but a hallucinated "../../etc/passwd"
            // style path should still be refused rather than silently resolved.
            if path.contains("..") || path.starts_with('/') {
                return format!("error: path \"{path}\" is not allowed");
            }
            match std::fs::read_to_string(scratch_dir.join(path)) {
                Ok(content) => content,
                Err(e) => format!("error reading \"{path}\": {e}"),
            }
        }
        other => format!("error: unknown tool \"{other}\""),
    }
}
