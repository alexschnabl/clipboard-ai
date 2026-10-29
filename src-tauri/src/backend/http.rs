use std::sync::Arc;

use crate::backend::{Backend, BackendError, HttpClient};
use crate::config::BackendConfig;

pub struct HttpEndpoint {
    name: String,
    url: Option<String>,
    api_key: Option<String>,
    expects_response: bool,
    timeout_ms: u64,
    transport: Arc<dyn HttpClient>,
}

impl HttpEndpoint {
    pub fn new(config: &BackendConfig, transport: Arc<dyn HttpClient>) -> Self {
        Self {
            name: config.name.clone(),
            url: config.base_url.clone().filter(|u| !u.trim().is_empty()),
            api_key: config.api_key.clone().filter(|k| !k.trim().is_empty()),
            expects_response: config.expects_response,
            timeout_ms: config.timeout_ms,
            transport,
        }
    }
}

impl Backend for HttpEndpoint {
    fn name(&self) -> &str {
        &self.name
    }

    fn uses_sentinel(&self) -> bool {
        false
    }

    fn pastes_result(&self) -> bool {
        self.expects_response
    }

    fn supports_rich_text(&self) -> bool {
        false
    }

    fn transform(&self, _system_prompt: &str, text: &str) -> Result<String, BackendError> {
        let url = self
            .url
            .as_deref()
            .ok_or_else(|| BackendError::Misconfigured("url is required".into()))?;

        let mut headers =
            vec![("Content-Type".to_string(), "text/plain; charset=utf-8".to_string())];
        if let Some(key) = &self.api_key {
            headers.push(("Authorization".to_string(), format!("Bearer {key}")));
        }

        let body = self.transport.post_text(url, &headers, text, self.timeout_ms)?;

        if self.expects_response && body.trim().is_empty() {
            return Err(BackendError::BadResponse("endpoint returned an empty body".into()));
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::Value;

    use super::*;
    use crate::config::{Effort, KIND_HTTP};

    struct FakeTransport {
        seen: Mutex<Option<(String, Vec<(String, String)>, String, u64)>>,
        result: Result<String, BackendError>,
    }

    impl FakeTransport {
        fn ok(body: &str) -> Arc<Self> {
            Arc::new(Self { seen: Mutex::new(None), result: Ok(body.to_string()) })
        }
        fn failing(err: BackendError) -> Arc<Self> {
            Arc::new(Self { seen: Mutex::new(None), result: Err(err) })
        }
    }

    impl HttpClient for FakeTransport {
        fn post_json(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &Value,
            _timeout_ms: u64,
        ) -> Result<Value, BackendError> {
            panic!("the http backend must never send JSON");
        }

        fn post_text(
            &self,
            url: &str,
            headers: &[(String, String)],
            body: &str,
            timeout_ms: u64,
        ) -> Result<String, BackendError> {
            *self.seen.lock().unwrap() =
                Some((url.to_string(), headers.to_vec(), body.to_string(), timeout_ms));
            self.result.clone()
        }
    }

    fn cfg() -> BackendConfig {
        BackendConfig {
            id: "hook".into(),
            name: "My webhook".into(),
            kind: KIND_HTTP.into(),
            base_url: Some("http://localhost:8080/transform".into()),
            model: None,
            api_key: None,
            timeout_ms: 120_000,
            effort: Effort::Low,
            expects_response: true,
        }
    }

    #[test]
    fn posts_the_selected_text_as_the_raw_body_to_the_configured_url() {
        let transport = FakeTransport::ok("PONG");
        let backend = HttpEndpoint::new(&cfg(), transport.clone());
        backend.transform("a prompt that must not be sent", "selected text").unwrap();
        let (url, headers, body, timeout_ms) = transport.seen.lock().unwrap().clone().unwrap();
        assert_eq!(url, "http://localhost:8080/transform");
        assert_eq!(body, "selected text");
        assert_eq!(timeout_ms, 120_000);
        let content_type =
            headers.iter().find(|(k, _)| k == "Content-Type").expect("a content type is set");
        assert_eq!(content_type.1, "text/plain; charset=utf-8");
        assert!(
            !headers.iter().any(|(_, v)| v.contains("must not be sent")),
            "the command prompt must not leak into any header"
        );
    }

    #[test]
    fn the_response_body_is_returned_verbatim() {
        let backend = HttpEndpoint::new(&cfg(), FakeTransport::ok("transformed\n"));
        assert_eq!(backend.transform("s", "t").unwrap(), "transformed\n");
    }

    #[test]
    fn an_api_key_is_sent_as_a_bearer_token() {
        let mut config = cfg();
        config.api_key = Some("secret-token".into());
        let transport = FakeTransport::ok("ok");
        let backend = HttpEndpoint::new(&config, transport.clone());
        backend.transform("s", "t").unwrap();
        let (_url, headers, _body, _t) = transport.seen.lock().unwrap().clone().unwrap();
        let auth = headers.iter().find(|(k, _)| k == "Authorization").expect("Authorization set");
        assert_eq!(auth.1, "Bearer secret-token");
    }

    #[test]
    fn no_authorization_header_without_a_key() {
        let transport = FakeTransport::ok("ok");
        let backend = HttpEndpoint::new(&cfg(), transport.clone());
        backend.transform("s", "t").unwrap();
        let (_url, headers, _body, _t) = transport.seen.lock().unwrap().clone().unwrap();
        assert!(!headers.iter().any(|(k, _)| k == "Authorization"));
    }

    #[test]
    fn a_blank_key_is_treated_as_absent() {
        let mut config = cfg();
        config.api_key = Some("   ".into());
        let transport = FakeTransport::ok("ok");
        let backend = HttpEndpoint::new(&config, transport.clone());
        backend.transform("s", "t").unwrap();
        let (_url, headers, _body, _t) = transport.seen.lock().unwrap().clone().unwrap();
        assert!(!headers.iter().any(|(k, _)| k == "Authorization"));
    }

    #[test]
    fn a_missing_or_blank_url_is_misconfigured() {
        for url in [None, Some("".to_string()), Some("   ".to_string())] {
            let mut config = cfg();
            config.base_url = url.clone();
            let backend = HttpEndpoint::new(&config, FakeTransport::ok("ok"));
            assert!(
                matches!(backend.transform("s", "t"), Err(BackendError::Misconfigured(_))),
                "url {url:?} must be rejected before any request is sent"
            );
        }
    }

    #[test]
    fn an_empty_body_is_an_error_when_a_response_is_expected() {
        let backend = HttpEndpoint::new(&cfg(), FakeTransport::ok("  \n "));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))));
    }

    #[test]
    fn an_empty_body_is_fine_when_no_response_is_expected() {
        let mut config = cfg();
        config.expects_response = false;
        let backend = HttpEndpoint::new(&config, FakeTransport::ok(""));
        assert!(backend.transform("s", "t").is_ok());
    }

    #[test]
    fn send_only_still_surfaces_a_transport_failure() {
        let mut config = cfg();
        config.expects_response = false;
        let backend = HttpEndpoint::new(
            &config,
            FakeTransport::failing(BackendError::BadResponse("HTTP 500".into())),
        );
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))));
    }

    #[test]
    fn the_sentinel_contract_is_declared_off_and_paste_follows_expects_response() {
        let expecting = HttpEndpoint::new(&cfg(), FakeTransport::ok("ok"));
        assert!(!expecting.uses_sentinel());
        assert!(expecting.pastes_result());

        let mut config = cfg();
        config.expects_response = false;
        let send_only = HttpEndpoint::new(&config, FakeTransport::ok("ok"));
        assert!(!send_only.pastes_result());
    }
}
