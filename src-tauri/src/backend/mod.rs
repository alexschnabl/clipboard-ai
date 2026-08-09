pub mod anthropic;
pub mod claude_cli;
pub mod openai;

use std::{
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

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

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = (|| {
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
            })();
            let _ = tx.send(result);
        });

        rx.recv_timeout(Duration::from_millis(timeout_ms))
            .unwrap_or(Err(BackendError::Timeout))
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

#[cfg(test)]
mod build_tests {
    use super::*;
    use crate::config::{BackendConfig, Effort, KIND_ANTHROPIC, KIND_CLAUDE_CLI, KIND_OPENAI};

    fn cfg(kind: &str) -> BackendConfig {
        BackendConfig {
            id: "b".into(),
            name: "B".into(),
            kind: kind.into(),
            base_url: Some("http://localhost/v1".into()),
            model: Some("m".into()),
            api_key: None,
            timeout_ms: 120_000,
            effort: Effort::Low,
        }
    }

    #[test]
    fn builds_the_matching_backend_for_every_shipped_kind() {
        for kind in [KIND_OPENAI, KIND_ANTHROPIC, KIND_CLAUDE_CLI] {
            let backend = build(&cfg(kind)).unwrap_or_else(|e| panic!("{kind}: {e}"));
            assert_eq!(backend.name(), "B", "kind: {kind}");
        }
    }

    #[test]
    fn an_unknown_kind_is_misconfigured() {
        assert!(matches!(build(&cfg("telepathy")), Err(BackendError::Misconfigured(_))));
    }
}

#[cfg(test)]
mod reqwest_client_tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::Instant,
    };

    use super::*;

    #[test]
    fn a_slow_body_does_not_get_a_second_full_timeout_budget() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                let _ = stream.read(&mut buf);

                thread::sleep(Duration::from_millis(300));
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n",
                );
                let _ = stream.flush();

                thread::sleep(Duration::from_secs(5));
            }
        });

        let timeout_ms = 500;
        let start = Instant::now();
        let result =
            ReqwestClient.post_json(&format!("http://{addr}"), &[], &serde_json::json!({}), timeout_ms);
        let elapsed = start.elapsed();

        assert!(matches!(result, Err(BackendError::Timeout)), "expected Timeout, got {result:?}");
        assert!(
            elapsed < Duration::from_millis(1500),
            "a single {timeout_ms}ms deadline must bound headers+body together, not double it; took {elapsed:?}"
        );
    }
}
