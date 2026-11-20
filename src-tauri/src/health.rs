use std::{
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::backend::{
    self,
    claude_cli::{CommandRunner, ProcessRunner},
    BackendError,
};
use crate::config::{BackendConfig, KIND_ANTHROPIC, KIND_CLAUDE_CLI, KIND_HTTP, KIND_OPENAI};
use crate::secrets::{self, SecretStore};

const CHECK_TIMEOUT_CAP_MS: u64 = 60_000;
const SLOW_MS: u64 = 5_000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CLI_TIMEOUT_MS: u64 = 15_000;
const PROBE_SYSTEM: &str = "Reply with the single word OK and nothing else.";
const PROBE_TEXT: &str = "ping";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Health {
    Ok { ms: u64 },
    Cold { first_ms: u64, warm_ms: u64 },
    Slow { ms: u64 },
    Reachable { ms: u64 },
    CliReady { version: String },
    CliSignedOut { version: String },
    CliUnknownAuth { version: String },
    CliMissing,
    KeyMissing,
    KeyRejected,
    ModelNotFound,
    NotReachable { detail: String },
    Timeout { secs: u64 },
    Misconfigured { detail: String },
    BadResponse { detail: String },
}

fn short(detail: &str) -> String {
    let first = detail.lines().next().unwrap_or("").trim();
    if first.chars().count() > 140 {
        format!("{}...", first.chars().take(140).collect::<String>())
    } else {
        first.to_string()
    }
}

fn classify(err: BackendError, has_key: bool, timeout_ms: u64) -> Health {
    match err {
        BackendError::Timeout => Health::Timeout { secs: timeout_ms.div_ceil(1000) },
        BackendError::Transport(detail) => Health::NotReachable { detail: short(&detail) },
        BackendError::Misconfigured(detail) if detail.contains("apiKey is required") => {
            Health::KeyMissing
        }
        BackendError::Misconfigured(detail) => Health::Misconfigured { detail: short(&detail) },
        BackendError::BadResponse(detail) => {
            if detail.starts_with("HTTP 401") || detail.starts_with("HTTP 403") {
                if has_key {
                    Health::KeyRejected
                } else {
                    Health::KeyMissing
                }
            } else if detail.starts_with("HTTP 404") {
                Health::ModelNotFound
            } else {
                Health::BadResponse { detail: short(&detail) }
            }
        }
    }
}

fn probe_model(config: &BackendConfig, store: &dyn SecretStore) -> Health {
    let mut config = config.clone();
    config.timeout_ms = config.timeout_ms.min(CHECK_TIMEOUT_CAP_MS);
    let has_key = matches!(
        secrets::resolve_with(store, &config.id, config.api_key.as_deref()),
        Ok(Some(_))
    );
    let built = match backend::build_with(&config, store) {
        Ok(b) => b,
        Err(e) => return classify(e, has_key, config.timeout_ms),
    };
    let timed = || {
        let start = Instant::now();
        built.transform(PROBE_SYSTEM, PROBE_TEXT).map(|_| start.elapsed().as_millis() as u64)
    };
    let first_ms = match timed() {
        Ok(ms) => ms,
        Err(e) => return classify(e, has_key, config.timeout_ms),
    };
    if first_ms < SLOW_MS {
        return Health::Ok { ms: first_ms };
    }
    if config.kind != KIND_OPENAI {
        return Health::Slow { ms: first_ms };
    }
    match timed() {
        Ok(warm_ms) if warm_ms * 2 < first_ms => Health::Cold { first_ms, warm_ms },
        Ok(warm_ms) => Health::Slow { ms: warm_ms.max(first_ms) },
        Err(e) => classify(e, has_key, config.timeout_ms),
    }
}

fn probe_connection(config: &BackendConfig) -> Health {
    let Some(raw) = config.base_url.as_deref().map(str::trim).filter(|u| !u.is_empty()) else {
        return Health::Misconfigured { detail: "no URL set".into() };
    };
    let url = match reqwest::Url::parse(raw) {
        Ok(url) => url,
        Err(e) => return Health::Misconfigured { detail: format!("the URL is not valid ({e})") },
    };
    let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
        return Health::Misconfigured { detail: "the URL has no host".into() };
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let start = Instant::now();
    let addrs = match (host, port).to_socket_addrs() {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(e) => return Health::NotReachable { detail: short(&e.to_string()) },
    };
    let mut last_error = String::from("no address found");
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(_) => return Health::Reachable { ms: start.elapsed().as_millis() as u64 },
            Err(e) => last_error = e.to_string(),
        }
    }
    Health::NotReachable { detail: short(&last_error) }
}

fn probe_cli(runner: &dyn CommandRunner) -> Health {
    let version = match runner.run("claude", &["--version".to_string()], "", CLI_TIMEOUT_MS) {
        Ok(out) => out.trim().to_string(),
        Err(BackendError::Transport(_)) => return Health::CliMissing,
        Err(e) => return classify(e, false, CLI_TIMEOUT_MS),
    };
    let args = ["auth".to_string(), "status".to_string(), "--json".to_string()];
    match runner.run("claude", &args, "", CLI_TIMEOUT_MS) {
        Ok(out) => match serde_json::from_str::<serde_json::Value>(out.trim())
            .ok()
            .and_then(|v| v.get("loggedIn").and_then(|b| b.as_bool()))
        {
            Some(true) => Health::CliReady { version },
            Some(false) => Health::CliSignedOut { version },
            None => Health::CliUnknownAuth { version },
        },
        Err(BackendError::BadResponse(detail))
            if detail.contains("\"loggedIn\": false") || detail.contains("\"loggedIn\":false") =>
        {
            Health::CliSignedOut { version }
        }
        Err(BackendError::BadResponse(detail)) if detail.to_lowercase().contains("not logged in") => {
            Health::CliSignedOut { version }
        }
        Err(_) => Health::CliUnknownAuth { version },
    }
}

pub fn check(config: &BackendConfig) -> Health {
    check_with(config, &secrets::CredentialManager, &ProcessRunner)
}

pub(crate) fn check_with(
    config: &BackendConfig,
    store: &dyn SecretStore,
    runner: &dyn CommandRunner,
) -> Health {
    match config.kind.as_str() {
        KIND_HTTP => probe_connection(config),
        KIND_CLAUDE_CLI => probe_cli(runner),
        KIND_OPENAI | KIND_ANTHROPIC => probe_model(config, store),
        other => Health::Misconfigured { detail: format!("unknown kind {other}") },
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Mutex,
        thread,
    };

    use super::*;
    use crate::config::Effort;
    use crate::secrets::fake::FakeStore;

    fn cfg(kind: &str, base_url: Option<String>) -> BackendConfig {
        BackendConfig {
            id: "h".into(),
            name: "H".into(),
            kind: kind.into(),
            base_url,
            model: Some("m".into()),
            api_key: None,
            timeout_ms: 10_000,
            effort: Effort::Low,
            expects_response: true,
        }
    }

    struct ScriptedRunner {
        replies: Mutex<Vec<Result<String, BackendError>>>,
        seen: Mutex<Vec<Vec<String>>>,
    }

    impl ScriptedRunner {
        fn new(replies: Vec<Result<String, BackendError>>) -> Self {
            Self { replies: Mutex::new(replies), seen: Mutex::new(Vec::new()) }
        }
    }

    impl CommandRunner for ScriptedRunner {
        fn run(&self, _p: &str, args: &[String], _s: &str, _t: u64) -> Result<String, BackendError> {
            self.seen.lock().unwrap().push(args.to_vec());
            self.replies.lock().unwrap().remove(0)
        }
    }

    #[test]
    fn classifies_backend_errors_into_plain_states() {
        assert_eq!(classify(BackendError::Timeout, true, 60_000), Health::Timeout { secs: 60 });
        assert_eq!(
            classify(BackendError::BadResponse("HTTP 401 Unauthorized: x".into()), true, 1),
            Health::KeyRejected
        );
        assert_eq!(
            classify(BackendError::BadResponse("HTTP 401 Unauthorized: x".into()), false, 1),
            Health::KeyMissing
        );
        assert_eq!(
            classify(BackendError::BadResponse("HTTP 404 Not Found: x".into()), true, 1),
            Health::ModelNotFound
        );
        assert_eq!(
            classify(BackendError::Misconfigured("apiKey is required".into()), false, 1),
            Health::KeyMissing
        );
        assert!(matches!(
            classify(BackendError::Transport("connection refused".into()), false, 1),
            Health::NotReachable { .. }
        ));
    }

    #[test]
    fn long_details_are_cut_to_one_short_line() {
        let detail = format!("{}\nsecond line", "x".repeat(400));
        let cut = short(&detail);
        assert!(cut.len() < 150 && !cut.contains('\n'));
    }

    #[test]
    fn an_anthropic_backend_without_a_key_reports_the_missing_key_without_a_request() {
        let health = check_with(
            &cfg(KIND_ANTHROPIC, None),
            &FakeStore::default(),
            &ScriptedRunner::new(vec![]),
        );
        assert_eq!(health, Health::KeyMissing);
    }

    #[test]
    fn the_http_backend_only_opens_a_connection_and_sends_nothing() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let received = std::sync::Arc::new(Mutex::new(Vec::new()));
        let sink = received.clone();
        let server = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                let mut buf = [0u8; 256];
                if let Ok(n) = stream.read(&mut buf) {
                    sink.lock().unwrap().extend_from_slice(&buf[..n]);
                }
                let _ = stream.write_all(b"");
            }
        });
        let health = check_with(
            &cfg(KIND_HTTP, Some(format!("http://{addr}/webhook/notes"))),
            &FakeStore::default(),
            &ScriptedRunner::new(vec![]),
        );
        server.join().unwrap();
        assert!(matches!(health, Health::Reachable { .. }), "got {health:?}");
        assert!(received.lock().unwrap().is_empty(), "no byte may reach the webhook");
    }

    #[test]
    fn an_http_backend_with_nothing_listening_is_not_reachable() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let health = check_with(
            &cfg(KIND_HTTP, Some(format!("http://127.0.0.1:{port}/hook"))),
            &FakeStore::default(),
            &ScriptedRunner::new(vec![]),
        );
        assert!(matches!(health, Health::NotReachable { .. }), "got {health:?}");
    }

    #[test]
    fn an_http_backend_without_a_url_is_misconfigured() {
        let health =
            check_with(&cfg(KIND_HTTP, None), &FakeStore::default(), &ScriptedRunner::new(vec![]));
        assert!(matches!(health, Health::Misconfigured { .. }));
    }

    #[test]
    fn the_cli_check_runs_version_then_auth_status() {
        let runner = ScriptedRunner::new(vec![
            Ok("2.1.329 (Claude Code)\n".into()),
            Ok(r#"{"loggedIn": true, "authMethod": "claude.ai"}"#.into()),
        ]);
        let health = check_with(&cfg(KIND_CLAUDE_CLI, None), &FakeStore::default(), &runner);
        assert_eq!(health, Health::CliReady { version: "2.1.329 (Claude Code)".into() });
        let seen = runner.seen.lock().unwrap();
        assert_eq!(seen[0], vec!["--version".to_string()]);
        assert_eq!(seen[1], vec!["auth".to_string(), "status".to_string(), "--json".to_string()]);
    }

    #[test]
    fn the_cli_check_reports_signed_out_missing_and_unknown() {
        let signed_out = ScriptedRunner::new(vec![
            Ok("2.1.0".into()),
            Err(BackendError::BadResponse(r#"claude exited with 1: {"loggedIn": false}"#.into())),
        ]);
        assert_eq!(
            check_with(&cfg(KIND_CLAUDE_CLI, None), &FakeStore::default(), &signed_out),
            Health::CliSignedOut { version: "2.1.0".into() }
        );
        let missing = ScriptedRunner::new(vec![Err(BackendError::Transport(
            "could not start claude".into(),
        ))]);
        assert_eq!(
            check_with(&cfg(KIND_CLAUDE_CLI, None), &FakeStore::default(), &missing),
            Health::CliMissing
        );
        let old_cli = ScriptedRunner::new(vec![Ok("1.0.0".into()), Err(BackendError::Timeout)]);
        assert_eq!(
            check_with(&cfg(KIND_CLAUDE_CLI, None), &FakeStore::default(), &old_cli),
            Health::CliUnknownAuth { version: "1.0.0".into() }
        );
    }

    #[test]
    fn an_openai_compatible_backend_that_answers_reports_its_time() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let body = r#"{"choices":[{"message":{"role":"assistant","content":"OK"}}]}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let health = check_with(
            &cfg(KIND_OPENAI, Some(format!("http://{addr}/v1"))),
            &FakeStore::default(),
            &ScriptedRunner::new(vec![]),
        );
        assert!(matches!(health, Health::Ok { .. }), "got {health:?}");
    }

    #[test]
    fn health_serializes_with_a_status_tag_and_camel_case_fields() {
        let json = serde_json::to_value(Health::Cold { first_ms: 16_000, warm_ms: 900 }).unwrap();
        assert_eq!(json, serde_json::json!({"status": "cold", "firstMs": 16000, "warmMs": 900}));
        let json = serde_json::to_value(Health::KeyRejected).unwrap();
        assert_eq!(json, serde_json::json!({"status": "keyRejected"}));
    }
}
