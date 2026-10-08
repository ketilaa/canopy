//! Schema-constrained completions against an OpenAI-compatible server (llama-server).
//!
//! Every call is observed: the full request, then either the full response with its token counts
//! and timings, or a classified failure. The client keeps no conversation between calls; the
//! caller builds each prompt from scratch.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::observe::{
    FailureKind, LlmCallFailed, LlmRequestSent, LlmResponseReceived, RunEvent, RunObserver, ServerTimings,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

/// Sent explicitly on every request, so results never depend on how the server was launched.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sampling {
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: u32,
    pub min_p: f64,
    pub max_tokens: u32,
}

impl Sampling {
    /// Qwen's recommended settings (thinking mode defaults), with a completion cap.
    pub fn qwen3() -> Self {
        Self {
            temperature: 0.6,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            max_tokens: 2048,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelEndpoint {
    pub base_url: String,
    pub model: String,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct StructuredRequest {
    /// Why this call is made ("generate-files", "retry-1"), recorded with every event.
    pub purpose: String,
    pub messages: Vec<Message>,
    pub schema_name: String,
    pub schema: Value,
    /// Whether the model may think before answering. Off by default for schema-constrained work.
    pub thinking: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmResponse {
    pub call_id: u64,
    pub content: String,
    pub finish_reason: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[error("LLM call {call_id} failed ({kind:?}): {message}")]
pub struct LlmError {
    pub call_id: u64,
    pub kind: FailureKind,
    pub status: Option<u16>,
    pub message: String,
}

/// Anything that can answer a schema-constrained request. The real implementation is [`LlmClient`];
/// tests script one.
pub trait Generator: Send + Sync {
    fn complete_structured(&self, request: &StructuredRequest) -> Result<LlmResponse, LlmError>;
}

pub struct LlmClient {
    endpoint: ModelEndpoint,
    sampling: Sampling,
    observer: Arc<dyn RunObserver>,
    next_call_id: AtomicU64,
}

impl LlmClient {
    pub fn new(endpoint: ModelEndpoint, sampling: Sampling, observer: Arc<dyn RunObserver>) -> Self {
        Self {
            endpoint,
            sampling,
            observer,
            next_call_id: AtomicU64::new(1),
        }
    }

    pub fn complete_structured(&self, request: &StructuredRequest) -> Result<LlmResponse, LlmError> {
        let call_id = self.next_call_id.fetch_add(1, Ordering::Relaxed);
        self.observer.on(&RunEvent::LlmRequestSent(LlmRequestSent {
            call_id,
            purpose: request.purpose.clone(),
            model: self.endpoint.model.clone(),
            messages: request.messages.clone(),
            schema_name: request.schema_name.clone(),
            schema: request.schema.clone(),
            sampling: self.sampling.clone(),
            thinking: request.thinking,
        }));
        let started = Instant::now();
        let fail = |kind: FailureKind, status: Option<u16>, message: String, body: Option<String>| {
            self.observer.on(&RunEvent::LlmCallFailed(LlmCallFailed {
                call_id,
                purpose: request.purpose.clone(),
                kind,
                status,
                message: message.clone(),
                body,
                elapsed_ms: started.elapsed().as_millis() as u64,
            }));
            LlmError {
                call_id,
                kind,
                status,
                message,
            }
        };

        let url = format!("{}/v1/chat/completions", self.endpoint.base_url);
        let agent = ureq::AgentBuilder::new().timeout(self.endpoint.timeout).build();
        let sent = agent
            .post(&url)
            .set("content-type", "application/json")
            .send_json(self.request_body(request));
        let text = match sent {
            Ok(response) => response.into_string().map_err(|e| {
                let kind = if is_timeout_io(&e) {
                    FailureKind::Timeout
                } else {
                    FailureKind::Transport
                };
                fail(kind, None, e.to_string(), None)
            })?,
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                return Err(fail(
                    FailureKind::HttpStatus,
                    Some(code),
                    format!("HTTP {code}"),
                    Some(body),
                ));
            }
            Err(ureq::Error::Transport(transport)) => {
                let kind = if is_timeout_transport(&transport) {
                    FailureKind::Timeout
                } else {
                    FailureKind::Transport
                };
                return Err(fail(kind, None, transport.to_string(), None));
            }
        };

        let Some(completion) = Completion::parse(&text) else {
            return Err(fail(
                FailureKind::BadShape,
                None,
                "response has no choices[0].message.content".into(),
                Some(text),
            ));
        };
        self.observer.on(&RunEvent::LlmResponseReceived(LlmResponseReceived {
            call_id,
            purpose: request.purpose.clone(),
            model: completion.model.clone().unwrap_or_else(|| self.endpoint.model.clone()),
            finish_reason: completion.finish_reason.clone(),
            content: completion.content.clone(),
            prompt_tokens: completion.prompt_tokens,
            completion_tokens: completion.completion_tokens,
            cached_tokens: completion.cached_tokens,
            elapsed_ms: started.elapsed().as_millis() as u64,
            server: completion.server,
        }));
        Ok(LlmResponse {
            call_id,
            content: completion.content,
            finish_reason: completion.finish_reason,
            prompt_tokens: completion.prompt_tokens,
            completion_tokens: completion.completion_tokens,
        })
    }

    fn request_body(&self, request: &StructuredRequest) -> Value {
        json!({
            "model": self.endpoint.model,
            "messages": request.messages,
            "temperature": self.sampling.temperature,
            "top_p": self.sampling.top_p,
            "top_k": self.sampling.top_k,
            "min_p": self.sampling.min_p,
            "max_tokens": self.sampling.max_tokens,
            "chat_template_kwargs": { "enable_thinking": request.thinking },
            "response_format": {
                "type": "json_schema",
                "json_schema": { "name": request.schema_name, "strict": true, "schema": request.schema }
            }
        })
    }
}

impl Generator for LlmClient {
    fn complete_structured(&self, request: &StructuredRequest) -> Result<LlmResponse, LlmError> {
        LlmClient::complete_structured(self, request)
    }
}

/// The parts of a completion response the harness records.
struct Completion {
    model: Option<String>,
    content: String,
    finish_reason: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    cached_tokens: Option<u64>,
    server: Option<ServerTimings>,
}

impl Completion {
    fn parse(text: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(text).ok()?;
        let choice = v.get("choices")?.get(0)?;
        let content = choice.get("message")?.get("content")?.as_str()?.to_string();
        let tokens = |key: &str| v["usage"][key].as_u64().unwrap_or(0);
        let timings = &v["timings"];
        let server = (|| {
            Some(ServerTimings {
                prompt_ms: timings.get("prompt_ms")?.as_f64()?,
                predicted_ms: timings.get("predicted_ms")?.as_f64()?,
                prompt_per_second: timings.get("prompt_per_second")?.as_f64()?,
                predicted_per_second: timings.get("predicted_per_second")?.as_f64()?,
            })
        })();
        Some(Self {
            model: v["model"].as_str().map(str::to_string),
            content,
            finish_reason: choice["finish_reason"].as_str().unwrap_or("unknown").to_string(),
            prompt_tokens: tokens("prompt_tokens"),
            completion_tokens: tokens("completion_tokens"),
            cached_tokens: v["usage"]["prompt_tokens_details"]["cached_tokens"].as_u64(),
            server,
        })
    }
}

fn is_timeout_io(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock)
        || e.to_string().contains("timed out")
}

fn is_timeout_transport(t: &ureq::Transport) -> bool {
    use std::error::Error;
    let mut source: Option<&(dyn Error + 'static)> = t.source();
    while let Some(err) = source {
        if let Some(io) = err.downcast_ref::<std::io::Error>() {
            if is_timeout_io(io) {
                return true;
            }
        }
        source = err.source();
    }
    t.to_string().contains("timed out")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe::{RecordingObserver, RunEvent};
    use crate::testing::FakeServer;

    const OK_BODY: &str = r#"{
      "model": "Qwen/Qwen3-14B-GGUF:Q4_K_M",
      "choices": [{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"{\"files\":[]}"}}],
      "usage": {"prompt_tokens": 74, "completion_tokens": 185, "total_tokens": 259,
                "prompt_tokens_details": {"cached_tokens": 12}},
      "timings": {"cache_n": 12, "prompt_n": 62, "prompt_ms": 919.6, "prompt_per_second": 80.4,
                  "predicted_n": 185, "predicted_ms": 15648.1, "predicted_per_second": 11.75}
    }"#;

    fn request(purpose: &str) -> StructuredRequest {
        StructuredRequest {
            purpose: purpose.into(),
            messages: vec![Message::system("be brief"), Message::user("make files")],
            schema_name: "files".into(),
            schema: serde_json::json!({"type": "object"}),
            thinking: false,
        }
    }

    fn client(url: &str, timeout_ms: u64) -> (LlmClient, Arc<RecordingObserver>) {
        let observer = Arc::new(RecordingObserver::new());
        let client = LlmClient::new(
            ModelEndpoint {
                base_url: url.into(),
                model: "qwen3-14b".into(),
                timeout: Duration::from_millis(timeout_ms),
            },
            Sampling::qwen3(),
            observer.clone(),
        );
        (client, observer)
    }

    #[test]
    fn success_returns_content_and_token_counts() {
        let server = FakeServer::respond(200, OK_BODY, Duration::ZERO);
        let (client, _) = client(&server.url, 5000);
        let response = client.complete_structured(&request("generate-files")).unwrap();
        assert_eq!(response.content, "{\"files\":[]}");
        assert_eq!(response.finish_reason, "stop");
        assert_eq!((response.prompt_tokens, response.completion_tokens), (74, 185));
    }

    #[test]
    fn success_emits_request_then_response_with_everything_needed_for_analysis() {
        let server = FakeServer::respond(200, OK_BODY, Duration::from_millis(60));
        let (client, observer) = client(&server.url, 5000);
        client.complete_structured(&request("generate-files")).unwrap();

        let events = observer.events();
        assert_eq!(events.len(), 2);
        let RunEvent::LlmRequestSent(sent) = &events[0] else {
            panic!("first event: {:?}", events[0])
        };
        assert_eq!(
            (sent.call_id, sent.purpose.as_str(), sent.model.as_str()),
            (1, "generate-files", "qwen3-14b")
        );
        assert_eq!(sent.messages.len(), 2);
        assert_eq!(sent.schema_name, "files");
        assert_eq!(sent.sampling, Sampling::qwen3());
        assert!(!sent.thinking);

        let RunEvent::LlmResponseReceived(got) = &events[1] else {
            panic!("second event: {:?}", events[1])
        };
        assert_eq!((got.call_id, got.purpose.as_str()), (1, "generate-files"));
        assert_eq!(got.model, "Qwen/Qwen3-14B-GGUF:Q4_K_M");
        assert_eq!(got.finish_reason, "stop");
        assert_eq!(got.content, "{\"files\":[]}");
        assert_eq!(
            (got.prompt_tokens, got.completion_tokens, got.cached_tokens),
            (74, 185, Some(12))
        );
        assert!(got.elapsed_ms >= 60, "elapsed {}", got.elapsed_ms);
        let server_timings = got.server.as_ref().unwrap();
        assert_eq!(server_timings.predicted_ms, 15648.1);
        assert_eq!(server_timings.predicted_per_second, 11.75);
        assert_eq!(server_timings.prompt_per_second, 80.4);
    }

    #[test]
    fn the_request_asks_for_the_schema_no_thinking_and_explicit_sampling() {
        let server = FakeServer::respond(200, OK_BODY, Duration::ZERO);
        let (client, _) = client(&server.url, 5000);
        client.complete_structured(&request("p")).unwrap();

        let body: Value = serde_json::from_str(&server.requests.lock().unwrap()[0]).unwrap();
        assert_eq!(body["model"], "qwen3-14b");
        assert_eq!(
            body["messages"][1],
            serde_json::json!({"role": "user", "content": "make files"})
        );
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["response_format"]["json_schema"]["name"], "files");
        assert_eq!(
            body["response_format"]["json_schema"]["schema"],
            serde_json::json!({"type": "object"})
        );
        assert_eq!(body["chat_template_kwargs"]["enable_thinking"], false);
        assert_eq!(body["temperature"], 0.6);
        assert_eq!(body["top_p"], 0.95);
        assert_eq!(body["top_k"], 20);
        assert_eq!(body["min_p"], 0.0);
        assert_eq!(body["max_tokens"], 2048);
    }

    #[test]
    fn call_ids_increase_per_call() {
        let server = FakeServer::respond(200, OK_BODY, Duration::ZERO);
        let (client, observer) = client(&server.url, 5000);
        let first = client.complete_structured(&request("a")).unwrap();
        let second = client.complete_structured(&request("b")).unwrap();
        assert_eq!((first.call_id, second.call_id), (1, 2));
        assert_eq!(observer.events().len(), 4);
    }

    #[test]
    fn http_error_is_reported_with_status_and_body_and_no_response_event() {
        let server = FakeServer::respond(500, r#"{"error":"boom"}"#, Duration::ZERO);
        let (client, observer) = client(&server.url, 5000);
        let err = client.complete_structured(&request("p")).unwrap_err();
        assert_eq!((err.kind, err.status), (FailureKind::HttpStatus, Some(500)));

        let events = observer.events();
        assert_eq!(events.len(), 2);
        let RunEvent::LlmCallFailed(failed) = &events[1] else {
            panic!("{:?}", events[1])
        };
        assert_eq!((failed.kind, failed.status), (FailureKind::HttpStatus, Some(500)));
        assert_eq!(failed.body.as_deref(), Some(r#"{"error":"boom"}"#));
        assert_eq!((failed.call_id, failed.purpose.as_str()), (1, "p"));
    }

    #[test]
    fn an_unusable_body_is_a_bad_shape_failure_that_keeps_the_raw_body() {
        let server = FakeServer::respond(200, r#"{"choices":[]}"#, Duration::ZERO);
        let (client, observer) = client(&server.url, 5000);
        let err = client.complete_structured(&request("p")).unwrap_err();
        assert_eq!(err.kind, FailureKind::BadShape);
        let RunEvent::LlmCallFailed(failed) = &observer.events()[1] else {
            panic!()
        };
        assert_eq!(failed.body.as_deref(), Some(r#"{"choices":[]}"#));
    }

    #[test]
    fn a_slow_server_is_a_timeout_failure_with_the_time_spent() {
        let server = FakeServer::respond(200, OK_BODY, Duration::from_millis(1500));
        let (client, observer) = client(&server.url, 200);
        let err = client.complete_structured(&request("p")).unwrap_err();
        assert_eq!(err.kind, FailureKind::Timeout);
        let RunEvent::LlmCallFailed(failed) = &observer.events()[1] else {
            panic!()
        };
        assert!(
            failed.elapsed_ms >= 150 && failed.elapsed_ms < 1400,
            "elapsed {}",
            failed.elapsed_ms
        );
    }

    #[test]
    fn an_unreachable_server_is_a_transport_failure() {
        let (client, observer) = client("http://127.0.0.1:1", 2000);
        let err = client.complete_structured(&request("p")).unwrap_err();
        assert_eq!(err.kind, FailureKind::Transport);
        assert_eq!(observer.events().len(), 2);
    }
}
