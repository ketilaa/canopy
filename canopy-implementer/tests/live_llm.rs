//! Talks to a real llama-server. Skipped unless `CANOPY_IMPLEMENTER_LLM_TEST=1`, so a plain
//! `cargo test` never needs a model.
//!
//!   CANOPY_IMPLEMENTER_LLM_TEST=1 cargo test -p canopy-implementer --test live_llm -- --nocapture
//!
//! Override the endpoint and model with `CANOPY_IMPLEMENTER_LLM_URL` and `CANOPY_IMPLEMENTER_LLM_MODEL`.

use std::sync::Arc;
use std::time::Duration;

use canopy_implementer::files::GeneratedFiles;
use canopy_implementer::llm::{LlmClient, Message, ModelEndpoint, Sampling, StructuredRequest};
use canopy_implementer::observe::{Observers, RecordingObserver, RunEvent, TraceWriter};

#[test]
fn a_real_schema_constrained_call_is_fully_observed() {
    if std::env::var("CANOPY_IMPLEMENTER_LLM_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set CANOPY_IMPLEMENTER_LLM_TEST=1 to run against a live server");
        return;
    }
    let url = std::env::var("CANOPY_IMPLEMENTER_LLM_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    let model = std::env::var("CANOPY_IMPLEMENTER_LLM_MODEL").unwrap_or_else(|_| "Qwen/Qwen3-14B-GGUF:Q4_K_M".into());

    let dir = tempfile::tempdir().unwrap();
    let trace_path = dir.path().join("events.jsonl");
    let recorder = Arc::new(RecordingObserver::new());
    let observers = Observers::new()
        .with(recorder.clone())
        .with(Arc::new(TraceWriter::create(&trace_path, "live-smoke").unwrap()));
    let client = LlmClient::new(
        ModelEndpoint {
            base_url: url,
            model,
            timeout: Duration::from_secs(300),
        },
        Sampling::qwen3(),
        Arc::new(observers),
    );

    let response = client
        .complete_structured(&StructuredRequest {
            purpose: "live-smoke".into(),
            messages: vec![Message::user(
                "Create a Java class `Greeter` in package `com.example.demo` with a method \
                 `String greet(String name)` returning \"Hello, <name>!\". Return the file.",
            )],
            schema_name: "files".into(),
            schema: GeneratedFiles::schema(),
            thinking: false,
        })
        .expect("live call");

    let files = GeneratedFiles::parse(&response.content).expect("the schema-constrained answer must parse");
    assert!(!files.files.is_empty());

    let events = recorder.events();
    assert!(matches!(events[0], RunEvent::LlmRequestSent(_)));
    let RunEvent::LlmResponseReceived(got) = &events[1] else {
        panic!("{:?}", events[1])
    };
    assert!(got.prompt_tokens > 0 && got.completion_tokens > 0 && got.elapsed_ms > 0);
    assert!(got.server.is_some(), "llama-server reports timings");

    eprintln!(
        "--- trace ({} lines) ---",
        std::fs::read_to_string(&trace_path).unwrap().lines().count()
    );
    for line in std::fs::read_to_string(&trace_path).unwrap().lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let e = &v["event"];
        match e["type"].as_str().unwrap() {
            "llm_request_sent" => eprintln!(
                "seq {} {} request  call={} purpose={}",
                v["seq"], v["ts"], e["call_id"], e["purpose"]
            ),
            "llm_response_received" => eprintln!(
                "seq {} {} response call={} finish={} tokens in/out={}/{} cached={} elapsed={}ms server={}",
                v["seq"],
                v["ts"],
                e["call_id"],
                e["finish_reason"],
                e["prompt_tokens"],
                e["completion_tokens"],
                e["cached_tokens"],
                e["elapsed_ms"],
                e["server"]
            ),
            other => eprintln!("seq {} {other}", v["seq"]),
        }
    }
}
