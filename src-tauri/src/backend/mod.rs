pub mod anthropic;
pub mod claude_cli;
pub mod openai;

use std::{sync::Arc, time::Duration};

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum BackendError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("backend returned an unusable response: {0}")]
    BadResponse(String),
    #[error("backend is misconfigured: {0}")]
    Misconfigured(String),
    #[error("backend timed out")]
    Timeout,
}

pub trait HttpClient: Send + Sync {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &Value,
        timeout_ms: u64,
    ) -> Result<Value, BackendError>;
}

pub trait Backend: Send + Sync {
    fn name(&self) -> &str;
    fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError>;
}

pub struct ReqwestClient;

impl HttpClient for ReqwestClient {
    fn post_json(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: &Value,
        timeout_ms: u64,
    ) -> Result<Value, BackendError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| BackendError::Transport(e.to_string()))?;

        let mut request = client.post(url).json(body);
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }

        let response = request.send().map_err(|e| {
            if e.is_timeout() {
                BackendError::Timeout
            } else {
                BackendError::Transport(e.to_string())
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(BackendError::BadResponse(format!("HTTP {status}: {text}")));
        }

        response
            .json()
            .map_err(|e| BackendError::BadResponse(e.to_string()))
    }
}

pub fn shared_http() -> Arc<dyn HttpClient> {
    Arc::new(ReqwestClient)
}

use crate::config::{BackendConfig, KIND_ANTHROPIC, KIND_CLAUDE_CLI, KIND_OPENAI};

pub fn build(config: &BackendConfig) -> Result<Arc<dyn Backend>, BackendError> {
    match config.kind.as_str() {
        KIND_OPENAI => Ok(Arc::new(openai::OpenAiCompatible::new(config, shared_http()))),
        KIND_ANTHROPIC => Ok(Arc::new(anthropic::AnthropicApi::new(config, shared_http()))),
        KIND_CLAUDE_CLI => Ok(Arc::new(claude_cli::ClaudeCli::new(
            config,
            Arc::new(claude_cli::ProcessRunner),
        ))),
        other => Err(BackendError::Misconfigured(format!("unknown kind: {other}"))),
    }
}
