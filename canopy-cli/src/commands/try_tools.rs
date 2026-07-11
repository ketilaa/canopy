use crate::util::build_client;
use anyhow::{Context, Result};
use canopy_llm::{ChatMessage, LlmClient, ToolCall, ToolSpec, ToolTurn};
use roots_storage::Store;

const MAX_ITERATIONS: usize = 4;

/// EXPERIMENTAL — answers one question: can the configured local model reliably use a tool at
/// all? Not wired into `implement` or any real pipeline. Runs two independent scratch scenarios,
/// each self-contained and generically named (never leaking a dogfooding project's own
/// vocabulary into canopy's own source — see CLAUDE.md's "Generic placeholders" rule):
///
/// 1. `read_file` — a source file with an arbitrary, unguessable signature; pattern-completion
///    from training data can't get this right, so a correct final answer is real evidence the
///    tool was used, not luck.
/// 2. `find_symbol` — a real Roots-backed symbol lookup, tested against the exact recurring
///    real-world failure this was built to address: a file that calls a sibling module's export
///    without importing it (`TS2304: Cannot find name 'X'`), traced back to a genuine dogfooding
///    bug (`createProduct` never imported into `ProductService.ts`) rather than invented.
///
/// This deliberately does NOT try to be a general agent loop, a new `implement` mode, or
/// anything reusable yet — see CLAUDE.md's Diagnosing Dogfooding Runs / the session notes on
/// why this is being tried before building anything bigger on the assumption that it works.
pub(crate) fn cmd_try_tools(debug: bool) -> Result<()> {
    let client = build_client("developer", debug)?;

    println!("=== Scenario 1: read_file ===\n");
    run_read_file_scenario(&client)?;

    println!("\n=== Scenario 2: find_symbol (Roots-backed) ===\n");
    run_symbol_lookup_scenario(&client)?;

    Ok(())
}

/// Runs the model/tool loop to completion, printing each iteration as it happens. `dispatch`
/// executes one tool call and returns its result content. Shared by every scenario so each one
/// only has to define its own scratch setup, tool spec, and verdict logic.
fn run_tool_loop(
    client: &LlmClient,
    initial_message: ChatMessage,
    tool: &ToolSpec,
    mut dispatch: impl FnMut(&ToolCall) -> String,
) -> Result<(usize, Option<String>)> {
    let mut messages = vec![initial_message];
    let mut tool_call_count = 0usize;
    let mut final_answer: Option<String> = None;

    for iteration in 1..=MAX_ITERATIONS {
        println!("--- iteration {iteration}/{MAX_ITERATIONS} ---");
        let turn = client
            .complete_with_tools(&messages, std::slice::from_ref(tool))
            .context("LLM tool-call request failed")?;

        match turn {
            ToolTurn::ToolCalls(calls) => {
                println!("model called {} tool(s):", calls.len());
                messages.push(ChatMessage::Assistant { content: None, tool_calls: calls.clone() });
                for call in &calls {
                    tool_call_count += 1;
                    let result = dispatch(call);
                    println!(
                        "  {}({}) -> {}",
                        call.name,
                        call.arguments,
                        if result.len() > 120 { format!("{}...", &result[..120]) } else { result.clone() }
                    );
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

    Ok((tool_call_count, final_answer))
}

fn run_read_file_scenario(client: &LlmClient) -> Result<()> {
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

    let initial_message = ChatMessage::User(format!(
        "There is a TypeScript file named \"{scratch_file}\" that you have not seen. It exports \
         one function, `calculateShippingFee`. Call the `read_file` tool to read \"{scratch_file}\" \
         and find its exact parameter list, then answer with ONLY the parameter names and types, \
         in order, comma-separated — nothing else, no explanation, no code fences."
    ));

    let (tool_call_count, final_answer) = run_tool_loop(client, initial_message, &read_file_tool, |call| {
        dispatch_read_file(call, &scratch_dir)
    })?;

    let _ = std::fs::remove_dir_all(&scratch_dir);

    println!("\n=== Report: read_file ===");
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

/// Executes one `read_file` tool call. Anything else (a hallucinated tool name) is reported
/// back as an error string, the same way a real tool registry would.
fn dispatch_read_file(call: &ToolCall, scratch_dir: &std::path::Path) -> String {
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

/// Mirrors the exact real-world bug this scenario was built to address (a dogfooding project's
/// `ProductService.ts` calling `createProduct` without importing it — see the 2026-07-10 session
/// notes) with fully generic naming: `conjureWidget` is deliberately not the conventional
/// `create<Entity>` shape, so a correct answer can't come from guessing the naming convention —
/// only from actually resolving the symbol via the tool.
fn run_symbol_lookup_scenario(client: &LlmClient) -> Result<()> {
    let scratch_dir = std::env::temp_dir().join(format!("canopy-try-tools-symbol-{}", std::process::id()));
    let models_dir = scratch_dir.join("models");
    let services_dir = scratch_dir.join("services");
    std::fs::create_dir_all(&models_dir).context("failed to create scratch models dir")?;
    std::fs::create_dir_all(&services_dir).context("failed to create scratch services dir")?;

    std::fs::write(scratch_dir.join("package.json"), "{\"name\": \"scratch\", \"version\": \"0.0.0\"}\n")
        .context("failed to write scratch package.json")?;
    std::fs::write(
        models_dir.join("Widget.ts"),
        "export interface Widget {\n  size: number;\n  color: string;\n}\n\nexport function conjureWidget(size: number, color: string): Widget {\n  return { size, color };\n}\n",
    ).context("failed to write scratch Widget.ts")?;
    let widget_service_content = "import { Widget } from '../models/Widget';\n\nexport class WidgetService {\n  build(size: number, color: string): Widget {\n    return conjureWidget(size, color);\n  }\n}\n";
    std::fs::write(services_dir.join("WidgetService.ts"), widget_service_content)
        .context("failed to write scratch WidgetService.ts")?;

    println!("Scratch project: {}", scratch_dir.display());
    println!("(indexing with the real `roots` binary, then querying the real index — not a mock)\n");

    let init_status = std::process::Command::new("roots")
        .arg("init")
        .current_dir(&scratch_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let index_status = std::process::Command::new("roots")
        .arg("index")
        .arg(".")
        .current_dir(&scratch_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if !matches!(init_status, Ok(s) if s.success()) || !matches!(index_status, Ok(s) if s.success()) {
        let _ = std::fs::remove_dir_all(&scratch_dir);
        println!("verdict:           SKIPPED — `roots init`/`roots index` failed or the `roots` binary isn't on PATH.");
        return Ok(());
    }

    let store = Store::open(&scratch_dir.join(".roots/index.db")).context("failed to open scratch Roots index")?;

    let find_symbol_tool = ToolSpec {
        name: "find_symbol".to_string(),
        description: "Look up where a symbol (function, class, interface) is defined in this project by its exact name. Returns its kind, defining file, and line — or reports that it wasn't found.".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "The exact symbol name to look up, e.g. \"conjureWidget\""}
            },
            "required": ["name"]
        }),
    };

    let initial_message = ChatMessage::User(format!(
        "The TypeScript file \"services/WidgetService.ts\" fails to compile:\n\n\
         error TS2304: Cannot find name 'conjureWidget'.\n\n\
         Current file content:\n{widget_service_content}\n\n\
         Call the `find_symbol` tool to find out which file exports `conjureWidget`, then answer \
         with ONLY the corrected import line that needs to be added to this file — nothing else, \
         no explanation, no code fences, no other lines."
    ));

    let (tool_call_count, final_answer) = run_tool_loop(client, initial_message, &find_symbol_tool, |call| {
        dispatch_find_symbol(call, &store)
    })?;

    let _ = std::fs::remove_dir_all(&scratch_dir);

    println!("\n=== Report: find_symbol ===");
    println!("tool calls made:   {tool_call_count}");
    match &final_answer {
        Some(answer) => {
            println!("final answer:      {answer}");
            let mentions_symbol = answer.contains("conjureWidget");
            let mentions_path = answer.contains("models/Widget");
            println!("expected:          an import of `conjureWidget` from `../models/Widget`");
            println!("symbol present:    {mentions_symbol}");
            println!("path present:      {mentions_path}");
            if tool_call_count == 0 {
                println!("verdict:           DID NOT call the tool — answer is a guess, not evidence of tool use.");
            } else if mentions_symbol && mentions_path {
                println!("verdict:           called the tool AND produced a correct import.");
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

/// Executes one `find_symbol` tool call against the scratch project's real Roots index.
fn dispatch_find_symbol(call: &ToolCall, store: &Store) -> String {
    match call.name.as_str() {
        "find_symbol" => {
            let Some(name) = call.arguments.get("name").and_then(|v| v.as_str()) else {
                return "error: missing required \"name\" argument".to_string();
            };
            match store.query_exact("default", name) {
                Ok(rows) if !rows.is_empty() => rows
                    .iter()
                    .map(|r| format!("{} {} — defined in {} (line {})", r.kind, r.name, r.file, r.line))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Ok(_) => format!("no symbol named \"{name}\" found in the index"),
                Err(e) => format!("error querying index: {e}"),
            }
        }
        other => format!("error: unknown tool \"{other}\""),
    }
}
