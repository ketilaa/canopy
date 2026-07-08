use canopy_core::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("ANTHROPIC_API_KEY environment variable is not set. Export it before running canopy.")]
    MissingApiKey,
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("Failed to parse JSON from LLM response: {0}")]
    JsonParse(String),
    #[error("Failed to parse YAML from LLM response: {source}\nRaw LLM output:\n{raw}")]
    YamlParse {
        #[source]
        source: serde_yaml::Error,
        raw: String,
    },
    #[error("Unexpected LLM response shape: {0}")]
    UnexpectedShape(String),
}

pub struct LlmClient {
    api_key: String,
    model: String,
    debug: bool,
    provider: LlmProvider,
    base_url: String,
    log_path: Option<std::path::PathBuf>,
}

impl LlmClient {
    pub fn default_local(debug: bool) -> Self {
        Self {
            api_key: String::new(),
            model: "qwen2.5:32b".to_string(),
            debug,
            provider: LlmProvider::Ollama,
            base_url: "http://localhost:11434".to_string(),
            log_path: None,
        }
    }

    pub fn from_agent_config(cfg: &AgentLlmConfig, debug: bool) -> Self {
        let base_url = cfg.base_url.clone().unwrap_or_else(|| match cfg.provider {
            LlmProvider::Anthropic => "https://api.anthropic.com".to_string(),
            LlmProvider::Ollama => "http://localhost:11434".to_string(),
        });
        let api_key = match cfg.provider {
            LlmProvider::Anthropic => std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            LlmProvider::Ollama => String::new(),
        };
        Self {
            api_key,
            model: cfg.model.clone(),
            debug,
            provider: cfg.provider.clone(),
            base_url,
            log_path: None,
        }
    }

    pub fn with_log_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.log_path = Some(path.into());
        self
    }

    pub fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        self.complete_with_max_tokens(prompt, 4096)
    }

    /// Use for code generation where output can be significantly larger than planning artifacts.
    pub fn complete_large(&self, prompt: &str) -> Result<String, LlmError> {
        self.complete_with_max_tokens(prompt, 8192)
    }

    fn complete_with_max_tokens(&self, prompt: &str, max_tokens: u32) -> Result<String, LlmError> {
        if self.debug {
            eprintln!("\n╔══ LLM INPUT ═══════════════════════════════════════════╗");
            eprintln!("{prompt}");
            eprintln!("╚════════════════════════════════════════════════════════╝\n");
        }

        let (text, json) = match self.provider {
            LlmProvider::Anthropic => self.call_anthropic(prompt, max_tokens)?,
            LlmProvider::Ollama => self.call_openai_compatible(prompt)?,
        };

        let model = json["model"].as_str().unwrap_or(&self.model);
        let input_tokens = json["usage"]["input_tokens"]
            .as_u64()
            .or_else(|| json["usage"]["prompt_tokens"].as_u64())
            .unwrap_or(0);
        let output_tokens = json["usage"]["output_tokens"]
            .as_u64()
            .or_else(|| json["usage"]["completion_tokens"].as_u64())
            .unwrap_or(0);

        if self.debug {
            eprintln!("╔══ LLM OUTPUT ══════════════════════════════════════════╗");
            eprintln!("  model:         {model}");
            eprintln!("  input tokens:  {input_tokens}");
            eprintln!("  output tokens: {output_tokens}");
            eprintln!("──────────────────────────────────────────────────────────");
            eprintln!("{text}");
            eprintln!("╚════════════════════════════════════════════════════════╝\n");
        }

        if let Some(log_path) = &self.log_path {
            let ts = llm_timestamp();
            let entry = format!(
                "\n[{ts}] ╔══ LLM INPUT ═══════════════════════════════════════════╗\n\
                 {prompt}\n\
                 [{ts}] ╚════════════════════════════════════════════════════════╝\n\
                 [{ts}] ╔══ LLM OUTPUT ══════════════════════════════════════════╗\n\
                 [{ts}]   model:         {model}\n\
                 [{ts}]   input tokens:  {input_tokens}\n\
                 [{ts}]   output tokens: {output_tokens}\n\
                 [{ts}] ──────────────────────────────────────────────────────────\n\
                 {text}\n\
                 [{ts}] ╚════════════════════════════════════════════════════════╝\n"
            );
            if let Some(parent) = log_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
                let _ = f.write_all(entry.as_bytes());
            }
        }

        Ok(text)
    }

    fn call_anthropic(&self, prompt: &str, max_tokens: u32) -> Result<(String, serde_json::Value), LlmError> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": [{"role": "user", "content": prompt}]
        });
        let url = format!("{}/v1/messages", self.base_url);
        let response = ureq::post(&url)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| LlmError::Http(e.to_string()))?;
        let json: serde_json::Value = response
            .into_json()
            .map_err(|e| LlmError::JsonParse(e.to_string()))?;
        let text = json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| LlmError::UnexpectedShape(
                format!("expected content[0].text, got: {json}")
            ))?
            .to_string();
        Ok((text, json))
    }

    fn call_openai_compatible(&self, prompt: &str) -> Result<(String, serde_json::Value), LlmError> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}]
        });
        let url = format!("{}/v1/chat/completions", self.base_url);
        let response = ureq::post(&url)
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| LlmError::Http(e.to_string()))?;
        let json: serde_json::Value = response
            .into_json()
            .map_err(|e| LlmError::JsonParse(e.to_string()))?;
        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| LlmError::UnexpectedShape(
                format!("expected choices[0].message.content, got: {json}")
            ))?
            .to_string();
        Ok((text, json))
    }
}

/// UTC timestamp formatted as YYYY-MM-DDTHH:MM:SSZ using only std.
pub(crate) fn llm_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = (secs % 60) as u32;
    let m = ((secs / 60) % 60) as u32;
    let h = ((secs / 3600) % 24) as u32;
    // civil_from_days — Howard Hinnant's algorithm
    let z = (secs / 86400) as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = if mo <= 2 { y + 1 } else { y };
    format!("{yr:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}
