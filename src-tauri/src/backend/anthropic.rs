use std::sync::Arc;

use serde_json::json;

use crate::backend::{Backend, BackendError, HttpClient};
use crate::config::BackendConfig;

const DEFAULT_MODEL: &str = "claude-opus-5";
const API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Talks the Anthropic Messages API directly (`POST /v1/messages`), as
/// distinct from `claude_cli.rs`'s `ClaudeCli`, which shells out to the
/// `claude` CLI instead -- this backend is for a user's own API key rather
/// than a Claude subscription's local CLI auth. `model` defaults to a named
/// constant rather than erroring on a missing value (contrast
/// `openai.rs`'s `OpenAiCompatible`, which treats a missing `model` as
/// `Misconfigured`): unlike an arbitrary local server's model name, there
/// is one sane default Anthropic model to fall back to, so defaulting is
/// strictly more useful here than forcing the user to fill it in.
pub struct AnthropicApi {
    name: String,
    model: String,
    api_key: Option<String>,
    timeout_ms: u64,
    http: Arc<dyn HttpClient>,
}

impl AnthropicApi {
    pub fn new(config: &BackendConfig, http: Arc<dyn HttpClient>) -> Self {
        Self {
            name: config.name.clone(),
            // A blank model falls back to the default rather than shipping
            // `"model": ""`, matching how a missing one is treated -- the two
            // are indistinguishable in intent, and only hand-editing
            // config.json can produce the blank.
            model: config
                .model
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            api_key: config.api_key.clone(),
            timeout_ms: config.timeout_ms,
            http,
        }
    }
}

impl Backend for AnthropicApi {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError> {
        let key = self
            .api_key
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| BackendError::Misconfigured("apiKey is required".into()))?;

        // Anthropic's auth header is its own `x-api-key`, not the
        // `Authorization: Bearer` shape `openai.rs` uses -- these are two
        // genuinely different wire conventions, not an inconsistency to fix.
        // `anthropic-version` is a required, literal API-version header (not
        // a client library version); pinned to a fixed date rather than
        // "latest" so a future Anthropic API revision can't silently change
        // this backend's response shape out from under `transform` below.
        let headers = vec![
            ("x-api-key".to_string(), key.to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ];

        let body = json!({
            "model": self.model,
            "max_tokens": 16000,
            "system": system_prompt,
            "messages": [{"role": "user", "content": text}],
        });

        let response = self.http.post_json(API_URL, &headers, &body, self.timeout_ms)?;

        // Checked before the content blocks are even looked at: a refusal
        // response still carries a well-formed (often empty) `content`
        // array, so extracting text first and only checking `stop_reason`
        // afterwards would either silently return an empty string or fall
        // through to the "no text blocks" error below with a message that
        // has nothing to do with the real cause -- the model declining the
        // request, not a malformed response.
        if response.get("stop_reason").and_then(|s| s.as_str()) == Some("refusal") {
            return Err(BackendError::BadResponse("model declined the request".into()));
        }

        let blocks = response
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| BackendError::BadResponse(format!("no content in {response}")))?;

        // `content` can hold block types other than `"text"` -- most
        // notably `"thinking"`, extended-thinking output that is not the
        // answer this app pastes. Filtering to `"text"` blocks and joining
        // them (a real response can split one answer across more than one
        // text block) is what keeps a thinking block's contents out of the
        // clipboard.
        let joined: String = blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect();

        // Distinct from the `stop_reason` refusal check above: this is a
        // response that was not a refusal but still carried no usable text
        // (e.g. only thinking blocks, no text block at all) -- a different
        // failure mode that deserves its own message rather than being
        // folded into either the refusal case or the missing-`content` case.
        if joined.trim().is_empty() {
            return Err(BackendError::BadResponse("no text blocks in response".into()));
        }
        Ok(joined)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::backend::{BackendError, HttpClient};
    use crate::config::{BackendConfig, Effort, KIND_ANTHROPIC};

    struct FakeHttp {
        response: serde_json::Value,
        seen: Mutex<Option<(String, Vec<(String, String)>, serde_json::Value, u64)>>,
    }

    impl FakeHttp {
        fn new(response: serde_json::Value) -> Arc<Self> {
            Arc::new(Self { response, seen: Mutex::new(None) })
        }
    }

    impl HttpClient for FakeHttp {
        fn post_json(
            &self,
            url: &str,
            headers: &[(String, String)],
            body: &serde_json::Value,
            timeout_ms: u64,
        ) -> Result<serde_json::Value, BackendError> {
            *self.seen.lock().unwrap() = Some((url.into(), headers.to_vec(), body.clone(), timeout_ms));
            Ok(self.response.clone())
        }
    }

    fn cfg() -> BackendConfig {
        BackendConfig {
            id: "anthropic".into(),
            name: "Claude API".into(),
            kind: KIND_ANTHROPIC.into(),
            base_url: None,
            model: None,
            api_key: Some("sk-test".into()),
            timeout_ms: 120_000,
            effort: Effort::Low,
            session_persistence: false,
        }
    }

    fn message(blocks: serde_json::Value) -> serde_json::Value {
        json!({"content": blocks, "stop_reason": "end_turn"})
    }

    #[test]
    fn returns_the_text_block() {
        let http = FakeHttp::new(message(json!([{"type":"text","text":"Corrected."}])));
        let backend = AnthropicApi::new(&cfg(), http);
        assert_eq!(backend.transform("s", "t").unwrap(), "Corrected.");
    }

    #[test]
    fn skips_thinking_blocks_and_joins_text_blocks() {
        let blocks = json!([
            {"type":"thinking","thinking":""},
            {"type":"text","text":"One."},
            {"type":"text","text":" Two."}
        ]);
        let backend = AnthropicApi::new(&cfg(), FakeHttp::new(message(blocks)));
        assert_eq!(backend.transform("s", "t").unwrap(), "One. Two.");
    }

    #[test]
    fn a_refusal_is_a_backend_error_and_content_is_not_read() {
        // A real text block alongside `stop_reason: "refusal"` -- with the
        // old `content: []` fixture this test could not tell "the refusal
        // check fired" apart from "the empty-content check fired", since
        // both produce the identical `BadResponse` outcome. A response that
        // would otherwise succeed is the only way to pin the refusal branch
        // specifically.
        let body = json!({
            "content": [{"type": "text", "text": "should never be read"}],
            "stop_reason": "refusal"
        });
        let backend = AnthropicApi::new(&cfg(), FakeHttp::new(body));
        let err = backend.transform("s", "t").unwrap_err();
        assert!(
            matches!(&err, BackendError::BadResponse(msg) if msg.contains("declined")),
            "expected the refusal-specific message, got: {err:?}"
        );
    }

    #[test]
    fn sends_the_documented_request_shape() {
        let http = FakeHttp::new(message(json!([{"type":"text","text":"ok"}])));
        let backend = AnthropicApi::new(&cfg(), http.clone());
        backend.transform("Fix it.", "input").unwrap();
        let (url, headers, body, _timeout_ms) = http.seen.lock().unwrap().clone().unwrap();
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
        assert!(headers.iter().any(|(k, v)| k == "x-api-key" && v == "sk-test"));
        assert!(headers.iter().any(|(k, v)| k == "anthropic-version" && v == "2023-06-01"));
        assert_eq!(body["model"], "claude-opus-5");
        assert_eq!(body["max_tokens"], 16000);
        assert_eq!(body["system"], "Fix it.");
        assert_eq!(body["messages"][0]["content"], "input");
        assert!(body.get("temperature").is_none());
        assert!(body.get("stream").is_none());
    }

    // The configured timeout must reach the transport call unaltered --
    // otherwise a backend-specific timeout setting would be silently
    // ignored.
    #[test]
    fn the_configured_timeout_reaches_the_transport_call() {
        let mut config = cfg();
        config.timeout_ms = 42_000;
        let http = FakeHttp::new(message(json!([{"type":"text","text":"ok"}])));
        let backend = AnthropicApi::new(&config, http.clone());
        backend.transform("s", "t").unwrap();
        let (.., timeout_ms) = http.seen.lock().unwrap().clone().unwrap();
        assert_eq!(timeout_ms, 42_000);
    }

    // Merged: both are the same "no usable key" outcome, one missing and
    // one blank (reachable only by hand-editing config.json; the settings
    // UI trims empties to null). A blank key would otherwise be sent as an
    // empty `x-api-key` header and come back as an opaque 401.
    #[test]
    fn a_missing_or_blank_api_key_is_a_backend_error() {
        for api_key in [None, Some("  ".to_string())] {
            let mut config = cfg();
            config.api_key = api_key.clone();
            let backend = AnthropicApi::new(&config, FakeHttp::new(json!({})));
            assert!(
                matches!(backend.transform("s", "t"), Err(BackendError::Misconfigured(_))),
                "api_key: {api_key:?}"
            );
        }
    }

    #[test]
    fn a_blank_model_falls_back_to_the_default() {
        let mut config = cfg();
        config.model = Some("".into());
        let backend = AnthropicApi::new(&config, FakeHttp::new(json!({})));
        assert_eq!(backend.model, DEFAULT_MODEL);
    }
}
