use std::sync::Arc;

use serde_json::json;

use crate::backend::{Backend, BackendError, HttpClient};
use crate::config::BackendConfig;

/// Talks the OpenAI Chat Completions wire format -- `POST
/// {baseUrl}/chat/completions` with a `messages` array and a `choices[0]`
/// response -- which is also what every "OpenAI-compatible" local server
/// (Ollama, LM Studio, llama.cpp's server mode, ...) implements. That's the
/// entire reason this backend kind exists as one implementation rather than
/// one per local-model server: they all speak this same shape.
pub struct OpenAiCompatible {
    name: String,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    timeout_ms: u64,
    http: Arc<dyn HttpClient>,
}

impl OpenAiCompatible {
    pub fn new(config: &BackendConfig, http: Arc<dyn HttpClient>) -> Self {
        Self {
            name: config.name.clone(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            api_key: config.api_key.clone(),
            timeout_ms: config.timeout_ms,
            http,
        }
    }
}

impl Backend for OpenAiCompatible {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError> {
        // `filter` before `ok_or_else`: a blank string is as unusable as an
        // absent one, and hand-editing config.json can produce either. The
        // settings UI already trims empties to null, so this covers the
        // hand-edited path the UI cannot.
        let base = self
            .base_url
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| BackendError::Misconfigured("baseUrl is required".into()))?;
        let url = format!("{}/chat/completions", base.trim_end_matches('/'));

        // Matches the `base_url` check above: a missing model must fail
        // clearly here, not ship as `"model": ""` and fail opaquely once it
        // reaches the remote server (an unrecognized-model HTTP error, or a
        // provider that silently falls back to whatever it treats as
        // "default").
        let model = self
            .model
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| BackendError::Misconfigured("model is required".into()))?;

        // Unlike `anthropic.rs`'s `x-api-key`, this is a bearer token in the
        // standard `Authorization` header -- the OpenAI convention every
        // compatible server also follows. Sent only when configured: a
        // local server (Ollama, ...) typically has no auth at all, and
        // sending an empty/absent key as a header some servers would still
        // parse is worse than omitting the header entirely.
        let mut headers = Vec::new();
        if let Some(key) = &self.api_key {
            headers.push(("Authorization".to_string(), format!("Bearer {key}")));
        }

        // `"stream": false` is not the wire format's default -- omitting it
        // would get an SSE chunk stream back instead of one JSON object, and
        // `post_json`'s `HttpClient` contract only ever returns a single
        // parsed `Value`, not an event stream.
        let body = json!({
            "model": model,
            "stream": false,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": text},
            ],
        });

        let response = self.http.post_json(&url, &headers, &body, self.timeout_ms)?;

        // `choices[0]` only: with `"n"` left unset (its default is 1) and no
        // multi-completion feature this app exposes, a compliant server
        // never returns more than one choice anyway. The whole chain is one
        // `.and_then()` collapsing every way a compliant-looking response
        // can still fail to carry usable text -- an empty `choices` array,
        // a `null` `message` (seen from at least one real Ollama build), a
        // `null` `content` -- into the same `BadResponse` rather than a
        // `.unwrap()`/index panic on a field a malformed or unusual server
        // response left out.
        let content = response
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| {
                BackendError::BadResponse(format!("no assistant content in {response}"))
            })?;

        Ok(content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::backend::{BackendError, HttpClient};
    use crate::config::{BackendConfig, KIND_OPENAI};

    struct FakeHttp {
        response: Result<serde_json::Value, String>,
        seen: Mutex<Option<(String, serde_json::Value)>>,
    }

    impl FakeHttp {
        fn ok(body: serde_json::Value) -> Arc<Self> {
            Arc::new(Self { response: Ok(body), seen: Mutex::new(None) })
        }
        fn err(message: &str) -> Arc<Self> {
            Arc::new(Self { response: Err(message.into()), seen: Mutex::new(None) })
        }
    }

    impl HttpClient for FakeHttp {
        fn post_json(
            &self,
            url: &str,
            _headers: &[(String, String)],
            body: &serde_json::Value,
            _timeout_ms: u64,
        ) -> Result<serde_json::Value, BackendError> {
            *self.seen.lock().unwrap() = Some((url.to_string(), body.clone()));
            self.response
                .clone()
                .map_err(|m| BackendError::Transport(m))
        }
    }

    fn cfg() -> BackendConfig {
        BackendConfig {
            id: "ollama".into(),
            name: "Local".into(),
            kind: KIND_OPENAI.into(),
            base_url: Some("http://localhost:11434/v1".into()),
            model: Some("qwen2.5:32b-instruct".into()),
            api_key: None,
            timeout_ms: 120_000,
        }
    }

    fn chat(content: &str) -> serde_json::Value {
        json!({"choices":[{"message":{"role":"assistant","content":content}}]})
    }

    #[test]
    fn returns_the_message_content() {
        let http = FakeHttp::ok(chat("Corrected."));
        let backend = OpenAiCompatible::new(&cfg(), http);
        assert_eq!(backend.transform("Fix it.", "corected").unwrap(), "Corrected.");
    }

    #[test]
    fn posts_to_the_chat_completions_url_with_both_messages() {
        let http = FakeHttp::ok(chat("ok"));
        let backend = OpenAiCompatible::new(&cfg(), http.clone());
        backend.transform("Fix it.", "input text").unwrap();
        let (url, body) = http.seen.lock().unwrap().clone().unwrap();
        assert_eq!(url, "http://localhost:11434/v1/chat/completions");
        assert_eq!(body["model"], "qwen2.5:32b-instruct");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "Fix it.");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "input text");
    }

    #[test]
    fn trailing_slash_on_base_url_does_not_double_up() {
        let mut config = cfg();
        config.base_url = Some("http://localhost:11434/v1/".into());
        let http = FakeHttp::ok(chat("ok"));
        let backend = OpenAiCompatible::new(&config, http.clone());
        backend.transform("s", "t").unwrap();
        let (url, _) = http.seen.lock().unwrap().clone().unwrap();
        assert_eq!(url, "http://localhost:11434/v1/chat/completions");
    }

    #[test]
    fn transport_failure_becomes_a_backend_error() {
        let backend = OpenAiCompatible::new(&cfg(), FakeHttp::err("connection refused"));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Transport(_))));
    }

    // Same code path (`transform`'s `.and_then()` chain over
    // `choices[0].message.content`), exercised with a different link of
    // that chain missing/null each time -- all three must fail cleanly as
    // a `BadResponse`, never panic.
    #[test]
    fn malformed_choices_are_a_backend_error_not_a_panic() {
        for body in [
            json!({"choices": []}),
            json!({"choices":[{"message": null}]}),
            json!({"choices":[{"message":{"content": null}}]}),
        ] {
            let backend = OpenAiCompatible::new(&cfg(), FakeHttp::ok(body.clone()));
            assert!(
                matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))),
                "body: {body}"
            );
        }
    }

    #[test]
    fn missing_base_url_is_a_backend_error() {
        let mut config = cfg();
        config.base_url = None;
        let backend = OpenAiCompatible::new(&config, FakeHttp::ok(chat("ok")));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Misconfigured(_))));
    }

    // Only hand-editing config.json can produce these -- the settings UI
    // trims empty fields to null -- but a blank value is exactly as unusable
    // as an absent one and must fail the same way, not ship to the server.
    #[test]
    fn a_blank_base_url_is_treated_as_missing() {
        let mut config = cfg();
        config.base_url = Some("   ".into());
        let backend = OpenAiCompatible::new(&config, FakeHttp::ok(chat("ok")));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Misconfigured(_))));
    }

    #[test]
    fn a_blank_model_is_treated_as_missing() {
        let mut config = cfg();
        config.model = Some("".into());
        let backend = OpenAiCompatible::new(&config, FakeHttp::ok(chat("ok")));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Misconfigured(_))));
    }

    // A missing model must fail the same clear way a missing base_url does,
    // rather than silently shipping `"model": ""` to the remote server.
    #[test]
    fn missing_model_is_a_backend_error() {
        let mut config = cfg();
        config.model = None;
        let backend = OpenAiCompatible::new(&config, FakeHttp::ok(chat("ok")));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Misconfigured(_))));
    }
}
