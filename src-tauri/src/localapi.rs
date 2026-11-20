use std::{
    fs,
    io::{self, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{Config, LocalApiSettings};
use crate::textrun::{self, Plan, ResolveError, RunError, TextResult};

pub const MAX_HEADER_BYTES: usize = 8 * 1024;
pub const MAX_HEADERS: usize = 64;
pub const MAX_BODY_BYTES: usize = 256 * 1024;
pub const MAX_TEXT_CHARS: usize = 100_000;
pub const MAX_PROMPT_CHARS: usize = 20_000;
pub const MAX_NAME_CHARS: usize = 200;
pub const MAX_CONNECTIONS: usize = 16;
pub const MAX_CONCURRENT_RUNS: usize = 4;
pub const STATE_FILE: &str = "local-api.json";
const HEAD_DEADLINE: Duration = Duration::from_secs(3);
const REQUEST_DEADLINE: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const DRAIN_LIMIT: usize = 1024 * 1024;
const DRAIN_TIMEOUT: Duration = Duration::from_millis(500);
const TOKEN_PREFIX: &str = "cai_";
const TOKEN_BYTES: usize = 32;
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub type SharedToken = Arc<RwLock<String>>;

pub trait Engine: Send + Sync {
    fn config(&self) -> Config;
    fn run(&self, plan: &Plan, text: &str) -> Result<TextResult, RunError> {
        textrun::run_plan(plan, text)
    }
}

pub fn generate_token() -> io::Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    let mut token = String::with_capacity(TOKEN_PREFIX.len() + TOKEN_BYTES * 2);
    token.push_str(TOKEN_PREFIX);
    for byte in bytes {
        token.push_str(&format!("{byte:02x}"));
    }
    Ok(token)
}

fn is_well_formed_token(token: &str) -> bool {
    token.len() == TOKEN_PREFIX.len() + TOKEN_BYTES * 2
        && token.starts_with(TOKEN_PREFIX)
        && token[TOKEN_PREFIX.len()..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn tokens_match(given: &[u8], expected: &[u8]) -> bool {
    if given.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in given.iter().zip(expected.iter()) {
        diff |= a ^ b;
    }
    std::hint::black_box(diff) == 0
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: Value,
    pub headers: Vec<(&'static str, String)>,
}

impl Response {
    fn ok(body: Value) -> Self {
        Response { status: 200, body, headers: Vec::new() }
    }

    fn error(status: u16, code: &str, message: impl Into<String>) -> Self {
        Response {
            status,
            body: json!({"ok": false, "error": {"code": code, "message": message.into()}}),
            headers: Vec::new(),
        }
    }

    fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    }
}

pub fn render_response(response: &Response) -> Vec<u8> {
    let body = response.body.to_string();
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n",
        response.status,
        reason_phrase(response.status),
        body.len()
    );
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(body.as_bytes());
    bytes
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    fn header_count(&self, name: &str) -> usize {
        self.headers.iter().filter(|(n, _)| n == name).count()
    }
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn is_timeout(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)
}

fn timed_out() -> Response {
    Response::error(408, "request-timeout", "the request was not sent in time")
}

fn is_token_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

pub fn parse_head(head: &[u8]) -> Result<(String, String, Vec<(String, String)>), Response> {
    let bad = |msg: &str| Response::error(400, "bad-request", msg);
    let head = std::str::from_utf8(head).map_err(|_| bad("the request head is not valid text"))?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad("malformed request line"));
    };
    if method.is_empty() || !method.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(bad("malformed method"));
    }
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(bad("only HTTP/1.0 and HTTP/1.1 are supported"));
    }
    if !target.starts_with('/') {
        return Err(bad("the request target must be a path"));
    }
    let path = target.split(['?', '#']).next().unwrap_or_default().to_string();

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if headers.len() >= MAX_HEADERS {
            return Err(Response::error(431, "headers-too-large", "too many headers"));
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(bad("malformed header line"));
        };
        if name.is_empty() || !name.bytes().all(is_token_char) {
            return Err(bad("malformed header name"));
        }
        headers.push((name.to_ascii_lowercase(), value.trim_matches([' ', '\t']).to_string()));
    }
    Ok((method.to_string(), path, headers))
}

fn read_request(stream: &mut TcpStream, started: Instant) -> Result<Request, Response> {
    let head_deadline = started + HEAD_DEADLINE;
    let deadline = started + REQUEST_DEADLINE;
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(pos) = find_head_end(&buf) {
            break pos;
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err(Response::error(431, "headers-too-large", "the request head is too large"));
        }
        let remaining = head_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(timed_out());
        }
        let _ = stream.set_read_timeout(Some(remaining));
        match stream.read(&mut chunk) {
            Ok(0) => return Err(Response::error(400, "bad-request", "the request ended early")),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if is_timeout(&e) => return Err(timed_out()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Response::error(400, "bad-request", "the request could not be read")),
        }
    };
    if head_end > MAX_HEADER_BYTES {
        return Err(Response::error(431, "headers-too-large", "the request head is too large"));
    }
    let (method, path, headers) = parse_head(&buf[..head_end])?;
    let mut request = Request { method, path, headers, body: Vec::new() };

    if request.header("transfer-encoding").is_some() {
        return Err(Response::error(
            501,
            "chunked-not-supported",
            "chunked bodies are not supported; send Content-Length",
        ));
    }
    let length = match request.header_count("content-length") {
        0 => 0,
        1 => {
            let raw = request.header("content-length").unwrap_or_default();
            if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) || raw.len() > 12 {
                return Err(Response::error(400, "bad-request", "malformed Content-Length"));
            }
            raw.parse::<usize>().map_err(|_| Response::error(400, "bad-request", "malformed Content-Length"))?
        }
        _ => return Err(Response::error(400, "bad-request", "more than one Content-Length")),
    };
    if length > MAX_BODY_BYTES {
        return Err(Response::error(
            413,
            "body-too-large",
            format!("the request body is larger than {MAX_BODY_BYTES} bytes"),
        ));
    }

    let mut body: Vec<u8> = buf[head_end + 4..].to_vec();
    body.truncate(length);
    while body.len() < length {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(timed_out());
        }
        let _ = stream.set_read_timeout(Some(remaining));
        let want = (length - body.len()).min(chunk.len());
        match stream.read(&mut chunk[..want]) {
            Ok(0) => return Err(Response::error(400, "bad-request", "the request body ended early")),
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(e) if is_timeout(&e) => return Err(timed_out()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Response::error(400, "bad-request", "the request could not be read")),
        }
    }
    request.body = body;
    Ok(request)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunBody {
    #[serde(default)]
    command: Option<String>,
    text: String,
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
}

struct Shared {
    token: SharedToken,
    engine: Arc<dyn Engine>,
    port: u16,
    connections: AtomicUsize,
    runs: AtomicUsize,
}

struct SlotGuard<'a>(&'a AtomicUsize);

impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn take_slot(counter: &AtomicUsize, limit: usize) -> Option<SlotGuard<'_>> {
    let previous = counter.fetch_add(1, Ordering::SeqCst);
    let guard = SlotGuard(counter);
    if previous >= limit {
        return None;
    }
    Some(guard)
}

fn host_is_allowed(host: &str, port: u16) -> bool {
    let host = host.to_ascii_lowercase();
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
}

fn route(request: &Request, shared: &Shared) -> Response {
    match request.header_count("host") {
        1 => {}
        0 => return Response::error(400, "bad-request", "the Host header is missing"),
        _ => return Response::error(400, "bad-request", "more than one Host header"),
    }
    if !host_is_allowed(request.header("host").unwrap_or_default(), shared.port) {
        return Response::error(403, "host-refused", "requests must be addressed to 127.0.0.1 or localhost");
    }
    let from_browser = ["origin", "sec-fetch-site", "sec-fetch-mode", "sec-fetch-dest"]
        .iter()
        .any(|name| request.header(name).is_some());
    if from_browser {
        return Response::error(403, "browser-refused", "requests from web pages are refused");
    }

    if request.header_count("authorization") != 1 {
        return Response::error(401, "unauthorized", "send the token as Authorization: Bearer <token>")
            .with_header("WWW-Authenticate", "Bearer");
    }
    let given = request
        .header("authorization")
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token.trim());
    let authorized = match given {
        Some(given) => {
            let expected = shared.token.read().map(|t| t.clone()).unwrap_or_default();
            !expected.is_empty() && tokens_match(given.as_bytes(), expected.as_bytes())
        }
        None => false,
    };
    if !authorized {
        return Response::error(401, "unauthorized", "the token is missing or wrong")
            .with_header("WWW-Authenticate", "Bearer error=\"invalid_token\"");
    }

    let allow = |methods: &'static str| {
        Response::error(405, "method-not-allowed", format!("use {methods} for this path"))
            .with_header("Allow", methods)
    };
    match (request.path.as_str(), request.method.as_str()) {
        ("/v1/health", "GET") => Response::ok(json!({"ok": true, "app": "clipboard-ai", "version": VERSION})),
        ("/v1/health", _) => allow("GET"),
        ("/v1/commands", "GET") => {
            let mut body = textrun::listing_json(&shared.engine.config());
            body["ok"] = json!(true);
            Response::ok(body)
        }
        ("/v1/commands", _) => allow("GET"),
        ("/v1/run", "POST") => handle_run(request, shared),
        ("/v1/run", _) => allow("POST"),
        _ => Response::error(404, "not-found", "unknown path; see /v1/health, /v1/commands and /v1/run"),
    }
}

fn too_long(field: &str, limit: usize) -> Response {
    Response::error(413, "field-too-large", format!("{field} is longer than {limit} characters"))
}

fn handle_run(request: &Request, shared: &Shared) -> Response {
    let media_type = request
        .header("content-type")
        .map(|v| v.split(';').next().unwrap_or_default().trim().to_ascii_lowercase());
    if media_type.as_deref() != Some("application/json") {
        return Response::error(415, "unsupported-media-type", "send the body as application/json");
    }
    let body: RunBody = match serde_json::from_slice(&request.body) {
        Ok(body) => body,
        Err(e) => return Response::error(400, "invalid-json", format!("the body is not a valid run request: {e}")),
    };
    if body.text.trim().is_empty() {
        return Response::error(400, "empty-text", "text is empty");
    }
    if body.text.chars().count() > MAX_TEXT_CHARS {
        return too_long("text", MAX_TEXT_CHARS);
    }
    if body.prompt.as_deref().is_some_and(|p| p.chars().count() > MAX_PROMPT_CHARS) {
        return too_long("prompt", MAX_PROMPT_CHARS);
    }
    for (field, value) in [("command", &body.command), ("backend", &body.backend)] {
        if value.as_deref().is_some_and(|v| v.chars().count() > MAX_NAME_CHARS) {
            return too_long(field, MAX_NAME_CHARS);
        }
    }

    let Some(_slot) = take_slot(&shared.runs, MAX_CONCURRENT_RUNS) else {
        return Response::error(429, "busy", format!("{MAX_CONCURRENT_RUNS} runs are already in progress"))
            .with_header("Retry-After", "1");
    };

    let config = shared.engine.config();
    let plan = match textrun::resolve(
        &config,
        body.command.as_deref(),
        body.backend.as_deref(),
        body.prompt.as_deref(),
    ) {
        Ok(plan) => plan,
        Err(e) => {
            let status = match e {
                ResolveError::UnknownCommand(_) | ResolveError::UnknownBackend(_) => 404,
                _ => 400,
            };
            return Response::error(status, e.code(), e.to_string());
        }
    };

    match shared.engine.run(&plan, &body.text) {
        Ok(result) => {
            let command = match (&plan.command_id, &plan.command_name) {
                (Some(id), Some(name)) => json!({"id": id, "name": name}),
                _ => Value::Null,
            };
            Response::ok(json!({
                "ok": true,
                "command": command,
                "backend": {"id": plan.backend.id, "name": plan.backend.name},
                "sent": result.text.is_none(),
                "text": result.text,
                "html": result.html,
                "backendMs": result.backend_ms,
            }))
        }
        Err(e) => Response::error(502, e.code(), format!("{} failed: {}", plan.backend.name, public_run_error(&e))),
    }
}

pub fn public_run_error(error: &RunError) -> String {
    use crate::backend::BackendError;
    match error {
        RunError::Backend(BackendError::Transport(_)) => "the backend could not be reached".into(),
        RunError::Backend(BackendError::BadResponse(_)) => {
            "the backend returned an error or an unusable response; run the command with the clipboard-ai tool to see the details".into()
        }
        RunError::Backend(BackendError::Timeout) => "the backend timed out".into(),
        RunError::Backend(BackendError::Misconfigured(_)) => {
            "the backend is not set up completely; check it in Settings".into()
        }
        other => other.to_string(),
    }
}

fn drain(stream: &mut TcpStream) {
    let _ = stream.set_read_timeout(Some(DRAIN_TIMEOUT));
    let started = Instant::now();
    let mut total = 0usize;
    let mut chunk = [0u8; 8192];
    while total < DRAIN_LIMIT && started.elapsed() < DRAIN_TIMEOUT {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }
}

fn handle_connection(mut stream: TcpStream, shared: &Shared) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let response = match read_request(&mut stream, Instant::now()) {
        Ok(request) => route(&request, shared),
        Err(response) => response,
    };
    let _ = stream.write_all(&render_response(&response));
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
    drain(&mut stream);
}

pub struct Server {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(port: u16, token: SharedToken, engine: Arc<dyn Engine>) -> io::Result<Server> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))?;
        let addr = listener.local_addr()?;
        if !addr.ip().is_loopback() {
            return Err(io::Error::other("refusing to listen on a non-loopback address"));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Shared {
            token,
            engine,
            port: addr.port(),
            connections: AtomicUsize::new(0),
            runs: AtomicUsize::new(0),
        });
        let stop_flag = stop.clone();
        let thread = thread::Builder::new().name("local-api".into()).spawn(move || {
            for stream in listener.incoming() {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else {
                    thread::sleep(Duration::from_millis(20));
                    continue;
                };
                if !stream.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false) {
                    continue;
                }
                let previous = shared.connections.fetch_add(1, Ordering::SeqCst);
                if previous >= MAX_CONNECTIONS {
                    shared.connections.fetch_sub(1, Ordering::SeqCst);
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                    let busy = Response::error(503, "too-many-connections", "too many open connections")
                        .with_header("Retry-After", "1");
                    let _ = stream.write_all(&render_response(&busy));
                    continue;
                }
                let shared = shared.clone();
                let spawned = thread::Builder::new().name("local-api-conn".into()).spawn(move || {
                    let _slot = SlotGuard(&shared.connections);
                    handle_connection(stream, &shared);
                });
                if spawned.is_err() {
                    thread::sleep(Duration::from_millis(20));
                }
            }
        })?;
        Ok(Server { addr, stop, thread: Some(thread) })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn stop(mut self) {
        self.shut_down();
    }

    fn shut_down(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.stop.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect_timeout(&self.addr, Duration::from_secs(1));
            let _ = thread.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shut_down();
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct StateFile {
    token: String,
    url: Option<String>,
}

fn read_state(dir: &Path) -> Option<StateFile> {
    let raw = fs::read_to_string(dir.join(STATE_FILE)).ok()?;
    let state: StateFile = serde_json::from_str(&raw).ok()?;
    is_well_formed_token(&state.token).then_some(state)
}

fn write_state(dir: &Path, state: &StateFile) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(STATE_FILE);
    let tmp = dir.join(format!("{STATE_FILE}.tmp"));
    let json = serde_json::to_string_pretty(state).map_err(io::Error::other)?;
    if let Err(e) = fs::write(&tmp, json) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiStatus {
    pub enabled: bool,
    pub running: bool,
    pub url: Option<String>,
    pub port: Option<u16>,
    pub token: Option<String>,
    pub error: Option<String>,
}

#[derive(Default)]
struct Inner {
    settings: LocalApiSettings,
    server: Option<Server>,
    started_with: Option<Option<u16>>,
    token: Option<SharedToken>,
    error: Option<String>,
}

pub struct LocalApi {
    config_dir: PathBuf,
    inner: Mutex<Inner>,
}

fn url_for(addr: SocketAddr) -> String {
    format!("http://127.0.0.1:{}", addr.port())
}

impl LocalApi {
    pub fn new(config_dir: PathBuf) -> Self {
        if let Some(state) = read_state(&config_dir).filter(|s| s.url.is_some()) {
            let _ = write_state(&config_dir, &StateFile { token: state.token, url: None });
        }
        LocalApi { config_dir, inner: Mutex::new(Inner::default()) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        crate::recover_lock(self.inner.lock(), "local api", Some(&self.config_dir))
    }

    fn log(&self, line: &str) {
        let line = format!("LOCALAPI {line}");
        eprintln!("{line}");
        crate::status::append_log(&self.config_dir, &line);
    }

    fn ensure_token(&self, inner: &mut Inner) -> io::Result<SharedToken> {
        if let Some(token) = &inner.token {
            return Ok(token.clone());
        }
        let token = match read_state(&self.config_dir) {
            Some(state) => state.token,
            None => {
                let token = generate_token()?;
                write_state(&self.config_dir, &StateFile { token: token.clone(), url: None })?;
                token
            }
        };
        let shared = Arc::new(RwLock::new(token));
        inner.token = Some(shared.clone());
        Ok(shared)
    }

    fn current_token(inner: &Inner) -> Option<String> {
        inner.token.as_ref().and_then(|t| t.read().ok().map(|t| t.clone()))
    }

    fn record_url(&self, inner: &Inner, url: Option<String>) {
        if let Some(token) = Self::current_token(inner) {
            if let Err(e) = write_state(&self.config_dir, &StateFile { token, url }) {
                self.log(&format!("state-file-write-failed reason={e}"));
            }
        }
    }

    fn stop_server(&self, inner: &mut Inner) {
        if let Some(server) = inner.server.take() {
            server.stop();
            inner.started_with = None;
            self.record_url(inner, None);
            self.log("stopped");
        }
    }

    pub fn apply(&self, settings: LocalApiSettings, engine: impl FnOnce() -> Arc<dyn Engine>) -> Option<String> {
        let mut inner = self.lock();
        inner.settings = settings;
        if !settings.enabled {
            self.stop_server(&mut inner);
            inner.error = None;
            return None;
        }
        let wanted_port = settings.port.filter(|p| *p != 0);
        if inner.server.is_some() && inner.started_with == Some(wanted_port) {
            return None;
        }
        self.stop_server(&mut inner);

        let token = match self.ensure_token(&mut inner) {
            Ok(token) => token,
            Err(e) => {
                let message = format!("the local API could not create its token: {e}");
                self.log(&format!("token-failed reason={e}"));
                inner.error = Some(message.clone());
                return Some(message);
            }
        };
        match Server::start(wanted_port.unwrap_or(0), token, engine()) {
            Ok(server) => {
                let url = url_for(server.addr());
                self.log(&format!("listening on {url}"));
                inner.server = Some(server);
                inner.started_with = Some(wanted_port);
                inner.error = None;
                self.record_url(&inner, Some(url));
                None
            }
            Err(e) => {
                let port = wanted_port.map(|p| p.to_string()).unwrap_or_else(|| "a free port".into());
                let message = format!("the local API could not listen on 127.0.0.1 port {port}: {e}");
                self.log(&format!("start-failed port={port} reason={e}"));
                inner.error = Some(message.clone());
                Some(message)
            }
        }
    }

    pub fn status(&self) -> LocalApiStatus {
        let inner = self.lock();
        let enabled = inner.settings.enabled;
        let addr = inner.server.as_ref().map(|s| s.addr());
        LocalApiStatus {
            enabled,
            running: addr.is_some(),
            url: addr.map(url_for),
            port: addr.map(|a| a.port()),
            token: if enabled { Self::current_token(&inner) } else { None },
            error: inner.error.clone(),
        }
    }

    pub fn regenerate_token(&self) -> Result<LocalApiStatus, String> {
        {
            let mut inner = self.lock();
            let fresh = generate_token().map_err(|e| format!("could not create a token: {e}"))?;
            let url = inner.server.as_ref().map(|s| url_for(s.addr()));
            write_state(&self.config_dir, &StateFile { token: fresh.clone(), url })
                .map_err(|e| format!("could not save the new token: {e}"))?;
            match &inner.token {
                Some(shared) => {
                    if let Ok(mut current) = shared.write() {
                        *current = fresh;
                    }
                }
                None => inner.token = Some(Arc::new(RwLock::new(fresh))),
            }
            self.log("token-regenerated");
        }
        Ok(self.status())
    }

    pub fn shutdown(&self) {
        let mut inner = self.lock();
        self.stop_server(&mut inner);
    }
}

struct AppEngine(tauri::AppHandle);

impl Engine for AppEngine {
    fn config(&self) -> Config {
        use tauri::Manager;
        match self.0.try_state::<crate::AppState>() {
            Some(state) => crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone(),
            None => Config {
                version: crate::config::CURRENT_VERSION,
                backends: Vec::new(),
                commands: Vec::new(),
                settings: Default::default(),
                vocabulary: Default::default(),
            },
        }
    }
}

pub(crate) fn apply_for_app(app: &tauri::AppHandle, settings: LocalApiSettings) -> Option<crate::Problem> {
    use tauri::Manager;
    let api = app.try_state::<LocalApi>()?;
    let handle = app.clone();
    api.apply(settings, move || Arc::new(AppEngine(handle)))
        .map(|message| crate::Problem { command_id: None, message })
}

pub(crate) fn shutdown_for_app(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(api) = app.try_state::<LocalApi>() {
        api.shutdown();
    }
}

#[tauri::command]
pub fn local_api_status(api: tauri::State<'_, LocalApi>) -> LocalApiStatus {
    api.status()
}

#[tauri::command]
pub fn local_api_regenerate_token(api: tauri::State<'_, LocalApi>) -> Result<LocalApiStatus, String> {
    api.regenerate_token()
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use crate::backend::BackendError;
    use crate::config::{self, BackendConfig, CommandConfig, Effort, Formatting, KIND_HTTP};

    fn test_config() -> Config {
        Config {
            version: config::CURRENT_VERSION,
            backends: vec![BackendConfig {
                id: "hook".into(),
                name: "Hook".into(),
                kind: KIND_HTTP.into(),
                base_url: Some("http://127.0.0.1:9/secret-path".into()),
                model: None,
                api_key: Some("sk-very-secret".into()),
                timeout_ms: 10_000,
                effort: Effort::Low,
                expects_response: true,
            }],
            commands: vec![CommandConfig {
                id: "fix".into(),
                name: "Fix grammar".into(),
                backend_id: "hook".into(),
                hotkey: "Ctrl+Alt+J".into(),
                system_prompt: "Fix.".into(),
                formatting: Formatting::Auto,
                app_rules: Vec::new(),
            }],
            settings: config::Settings::default(),
            vocabulary: Default::default(),
        }
    }

    #[derive(Default)]
    struct FakeEngine {
        gate: Option<Mutex<mpsc::Receiver<()>>>,
        fail: bool,
        seen: Mutex<Vec<(String, String)>>,
    }

    impl Engine for FakeEngine {
        fn config(&self) -> Config {
            test_config()
        }
        fn run(&self, plan: &Plan, text: &str) -> Result<TextResult, RunError> {
            self.seen.lock().unwrap().push((plan.system_prompt.clone(), text.to_string()));
            if let Some(gate) = &self.gate {
                let _ = gate.lock().unwrap().recv_timeout(Duration::from_secs(5));
            }
            if self.fail {
                return Err(RunError::Backend(BackendError::Transport("connection refused".into())));
            }
            Ok(TextResult { text: Some(text.to_uppercase()), html: None, backend_ms: 7 })
        }
    }

    const TOKEN: &str = "cai_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn start(engine: FakeEngine) -> (Server, SharedToken, Arc<FakeEngine>) {
        let token: SharedToken = Arc::new(RwLock::new(TOKEN.to_string()));
        let engine = Arc::new(engine);
        let server = Server::start(0, token.clone(), engine.clone()).unwrap();
        (server, token, engine)
    }

    struct Reply {
        status: u16,
        head: String,
        body: Value,
    }

    fn send_raw(addr: SocketAddr, raw: &[u8]) -> Reply {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
        let _ = stream.write_all(raw);
        let mut out = Vec::new();
        let _ = stream.read_to_end(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let status = head.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        Reply { status, head: head.to_string(), body: serde_json::from_str(body).unwrap_or(Value::Null) }
    }

    fn request(addr: SocketAddr, method: &str, path: &str, extra: &[(&str, &str)], body: &str) -> Reply {
        let mut raw = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n", addr.port());
        for (name, value) in extra {
            raw.push_str(&format!("{name}: {value}\r\n"));
        }
        if !body.is_empty() || method == "POST" {
            raw.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        raw.push_str("\r\n");
        raw.push_str(body);
        send_raw(addr, raw.as_bytes())
    }

    fn bearer() -> String {
        format!("Bearer {TOKEN}")
    }

    fn authed(addr: SocketAddr, method: &str, path: &str, body: &str) -> Reply {
        let auth = bearer();
        request(addr, method, path, &[("Authorization", &auth), ("Content-Type", "application/json")], body)
    }

    #[test]
    fn listens_on_loopback_only() {
        let (server, _, _) = start(FakeEngine::default());
        assert_eq!(server.addr().ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_ne!(server.addr().port(), 0);
        let unspecified = SocketAddr::from(([0, 0, 0, 0], server.addr().port()));
        assert_ne!(server.addr(), unspecified);
    }

    #[test]
    fn health_requires_the_token_and_answers_with_the_version() {
        let (server, _, _) = start(FakeEngine::default());
        let reply = request(server.addr(), "GET", "/v1/health", &[], "");
        assert_eq!(reply.status, 401);
        assert!(reply.head.contains("WWW-Authenticate: Bearer"));
        assert_eq!(reply.body["error"]["code"], "unauthorized");

        let reply = authed(server.addr(), "GET", "/v1/health", "");
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, json!({"ok": true, "app": "clipboard-ai", "version": VERSION}));
        assert!(reply.head.contains("Cache-Control: no-store"));
        assert!(!reply.head.to_ascii_lowercase().contains("access-control-allow"));
    }

    #[test]
    fn wrong_or_malformed_tokens_are_refused() {
        let (server, _, _) = start(FakeEngine::default());
        let wrong_last = format!("Bearer {}0", &TOKEN[..TOKEN.len() - 1]);
        let cases = [
            wrong_last,
            format!("Bearer {TOKEN}x"),
            format!("Basic {TOKEN}"),
            TOKEN.to_string(),
            "Bearer ".to_string(),
            format!("Bearer {}", &TOKEN[..10]),
        ];
        for auth in cases {
            let reply = request(server.addr(), "GET", "/v1/health", &[("Authorization", &auth)], "");
            assert_eq!(reply.status, 401, "{auth}");
        }
        let auth = bearer();
        let reply =
            request(server.addr(), "GET", "/v1/health", &[("Authorization", &auth), ("Authorization", &auth)], "");
        assert_eq!(reply.status, 401, "two Authorization headers are refused");
        let lower = format!("bearer {TOKEN}");
        let reply = request(server.addr(), "GET", "/v1/health", &[("authorization", &lower)], "");
        assert_eq!(reply.status, 200, "the scheme is case-insensitive");
    }

    #[test]
    fn a_regenerated_token_takes_effect_without_a_restart() {
        let (server, token, _) = start(FakeEngine::default());
        *token.write().unwrap() = "cai_new".into();
        assert_eq!(authed(server.addr(), "GET", "/v1/health", "").status, 401);
        let reply = request(server.addr(), "GET", "/v1/health", &[("Authorization", "Bearer cai_new")], "");
        assert_eq!(reply.status, 200);
    }

    #[test]
    fn browser_requests_and_foreign_hosts_are_refused_before_auth() {
        let (server, _, _) = start(FakeEngine::default());
        let auth = bearer();
        for header in ["Origin", "Sec-Fetch-Site", "Sec-Fetch-Mode", "Sec-Fetch-Dest"] {
            let reply =
                request(server.addr(), "GET", "/v1/health", &[("Authorization", &auth), (header, "x")], "");
            assert_eq!(reply.status, 403, "{header}");
            assert_eq!(reply.body["error"]["code"], "browser-refused");
        }
        let reply = request(server.addr(), "OPTIONS", "/v1/run", &[("Origin", "https://evil.example")], "");
        assert_eq!(reply.status, 403);
        assert!(!reply.head.to_ascii_lowercase().contains("access-control"));

        let raw = format!("GET /v1/health HTTP/1.1\r\nHost: evil.example:{}\r\nAuthorization: {auth}\r\n\r\n", server.addr().port());
        let reply = send_raw(server.addr(), raw.as_bytes());
        assert_eq!(reply.status, 403);
        assert_eq!(reply.body["error"]["code"], "host-refused");

        let raw = format!("GET /v1/health HTTP/1.1\r\nAuthorization: {auth}\r\n\r\n");
        assert_eq!(send_raw(server.addr(), raw.as_bytes()).status, 400);

        let raw = format!("GET /v1/health HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: {auth}\r\n\r\n", server.addr().port());
        assert_eq!(send_raw(server.addr(), raw.as_bytes()).status, 200);
    }

    #[test]
    fn lists_commands_without_secrets() {
        let (server, _, _) = start(FakeEngine::default());
        let reply = authed(server.addr(), "GET", "/v1/commands", "");
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body["ok"], true);
        assert_eq!(reply.body["commands"][0]["id"], "fix");
        assert_eq!(reply.body["backends"][0]["kind"], "http");
        let text = reply.body.to_string();
        assert!(!text.contains("sk-very-secret") && !text.contains("secret-path"), "{text}");
    }

    #[test]
    fn runs_a_command_and_returns_the_result() {
        let (server, _, engine) = start(FakeEngine::default());
        let reply = authed(server.addr(), "POST", "/v1/run", r#"{"command":"Fix grammar","text":"hello"}"#);
        assert_eq!(reply.status, 200, "{}", reply.body);
        assert_eq!(reply.body["text"], "HELLO");
        assert_eq!(reply.body["sent"], false);
        assert_eq!(reply.body["command"], json!({"id": "fix", "name": "Fix grammar"}));
        assert_eq!(reply.body["backend"], json!({"id": "hook", "name": "Hook"}));
        assert_eq!(engine.seen.lock().unwrap()[0], ("Fix.".to_string(), "hello".to_string()));

        let reply = authed(server.addr(), "POST", "/v1/run", r#"{"text":"hi","prompt":"Shout."}"#);
        assert_eq!(reply.status, 200, "{}", reply.body);
        assert_eq!(reply.body["command"], Value::Null);
        assert_eq!(engine.seen.lock().unwrap()[1].0, "Shout.");
    }

    #[test]
    fn run_errors_map_to_clear_statuses() {
        let (server, _, _) = start(FakeEngine::default());
        let cases: &[(&str, u16, &str)] = &[
            (r#"{"command":"nope","text":"x"}"#, 404, "unknown-command"),
            (r#"{"command":"fix","backend":"nope","text":"x"}"#, 404, "unknown-backend"),
            (r#"{"text":"x"}"#, 400, "nothing-to-run"),
            (r#"{"command":"fix","text":"   "}"#, 400, "empty-text"),
            (r#"{"command":"fix"}"#, 400, "invalid-json"),
            (r#"{"command":"fix","text":"x","extra":1}"#, 400, "invalid-json"),
            (r#"{"command":"fix","text":5}"#, 400, "invalid-json"),
            ("not json", 400, "invalid-json"),
        ];
        for (body, status, code) in cases {
            let reply = authed(server.addr(), "POST", "/v1/run", body);
            assert_eq!(reply.status, *status, "{body}: {}", reply.body);
            assert_eq!(reply.body["error"]["code"], *code, "{body}");
        }

        let (failing, _, _) = start(FakeEngine { fail: true, ..Default::default() });
        let reply = authed(failing.addr(), "POST", "/v1/run", r#"{"command":"fix","text":"x"}"#);
        assert_eq!(reply.status, 502);
        assert_eq!(reply.body["error"]["code"], "backend-error");
        let message = reply.body["error"]["message"].as_str().unwrap();
        assert!(message.contains("could not be reached"), "{message}");
        assert!(!message.contains("connection refused"), "transport details stay out of API answers");
    }

    #[test]
    fn run_requires_a_json_content_type() {
        let (server, _, _) = start(FakeEngine::default());
        let auth = bearer();
        for content_type in [None, Some("text/plain"), Some("application/x-www-form-urlencoded")] {
            let mut headers = vec![("Authorization", auth.as_str())];
            if let Some(ct) = content_type {
                headers.push(("Content-Type", ct));
            }
            let reply = request(server.addr(), "POST", "/v1/run", &headers, r#"{"command":"fix","text":"x"}"#);
            assert_eq!(reply.status, 415, "{content_type:?}");
        }
        let reply = request(
            server.addr(),
            "POST",
            "/v1/run",
            &[("Authorization", &auth), ("Content-Type", "Application/JSON; charset=utf-8")],
            r#"{"command":"fix","text":"x"}"#,
        );
        assert_eq!(reply.status, 200);
    }

    #[test]
    fn methods_and_paths_are_checked() {
        let (server, _, _) = start(FakeEngine::default());
        let reply = authed(server.addr(), "GET", "/v1/run", "");
        assert_eq!(reply.status, 405);
        assert!(reply.head.contains("Allow: POST"));
        assert_eq!(authed(server.addr(), "POST", "/v1/health", "").status, 405);
        assert_eq!(authed(server.addr(), "DELETE", "/v1/commands", "").status, 405);
        assert_eq!(authed(server.addr(), "GET", "/v1/nothing", "").status, 404);
        assert_eq!(authed(server.addr(), "GET", "/v1/health?x=1", "").status, 200);
        assert_eq!(request(server.addr(), "GET", "/v1/nothing", &[], "").status, 401, "auth comes before routing");
    }

    #[test]
    fn oversized_bodies_and_fields_are_refused() {
        let (server, _, engine) = start(FakeEngine::default());
        let auth = bearer();
        let raw = format!(
            "POST /v1/run HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: {auth}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            server.addr().port(),
            MAX_BODY_BYTES + 1
        );
        let reply = send_raw(server.addr(), raw.as_bytes());
        assert_eq!(reply.status, 413);
        assert_eq!(reply.body["error"]["code"], "body-too-large");

        let long_text = "a".repeat(MAX_TEXT_CHARS + 1);
        let body = json!({"command": "fix", "text": long_text}).to_string();
        let reply = authed(server.addr(), "POST", "/v1/run", &body);
        assert_eq!(reply.status, 413);
        assert_eq!(reply.body["error"]["code"], "field-too-large");

        let body = json!({"text": "x", "prompt": "p".repeat(MAX_PROMPT_CHARS + 1)}).to_string();
        assert_eq!(authed(server.addr(), "POST", "/v1/run", &body).status, 413);
        let body = json!({"command": "c".repeat(MAX_NAME_CHARS + 1), "text": "x"}).to_string();
        assert_eq!(authed(server.addr(), "POST", "/v1/run", &body).status, 413);
        assert!(engine.seen.lock().unwrap().is_empty(), "nothing oversized reached the backend");
    }

    #[test]
    fn malformed_framing_is_refused() {
        let (server, _, _) = start(FakeEngine::default());
        let port = server.addr().port();
        let auth = bearer();
        let huge_header = format!(
            "GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Pad: {}\r\n\r\n",
            "a".repeat(MAX_HEADER_BYTES + 10)
        );
        assert_eq!(send_raw(server.addr(), huge_header.as_bytes()).status, 431);

        let mut many = format!("GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
        for i in 0..MAX_HEADERS + 1 {
            many.push_str(&format!("X-{i}: 1\r\n"));
        }
        many.push_str("\r\n");
        assert_eq!(send_raw(server.addr(), many.as_bytes()).status, 431);

        let chunked = format!(
            "POST /v1/run HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: {auth}\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"
        );
        assert_eq!(send_raw(server.addr(), chunked.as_bytes()).status, 501);

        let two_lengths = format!(
            "POST /v1/run HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 2\r\nContent-Length: 3\r\n\r\n{{}}"
        );
        assert_eq!(send_raw(server.addr(), two_lengths.as_bytes()).status, 400);

        for bad in [
            "GET /v1/health\r\n\r\n".to_string(),
            "GET http://x/v1/health HTTP/1.1\r\n\r\n".to_string(),
            "get /v1/health HTTP/1.1\r\n\r\n".to_string(),
            "GET /v1/health HTTP/2\r\n\r\n".to_string(),
            format!("GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nBad Header: x\r\n\r\n"),
            format!("POST /v1/run HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: -1\r\n\r\n"),
            format!("POST /v1/run HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 10\r\n\r\nabc"),
        ] {
            let mut stream = TcpStream::connect(server.addr()).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
            stream.write_all(bad.as_bytes()).unwrap();
            let _ = stream.shutdown(Shutdown::Write);
            let mut out = String::new();
            let _ = stream.read_to_string(&mut out);
            assert!(out.starts_with("HTTP/1.1 400 "), "{bad:?} -> {out:?}");
        }
    }

    #[test]
    fn concurrent_runs_are_capped() {
        let (tx, rx) = mpsc::channel();
        let (server, _, engine) = start(FakeEngine { gate: Some(Mutex::new(rx)), ..Default::default() });
        let addr = server.addr();
        let mut handles = Vec::new();
        for _ in 0..MAX_CONCURRENT_RUNS {
            handles.push(thread::spawn(move || authed(addr, "POST", "/v1/run", r#"{"command":"fix","text":"x"}"#).status));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while engine.seen.lock().unwrap().len() < 1 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(300));
        let reply = authed(addr, "POST", "/v1/run", r#"{"command":"fix","text":"x"}"#);
        assert_eq!(reply.status, 429, "{}", reply.body);
        assert!(reply.head.contains("Retry-After: 1"));
        for _ in 0..MAX_CONCURRENT_RUNS {
            let _ = tx.send(());
        }
        for handle in handles {
            assert_eq!(handle.join().unwrap(), 200);
        }
    }

    #[test]
    fn a_stalled_client_gets_a_timeout_not_a_hung_thread() {
        let (server, _, _) = start(FakeEngine::default());
        let started = Instant::now();
        let mut stream = TcpStream::connect(server.addr()).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        stream.write_all(b"GET /v1/health HTTP/1.1\r\n").unwrap();
        let mut out = String::new();
        let _ = stream.read_to_string(&mut out);
        assert!(out.starts_with("HTTP/1.1 408 "), "{out:?}");
        assert!(started.elapsed() < HEAD_DEADLINE + Duration::from_secs(2));
    }

    #[test]
    fn stopping_frees_the_port() {
        let (server, _, _) = start(FakeEngine::default());
        let addr = server.addr();
        server.stop();
        let again = TcpListener::bind(addr).expect("the port must be free after stop");
        drop(again);
    }

    #[test]
    fn token_comparison_and_generation() {
        assert!(tokens_match(b"abc", b"abc"));
        assert!(!tokens_match(b"abc", b"abd"));
        assert!(!tokens_match(b"abc", b"abcd"));
        assert!(!tokens_match(b"", b"a"));
        let a = generate_token().unwrap();
        let b = generate_token().unwrap();
        assert!(is_well_formed_token(&a), "{a}");
        assert_ne!(a, b);
        assert!(!is_well_formed_token("cai_short"));
        assert!(!is_well_formed_token(&a.replace("cai_", "xyz_")));
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cai-localapi-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn engine() -> Arc<dyn Engine> {
        Arc::new(FakeEngine::default())
    }

    #[test]
    fn the_manager_starts_stops_and_keeps_its_token() {
        let dir = temp_dir("manager");
        let api = LocalApi::new(dir.clone());
        assert_eq!(api.status(), LocalApiStatus {
            enabled: false, running: false, url: None, port: None, token: None, error: None
        });
        assert!(!dir.join(STATE_FILE).exists(), "nothing is written while the API was never enabled");

        assert_eq!(api.apply(LocalApiSettings { enabled: true, port: None }, engine), None);
        let first = api.status();
        assert!(first.running && first.enabled);
        let token = first.token.clone().unwrap();
        assert!(is_well_formed_token(&token));
        let state = read_state(&dir).unwrap();
        assert_eq!(state.token, token);
        assert_eq!(state.url, first.url);

        assert_eq!(api.apply(LocalApiSettings { enabled: true, port: None }, engine), None);
        assert_eq!(api.status().port, first.port, "re-applying the same settings keeps the server");

        let regenerated = api.regenerate_token().unwrap();
        assert_ne!(regenerated.token, first.token);
        assert_eq!(read_state(&dir).unwrap().token, regenerated.token.clone().unwrap());
        let addr = SocketAddr::from(([127, 0, 0, 1], first.port.unwrap()));
        assert_eq!(authed(addr, "GET", "/v1/health", "").status, 401);

        api.apply(LocalApiSettings { enabled: false, port: None }, engine);
        let off = api.status();
        assert!(!off.running && off.token.is_none());
        assert_eq!(read_state(&dir).unwrap().url, None);

        let again = LocalApi::new(dir.clone());
        again.apply(LocalApiSettings { enabled: true, port: None }, engine);
        assert_eq!(again.status().token, regenerated.token, "the token survives a restart");
        again.shutdown();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn backend_details_never_reach_api_clients() {
        use crate::backend::BackendError;
        let cases = [
            RunError::Backend(BackendError::Transport("error sending request for url (http://h/secret-hook)".into())),
            RunError::Backend(BackendError::BadResponse("HTTP 500: body with secret-hook".into())),
            RunError::Backend(BackendError::Misconfigured("secret-hook".into())),
        ];
        for error in cases {
            assert!(!public_run_error(&error).contains("secret-hook"), "{error:?}");
        }
        assert_eq!(public_run_error(&RunError::NoSentinel), RunError::NoSentinel.to_string());
    }

    #[test]
    fn a_url_left_behind_by_a_crash_is_cleared_at_startup() {
        let dir = temp_dir("stale-url");
        write_state(&dir, &StateFile { token: TOKEN.into(), url: Some("http://127.0.0.1:5555".into()) }).unwrap();
        let api = LocalApi::new(dir.clone());
        assert_eq!(read_state(&dir).unwrap(), StateFile { token: TOKEN.into(), url: None });
        assert!(!api.status().running);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_busy_fixed_port_is_reported_not_fatal() {
        let dir = temp_dir("busy-port");
        let blocker = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = blocker.local_addr().unwrap().port();
        let api = LocalApi::new(dir.clone());
        let problem = api.apply(LocalApiSettings { enabled: true, port: Some(port) }, engine);
        let problem = problem.expect("a taken port must be reported");
        assert!(problem.contains(&port.to_string()), "{problem}");
        let status = api.status();
        assert!(!status.running);
        assert_eq!(status.error.as_deref(), Some(problem.as_str()));
        drop(blocker);
        assert_eq!(api.apply(LocalApiSettings { enabled: true, port: Some(port) }, engine), None);
        assert_eq!(api.status().port, Some(port));
        api.shutdown();
        let _ = fs::remove_dir_all(&dir);
    }
}
