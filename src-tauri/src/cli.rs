use std::{
    fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
};

use serde_json::{json, Value};

use crate::config::{self, Config};
use crate::history::{self, HistoryEntry, Outcome};
use crate::textrun::{self, Plan, TextResult};

pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_CONFIG: i32 = 3;

pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
pub const DEFAULT_HISTORY_LIMIT: usize = 20;
pub const CONFIG_DIR_ENV: &str = "CLIPBOARD_AI_CONFIG_DIR";
const APP_IDENTIFIER: &str = "com.clipboardai.app";
const VERSION: &str = env!("CARGO_PKG_VERSION");

const MAIN_HELP: &str = "\
Run your Clipboard AI commands from a terminal or a script.

Usage:
  clipboard-ai run [<command>] [options]   Run a command on text from stdin
  clipboard-ai list [--json]               Show commands and backends
  clipboard-ai history [--limit <n>] [--json]
                                           Show recent runs from the app
  clipboard-ai help [<subcommand>]         Show help for a subcommand

Options:
  --config-dir <path>  Read config from this folder instead of
                       %APPDATA%\\com.clipboardai.app
                       (or set the CLIPBOARD_AI_CONFIG_DIR variable)
  -h, --help           Show help
  -V, --version        Show the version

The tool only reads your config. It works whether or not the app is running.

Exit codes:
  0  success
  1  the backend failed or returned nothing usable
  2  wrong usage or unusable input
  3  the config could not be read, or a command or backend was not found

Examples:
  Get-Content notes.txt | clipboard-ai run \"Fix grammar\"
  clipboard-ai run fix-grammar --text \"teh quick fox\" --json
  clipboard-ai run --prompt \"Translate to German\" --text \"Good morning\"
";

const RUN_HELP: &str = "\
Usage: clipboard-ai run [<command>] [options]

Reads text from stdin (or --text), sends it through a command the same way the
hotkey does, and prints the result. Nothing is pasted and nothing is written.

Arguments:
  <command>            Command name or id. Optional when --prompt is given.

Options:
  --text <text>        Use this text instead of reading stdin
  --backend <name>     Use this backend (name or id) instead of the command's
  --prompt <text>      Use this instruction instead of the command's prompt
  --html               Print HTML when the command returns formatted text
  --json               Print one JSON object instead of plain text
  --config-dir <path>  Read config from this folder

Input is read as UTF-8 (UTF-16 with a byte order mark also works), up to 1 MiB.
Without a command, --prompt runs on the only backend, or the one named
with --backend.
";

const LIST_HELP: &str = "\
Usage: clipboard-ai list [--json]

Shows the configured commands and backends. API keys and URLs are never shown.

Options:
  --json               Print one JSON object instead of tables
  --config-dir <path>  Read config from this folder
";

const HISTORY_HELP: &str = "\
Usage: clipboard-ai history [--limit <n>] [--json]

Shows the most recent runs the app recorded, newest first. Runs made with
this tool are not recorded.

Options:
  --limit <n>          How many runs to show, 1 to 100 (default 20)
  --json               Print one JSON object with the full entries
  --config-dir <path>  Read config from this folder
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Run,
    List,
    History,
}

impl Topic {
    fn from_name(name: &str) -> Option<Topic> {
        match name {
            "run" => Some(Topic::Run),
            "list" => Some(Topic::List),
            "history" => Some(Topic::History),
            _ => None,
        }
    }

    fn help(topic: Option<Topic>) -> &'static str {
        match topic {
            None => MAIN_HELP,
            Some(Topic::Run) => RUN_HELP,
            Some(Topic::List) => LIST_HELP,
            Some(Topic::History) => HISTORY_HELP,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunArgs {
    pub command: Option<String>,
    pub text: Option<String>,
    pub backend: Option<String>,
    pub prompt: Option<String>,
    pub html: bool,
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Help(Option<Topic>),
    Version,
    Run(RunArgs),
    List { json: bool },
    History { limit: usize, json: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub action: Action,
    pub config_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError {
    pub message: String,
    pub topic: Option<Topic>,
}

fn usage(message: impl Into<String>, topic: Option<Topic>) -> UsageError {
    UsageError { message: message.into(), topic }
}

const VALUE_OPTIONS: &[&str] = &["text", "backend", "prompt", "limit", "config-dir"];
const FLAG_OPTIONS: &[&str] = &["json", "html", "help", "version"];

pub fn parse_args(args: &[String]) -> Result<Invocation, UsageError> {
    let mut positionals: Vec<String> = Vec::new();
    let mut options: Vec<(String, Option<String>)> = Vec::new();
    let mut only_positionals = false;
    let mut iter = args.iter();

    while let Some(arg) = iter.next() {
        if only_positionals || arg == "-" || !arg.starts_with('-') {
            positionals.push(arg.clone());
            continue;
        }
        if arg == "--" {
            only_positionals = true;
            continue;
        }
        let (name, inline_value) = match arg.as_str() {
            "-h" => ("help".to_string(), None),
            "-V" => ("version".to_string(), None),
            long if long.starts_with("--") => match long[2..].split_once('=') {
                Some((name, value)) => (name.to_string(), Some(value.to_string())),
                None => (long[2..].to_string(), None),
            },
            other => {
                let topic = positionals.first().and_then(|p| Topic::from_name(p));
                return Err(usage(format!("unknown option {other}"), topic));
            }
        };
        let topic = positionals.first().and_then(|p| Topic::from_name(p));
        if VALUE_OPTIONS.contains(&name.as_str()) {
            let value = match inline_value {
                Some(v) => v,
                None => match iter.next() {
                    Some(v) => v.clone(),
                    None => return Err(usage(format!("--{name} needs a value"), topic)),
                },
            };
            options.push((name, Some(value)));
        } else if FLAG_OPTIONS.contains(&name.as_str()) {
            if inline_value.is_some() {
                return Err(usage(format!("--{name} does not take a value"), topic));
            }
            options.push((name, None));
        } else {
            return Err(usage(format!("unknown option --{name}"), topic));
        }
    }

    let mut config_dir = None;
    let mut wants_help = false;
    let mut wants_version = false;
    let mut rest: Vec<(String, Option<String>)> = Vec::new();
    for (name, value) in options {
        match name.as_str() {
            "help" => wants_help = true,
            "version" => wants_version = true,
            "config-dir" => {
                if config_dir.is_some() {
                    return Err(usage("--config-dir was given twice", None));
                }
                let value = value.unwrap_or_default();
                if value.trim().is_empty() {
                    return Err(usage("--config-dir needs a folder", None));
                }
                config_dir = Some(PathBuf::from(value));
            }
            _ => rest.push((name, value)),
        }
    }

    let mut positionals = positionals.into_iter();
    let subcommand = positionals.next();
    let topic = match subcommand.as_deref() {
        None => None,
        Some("help") => {
            let wanted = positionals.next();
            if let Some(extra) = positionals.next() {
                return Err(usage(format!("unexpected argument {extra:?}"), None));
            }
            let topic = match wanted.as_deref() {
                None => None,
                Some(name) => match Topic::from_name(name) {
                    Some(topic) => Some(topic),
                    None => return Err(usage(format!("there is no subcommand called {name:?}"), None)),
                },
            };
            return Ok(Invocation { action: Action::Help(topic), config_dir });
        }
        Some(name) => match Topic::from_name(name) {
            Some(topic) => Some(topic),
            None => return Err(usage(format!("there is no subcommand called {name:?}"), None)),
        },
    };

    if wants_help {
        return Ok(Invocation { action: Action::Help(topic), config_dir });
    }
    if wants_version {
        return Ok(Invocation { action: Action::Version, config_dir });
    }
    let Some(topic) = topic else {
        return Err(usage("name a subcommand", None));
    };

    let mut seen: Vec<&str> = Vec::new();
    for (name, _) in &rest {
        if seen.contains(&name.as_str()) {
            return Err(usage(format!("--{name} was given twice"), Some(topic)));
        }
        seen.push(name);
    }
    let allowed: &[&str] = match topic {
        Topic::Run => &["text", "backend", "prompt", "html", "json"],
        Topic::List => &["json"],
        Topic::History => &["limit", "json"],
    };
    if let Some((name, _)) = rest.iter().find(|(name, _)| !allowed.contains(&name.as_str())) {
        let sub = match topic {
            Topic::Run => "run",
            Topic::List => "list",
            Topic::History => "history",
        };
        return Err(usage(format!("{sub} does not take --{name}"), Some(topic)));
    }
    let value_of = |wanted: &str| {
        rest.iter().find(|(name, _)| name == wanted).and_then(|(_, value)| value.clone())
    };
    let flag = |wanted: &str| rest.iter().any(|(name, _)| name == wanted);
    let extra: Vec<String> = positionals.collect();

    let action = match topic {
        Topic::Run => {
            let mut extra = extra.into_iter();
            let command = extra.next();
            if let Some(more) = extra.next() {
                return Err(usage(
                    format!("unexpected argument {more:?}; put a command name with spaces in quotes"),
                    Some(topic),
                ));
            }
            if command.as_deref().is_some_and(|c| c.trim().is_empty()) {
                return Err(usage("the command name is empty", Some(topic)));
            }
            let prompt = value_of("prompt");
            if command.is_none() && prompt.is_none() {
                return Err(usage("name a command, or give --prompt", Some(topic)));
            }
            Action::Run(RunArgs {
                command,
                text: value_of("text"),
                backend: value_of("backend"),
                prompt,
                html: flag("html"),
                json: flag("json"),
            })
        }
        Topic::List => {
            if let Some(more) = extra.first() {
                return Err(usage(format!("unexpected argument {more:?}"), Some(topic)));
            }
            Action::List { json: flag("json") }
        }
        Topic::History => {
            if let Some(more) = extra.first() {
                return Err(usage(format!("unexpected argument {more:?}"), Some(topic)));
            }
            let limit = match value_of("limit") {
                None => DEFAULT_HISTORY_LIMIT,
                Some(raw) => match raw.trim().parse::<usize>() {
                    Ok(n) if (1..=history::MAX_ENTRIES).contains(&n) => n,
                    _ => {
                        return Err(usage(
                            format!("--limit must be a number from 1 to {}", history::MAX_ENTRIES),
                            Some(topic),
                        ))
                    }
                },
            };
            Action::History { limit, json: flag("json") }
        }
    };
    Ok(Invocation { action, config_dir })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub exit: i32,
    pub code: &'static str,
    pub message: String,
}

impl CliError {
    fn new(exit: i32, code: &'static str, message: impl Into<String>) -> Self {
        Self { exit, code, message: message.into() }
    }
}

pub struct Io<'a> {
    pub stdin: &'a mut dyn Read,
    pub stdin_is_terminal: bool,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

pub struct Env {
    pub config_dir: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
}

impl Env {
    pub fn from_process() -> Self {
        Env {
            config_dir: std::env::var_os(CONFIG_DIR_ENV)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            appdata: std::env::var_os("APPDATA").filter(|v| !v.is_empty()).map(PathBuf::from),
        }
    }
}

pub fn main() -> i32 {
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(arg) => args.push(arg),
            Err(_) => {
                eprintln!("clipboard-ai: an argument is not valid Unicode text");
                return EXIT_USAGE;
            }
        }
    }
    let stdin = io::stdin();
    let stdin_is_terminal = stdin.is_terminal();
    let mut stdin = stdin.lock();
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let code = run(
        &args,
        &Env::from_process(),
        Io { stdin: &mut stdin, stdin_is_terminal, stdout: &mut stdout, stderr: &mut stderr },
    );
    let _ = stdout.flush();
    code
}

pub fn run(args: &[String], env: &Env, io: Io<'_>) -> i32 {
    let invocation = match parse_args(args) {
        Ok(invocation) => invocation,
        Err(e) => {
            let hint = match e.topic {
                Some(Topic::Run) => "clipboard-ai help run",
                Some(Topic::List) => "clipboard-ai help list",
                Some(Topic::History) => "clipboard-ai help history",
                None => "clipboard-ai --help",
            };
            let _ = writeln!(io.stderr, "clipboard-ai: {}", e.message);
            let _ = writeln!(io.stderr, "Run `{hint}` to see how to use it.");
            return EXIT_USAGE;
        }
    };

    let json_mode = match &invocation.action {
        Action::Run(args) => args.json,
        Action::List { json } | Action::History { json, .. } => *json,
        Action::Help(_) | Action::Version => false,
    };
    let result = execute(&invocation, env, io.stdin, io.stdin_is_terminal);
    match result {
        Ok(Output { stdout, stderr }) => {
            let _ = io.stdout.write_all(stdout.as_bytes());
            if let Some(note) = stderr {
                let _ = writeln!(io.stderr, "clipboard-ai: {note}");
            }
            EXIT_OK
        }
        Err(e) => {
            if json_mode {
                let body = json!({"ok": false, "error": {"code": e.code, "message": e.message}});
                let _ = writeln!(io.stdout, "{body}");
            }
            let _ = writeln!(io.stderr, "clipboard-ai: {}", e.message);
            e.exit
        }
    }
}

struct Output {
    stdout: String,
    stderr: Option<String>,
}

impl Output {
    fn out(text: String) -> Self {
        Output { stdout: text, stderr: None }
    }
}

fn execute(
    invocation: &Invocation,
    env: &Env,
    stdin: &mut dyn Read,
    stdin_is_terminal: bool,
) -> Result<Output, CliError> {
    match &invocation.action {
        Action::Help(topic) => {
            let mut text = String::new();
            if topic.is_none() {
                text.push_str(&format!("clipboard-ai {VERSION}\n"));
            }
            text.push_str(Topic::help(*topic));
            Ok(Output::out(text))
        }
        Action::Version => Ok(Output::out(format!("clipboard-ai {VERSION}\n"))),
        Action::List { json } => {
            let dir = config_dir(invocation, env)?;
            let config = load_config(&dir)?;
            Ok(Output::out(if *json { list_json(&config) } else { list_text(&config) }))
        }
        Action::History { limit, json } => {
            let dir = config_dir(invocation, env)?;
            let entries = load_history(&dir)?;
            let shown = &entries[..entries.len().min(*limit)];
            Ok(Output::out(if *json { history_json(shown) } else { history_text(shown) }))
        }
        Action::Run(args) => {
            let dir = config_dir(invocation, env)?;
            let config = load_config(&dir)?;
            let plan = textrun::resolve(
                &config,
                args.command.as_deref(),
                args.backend.as_deref(),
                args.prompt.as_deref(),
            )
            .map_err(|e| {
                let exit = match e {
                    textrun::ResolveError::EmptyPrompt | textrun::ResolveError::NothingToRun => {
                        EXIT_USAGE
                    }
                    _ => EXIT_CONFIG,
                };
                CliError::new(exit, e.code(), e.to_string())
            })?;
            let text = read_input(args.text.as_deref(), stdin, stdin_is_terminal)?;
            let result = textrun::run_plan(&plan, &text).map_err(|e| {
                CliError::new(EXIT_FAILED, e.code(), format!("{} failed: {e}", plan.backend.name))
            })?;
            Ok(run_output(&plan, &result, args))
        }
    }
}

pub fn default_config_dir(env: &Env) -> Option<PathBuf> {
    if let Some(dir) = &env.config_dir {
        return Some(dir.clone());
    }
    env.appdata.as_ref().map(|appdata| appdata.join(APP_IDENTIFIER))
}

fn config_dir(invocation: &Invocation, env: &Env) -> Result<PathBuf, CliError> {
    invocation.config_dir.clone().or_else(|| default_config_dir(env)).ok_or_else(|| {
        CliError::new(
            EXIT_CONFIG,
            "no-config-dir",
            format!("cannot find the config folder; set {CONFIG_DIR_ENV} or pass --config-dir"),
        )
    })
}

fn strip_bom(raw: &str) -> &str {
    raw.strip_prefix('\u{feff}').unwrap_or(raw)
}

fn load_config(dir: &Path) -> Result<Config, CliError> {
    let path = dir.join("config.json");
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(CliError::new(
                EXIT_CONFIG,
                "no-config",
                format!(
                    "there is no config file at {}; start Clipboard AI once to create it",
                    path.display()
                ),
            ))
        }
        Err(e) => {
            return Err(CliError::new(
                EXIT_CONFIG,
                "config-unreadable",
                format!("cannot read {}: {e}", path.display()),
            ))
        }
    };
    config::parse(strip_bom(&raw)).map_err(|e| {
        CliError::new(
            EXIT_CONFIG,
            "config-invalid",
            format!("the config file {} cannot be used: {e}", path.display()),
        )
    })
}

fn load_history(dir: &Path) -> Result<Vec<HistoryEntry>, CliError> {
    let path = dir.join(history::FILE_NAME);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(CliError::new(
                EXIT_CONFIG,
                "history-unreadable",
                format!("cannot read {}: {e}", path.display()),
            ))
        }
    };
    serde_json::from_str(strip_bom(&raw)).map_err(|e| {
        CliError::new(
            EXIT_CONFIG,
            "history-invalid",
            format!("the history file {} cannot be read: {e}", path.display()),
        )
    })
}

pub fn decode_input(bytes: &[u8]) -> Result<String, CliError> {
    let not_text = || {
        CliError::new(EXIT_USAGE, "input-not-text", "the input is not valid UTF-8 or UTF-16 text")
    };
    let decode_utf16 = |body: &[u8], little: bool| -> Result<String, CliError> {
        if body.len() % 2 != 0 {
            return Err(not_text());
        }
        let units: Vec<u16> = body
            .chunks_exact(2)
            .map(|pair| {
                if little {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            })
            .collect();
        String::from_utf16(&units).map_err(|_| not_text())
    };
    if let Some(body) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(body.to_vec()).map_err(|_| not_text());
    }
    if let Some(body) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(body, true);
    }
    if let Some(body) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(body, false);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| not_text())
}

fn read_input(
    text: Option<&str>,
    stdin: &mut dyn Read,
    stdin_is_terminal: bool,
) -> Result<String, CliError> {
    let raw = match text {
        Some(text) => {
            if text.len() > MAX_INPUT_BYTES {
                return Err(too_large());
            }
            text.to_string()
        }
        None => {
            if stdin_is_terminal {
                return Err(CliError::new(
                    EXIT_USAGE,
                    "no-input",
                    "no input text; pipe text into clipboard-ai run, or pass --text",
                ));
            }
            let mut bytes = Vec::new();
            stdin
                .take(MAX_INPUT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| CliError::new(EXIT_USAGE, "input-unreadable", format!("cannot read stdin: {e}")))?;
            if bytes.len() > MAX_INPUT_BYTES {
                return Err(too_large());
            }
            decode_input(&bytes)?
        }
    };
    let trimmed = raw.trim_end_matches(['\r', '\n']);
    if trimmed.trim().is_empty() {
        return Err(CliError::new(EXIT_USAGE, "empty-input", "the input text is empty"));
    }
    Ok(trimmed.to_string())
}

fn too_large() -> CliError {
    CliError::new(EXIT_USAGE, "input-too-large", "the input is larger than 1 MiB")
}

fn with_newline(mut text: String) -> String {
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn run_output(plan: &Plan, result: &TextResult, args: &RunArgs) -> Output {
    if args.json {
        let command = match (&plan.command_id, &plan.command_name) {
            (Some(id), Some(name)) => json!({"id": id, "name": name}),
            _ => Value::Null,
        };
        let body = json!({
            "ok": true,
            "command": command,
            "backend": {"id": plan.backend.id, "name": plan.backend.name},
            "sent": result.text.is_none(),
            "text": result.text,
            "html": result.html,
            "backendMs": result.backend_ms,
        });
        return Output::out(format!("{body}\n"));
    }
    match (&result.text, &result.html) {
        (None, _) => Output {
            stdout: String::new(),
            stderr: Some(format!("sent to {}; this backend returns no text", plan.backend.name)),
        },
        (Some(_), Some(html)) if args.html => Output::out(with_newline(html.clone())),
        (Some(text), _) => Output::out(with_newline(text.clone())),
    }
}

fn table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|i| rows.iter().filter_map(|r| r.get(i)).map(|c| c.chars().count()).max().unwrap_or(0))
        .collect();
    let mut out = String::new();
    for row in rows {
        let mut line = String::from("  ");
        for (i, cell) in row.iter().enumerate() {
            line.push_str(cell);
            if i + 1 < row.len() {
                let pad = widths[i] - cell.chars().count() + 2;
                line.push_str(&" ".repeat(pad));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn list_text(config: &Config) -> String {
    let mut out = String::from("Commands\n");
    if config.commands.is_empty() {
        out.push_str("  (none)\n");
    } else {
        let mut rows = vec![vec!["NAME".into(), "ID".into(), "HOTKEY".into(), "BACKEND".into()]];
        for c in &config.commands {
            let backend = config.backend(&c.backend_id).map(|b| b.name.clone()).unwrap_or_default();
            rows.push(vec![printable(&c.name), printable(&c.id), printable(&c.hotkey), printable(&backend)]);
        }
        out.push_str(&table(&rows));
    }
    out.push_str("\nBackends\n");
    if config.backends.is_empty() {
        out.push_str("  (none)\n");
    } else {
        let mut rows = vec![vec!["NAME".into(), "ID".into(), "KIND".into(), "MODEL".into()]];
        for b in textrun::public_backends(config) {
            rows.push(vec![
                printable(b.name),
                printable(b.id),
                printable(b.kind),
                printable(b.model.unwrap_or("-")),
            ]);
        }
        out.push_str(&table(&rows));
    }
    out
}

fn list_json(config: &Config) -> String {
    let mut body = textrun::listing_json(config);
    body["ok"] = json!(true);
    format!("{body}\n")
}

fn history_json(entries: &[HistoryEntry]) -> String {
    format!("{}\n", json!({"ok": true, "entries": entries}))
}

fn printable(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() && c != '\n' && c != '\t' { '\u{FFFD}' } else { c })
        .collect()
}

fn one_line(text: &str, max_chars: usize) -> String {
    let flat: String = printable(text).split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max_chars {
        return flat;
    }
    let cut: String = flat.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{}...", cut.trim_end())
}

fn duration_label(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{:.1} s", ms as f64 / 1000.0)
    }
}

fn history_text(entries: &[HistoryEntry]) -> String {
    if entries.is_empty() {
        return "No runs recorded yet.\n".to_string();
    }
    let mut out = String::new();
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let outcome = match &entry.outcome {
            Outcome::Succeeded => "ok".to_string(),
            Outcome::Aborted { reason } => format!("aborted: {}", one_line(reason, 60)),
            Outcome::Failed { error } => format!("failed: {}", one_line(error, 60)),
        };
        let duration = duration_label(entry.total_cycle_ms.unwrap_or(entry.backend_ms));
        out.push_str(&format!(
            "{}  {} - {} - {} - {}\n",
            format_timestamp(entry.timestamp_ms),
            one_line(&entry.command_name, 60),
            one_line(&entry.backend_name, 60),
            outcome,
            duration
        ));
        if let Some(input) = entry.input_text.as_deref().filter(|t| !t.trim().is_empty()) {
            out.push_str(&format!("  in   {}\n", one_line(input, 72)));
        }
        if let Some(result) = entry.result_text.as_deref().filter(|t| !t.trim().is_empty()) {
            out.push_str(&format!("  out  {}\n", one_line(result, 72)));
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Civil {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
}

fn civil_from_unix_ms(ms: u64) -> Civil {
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    Civil { year, month, day, hour: (rem / 3600) as u32, minute: ((rem % 3600) / 60) as u32 }
}

#[cfg(windows)]
fn local_civil(ms: u64) -> Option<Civil> {
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};

    let ticks = ms.checked_mul(10_000)?.checked_add(116_444_736_000_000_000)?;
    let file_time = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    unsafe {
        FileTimeToSystemTime(&file_time, &mut utc).ok()?;
        SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).ok()?;
    }
    Some(Civil {
        year: local.wYear as i64,
        month: local.wMonth as u32,
        day: local.wDay as u32,
        hour: local.wHour as u32,
        minute: local.wMinute as u32,
    })
}

#[cfg(not(windows))]
fn local_civil(_ms: u64) -> Option<Civil> {
    None
}

fn format_civil(c: Civil, suffix: &str) -> String {
    format!("{:04}-{:02}-{:02} {:02}:{:02}{suffix}", c.year, c.month, c.day, c.hour, c.minute)
}

fn format_timestamp(ms: u64) -> String {
    match local_civil(ms) {
        Some(local) => format_civil(local, ""),
        None => format_civil(civil_from_unix_ms(ms), " UTC"),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Cursor, Read as _, Write as _},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };

    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    fn parse(args: &[&str]) -> Result<Invocation, UsageError> {
        parse_args(&strings(args))
    }

    fn action(args: &[&str]) -> Action {
        parse(args).unwrap_or_else(|e| panic!("{args:?}: {}", e.message)).action
    }

    #[test]
    fn parses_a_full_run_invocation() {
        assert_eq!(
            action(&["run", "Fix grammar", "--backend", "Local", "--prompt=Shorter.", "--json", "--html"]),
            Action::Run(RunArgs {
                command: Some("Fix grammar".into()),
                text: None,
                backend: Some("Local".into()),
                prompt: Some("Shorter.".into()),
                html: true,
                json: true,
            })
        );
    }

    #[test]
    fn options_may_come_before_the_command_and_values_may_start_with_a_dash() {
        assert_eq!(
            action(&["run", "--text", "-x- y", "fix"]),
            Action::Run(RunArgs { command: Some("fix".into()), text: Some("-x- y".into()), ..Default::default() })
        );
    }

    #[test]
    fn a_prompt_alone_is_enough_to_run() {
        let Action::Run(args) = action(&["run", "--prompt", "Translate."]) else { panic!() };
        assert_eq!(args.command, None);
        assert_eq!(args.prompt.as_deref(), Some("Translate."));
    }

    #[test]
    fn double_dash_ends_option_parsing() {
        let Action::Run(args) = action(&["run", "--", "--odd name"]) else { panic!() };
        assert_eq!(args.command.as_deref(), Some("--odd name"));
    }

    #[test]
    fn help_and_version_are_recognised_everywhere() {
        assert_eq!(action(&["--help"]), Action::Help(None));
        assert_eq!(action(&["-h"]), Action::Help(None));
        assert_eq!(action(&["help"]), Action::Help(None));
        assert_eq!(action(&["help", "run"]), Action::Help(Some(Topic::Run)));
        assert_eq!(action(&["run", "--help"]), Action::Help(Some(Topic::Run)));
        assert_eq!(action(&["history", "-h"]), Action::Help(Some(Topic::History)));
        assert_eq!(action(&["--version"]), Action::Version);
        assert_eq!(action(&["-V"]), Action::Version);
    }

    #[test]
    fn list_and_history_parse_with_their_defaults() {
        assert_eq!(action(&["list"]), Action::List { json: false });
        assert_eq!(action(&["list", "--json"]), Action::List { json: true });
        assert_eq!(action(&["history"]), Action::History { limit: DEFAULT_HISTORY_LIMIT, json: false });
        assert_eq!(action(&["history", "--limit", "5", "--json"]), Action::History { limit: 5, json: true });
        assert_eq!(action(&["history", "--limit=100"]), Action::History { limit: 100, json: false });
    }

    #[test]
    fn config_dir_is_a_global_option() {
        let inv = parse(&["list", "--config-dir", "C:\\tmp\\cfg"]).unwrap();
        assert_eq!(inv.config_dir, Some(PathBuf::from("C:\\tmp\\cfg")));
        let inv = parse(&["--config-dir=D:\\x", "history"]).unwrap();
        assert_eq!(inv.config_dir, Some(PathBuf::from("D:\\x")));
    }

    #[test]
    fn usage_errors_name_the_problem() {
        let cases: &[(&[&str], &str)] = &[
            (&[], "name a subcommand"),
            (&["frobnicate"], "no subcommand called \"frobnicate\""),
            (&["run"], "name a command, or give --prompt"),
            (&["run", "Fix", "grammar"], "put a command name with spaces in quotes"),
            (&["run", "fix", "--text"], "--text needs a value"),
            (&["run", "fix", "--limit", "3"], "run does not take --limit"),
            (&["run", "fix", "--json", "--json"], "--json was given twice"),
            (&["run", "fix", "--json=yes"], "--json does not take a value"),
            (&["run", "fix", "--frob"], "unknown option --frob"),
            (&["run", "fix", "-x"], "unknown option -x"),
            (&["run", "  "], "the command name is empty"),
            (&["list", "extra"], "unexpected argument \"extra\""),
            (&["history", "--limit", "0"], "--limit must be a number from 1 to 100"),
            (&["history", "--limit", "101"], "--limit must be a number from 1 to 100"),
            (&["history", "--limit", "ten"], "--limit must be a number from 1 to 100"),
            (&["help", "nope"], "no subcommand called \"nope\""),
            (&["list", "--config-dir", ""], "--config-dir needs a folder"),
        ];
        for (args, expected) in cases {
            let err = parse(args).expect_err(&format!("{args:?} should fail"));
            assert!(err.message.contains(expected), "{args:?}: got {:?}, wanted {expected:?}", err.message);
        }
        assert_eq!(parse(&["run", "fix", "--frob"]).unwrap_err().topic, Some(Topic::Run));
    }

    #[test]
    fn decodes_utf8_with_or_without_a_bom_and_utf16_with_one() {
        assert_eq!(decode_input("Grüße".as_bytes()).unwrap(), "Grüße");
        assert_eq!(decode_input(b"\xEF\xBB\xBFhi").unwrap(), "hi");
        let mut le = vec![0xFF, 0xFE];
        le.extend("Grüße".encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(decode_input(&le).unwrap(), "Grüße");
        let mut be = vec![0xFE, 0xFF];
        be.extend("ok".encode_utf16().flat_map(|u| u.to_be_bytes()));
        assert_eq!(decode_input(&be).unwrap(), "ok");
        assert_eq!(decode_input(b"\xC3\x28").unwrap_err().code, "input-not-text");
        assert_eq!(decode_input(&[0xFF, 0xFE, 0x41]).unwrap_err().code, "input-not-text");
    }

    #[test]
    fn input_is_trimmed_of_trailing_newlines_and_bounded() {
        let mut stdin = Cursor::new(b"hello\r\n".to_vec());
        assert_eq!(read_input(None, &mut stdin, false).unwrap(), "hello");
        let mut empty = Cursor::new(b" \r\n".to_vec());
        assert_eq!(read_input(None, &mut empty, false).unwrap_err().code, "empty-input");
        let mut big = Cursor::new(vec![b'a'; MAX_INPUT_BYTES + 1]);
        assert_eq!(read_input(None, &mut big, false).unwrap_err().code, "input-too-large");
        let mut exact = Cursor::new(vec![b'a'; MAX_INPUT_BYTES]);
        assert_eq!(read_input(None, &mut exact, false).unwrap().len(), MAX_INPUT_BYTES);
        let mut unused = Cursor::new(Vec::new());
        assert_eq!(read_input(None, &mut unused, true).unwrap_err().code, "no-input");
        assert_eq!(read_input(Some("given\n"), &mut unused, true).unwrap(), "given");
    }

    #[test]
    fn terminal_control_sequences_are_neutralised_in_tables() {
        assert_eq!(printable("ok\x1b]52;c;ZXZpbA==\x07"), "ok\u{FFFD}]52;c;ZXZpbA==\u{FFFD}");
        assert_eq!(one_line("a\x1b[2Jb\u{9b}c", 40), "a\u{FFFD}[2Jb\u{FFFD}c");
    }

    #[test]
    fn utc_civil_conversion_is_correct() {
        assert_eq!(
            civil_from_unix_ms(0),
            Civil { year: 1970, month: 1, day: 1, hour: 0, minute: 0 }
        );
        assert_eq!(
            civil_from_unix_ms(1_795_132_800_000 + 13 * 3_600_000 + 7 * 60_000),
            Civil { year: 2026, month: 11, day: 20, hour: 13, minute: 7 }
        );
        assert_eq!(
            civil_from_unix_ms(951_782_400_000),
            Civil { year: 2000, month: 2, day: 29, hour: 0, minute: 0 }
        );
        assert_eq!(format_civil(civil_from_unix_ms(0), " UTC"), "1970-01-01 00:00 UTC");
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("cai-cli-test-{}-{tag}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fake_backend(reply: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_by_server = seen.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&chunk[..n]);
                            let text = String::from_utf8_lossy(&buf);
                            if let Some(head_end) = text.find("\r\n\r\n") {
                                let len = text[..head_end]
                                    .lines()
                                    .find_map(|l| {
                                        l.to_ascii_lowercase()
                                            .strip_prefix("content-length:")
                                            .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                                    })
                                    .unwrap_or(0);
                                if buf.len() >= head_end + 4 + len {
                                    break;
                                }
                            }
                        }
                    }
                }
                seen_by_server.lock().unwrap().push(String::from_utf8_lossy(&buf).into_owned());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (format!("http://{addr}/hook"), seen)
    }

    fn write_config(dir: &Path, http_url: &str, expects_response: bool) {
        let config = json!({
            "version": 2,
            "backends": [
                {"id": "hook", "name": "Local hook", "kind": "http", "baseUrl": http_url,
                 "apiKey": "secret-key-123", "timeoutMs": 10000, "expectsResponse": expects_response},
                {"id": "nowhere", "name": "Nowhere", "kind": "openai-compatible",
                 "baseUrl": "http://127.0.0.1:9/v1", "model": "m", "timeoutMs": 10000}
            ],
            "commands": [
                {"id": "fix", "name": "Fix grammar", "backendId": "hook",
                 "hotkey": "Ctrl+Alt+J", "systemPrompt": "Fix."}
            ]
        });
        fs::write(dir.join("config.json"), config.to_string()).unwrap();
    }

    struct Ran {
        code: i32,
        stdout: String,
        stderr: String,
    }

    fn run_cli(args: &[&str], dir: &Path, stdin: &str, terminal: bool) -> Ran {
        let mut input = Cursor::new(stdin.as_bytes().to_vec());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let env = Env { config_dir: Some(dir.to_path_buf()), appdata: None };
        let code = run(
            &strings(args),
            &env,
            Io { stdin: &mut input, stdin_is_terminal: terminal, stdout: &mut stdout, stderr: &mut stderr },
        );
        Ran {
            code,
            stdout: String::from_utf8(stdout).unwrap(),
            stderr: String::from_utf8(stderr).unwrap(),
        }
    }

    #[test]
    fn run_prints_the_backend_result_as_plain_text() {
        let dir = TempDir::new("run-text");
        let (url, seen) = fake_backend("The fixed text.");
        write_config(&dir.0, &url, true);
        let ran = run_cli(&["run", "Fix grammar"], &dir.0, "teh text\r\n", false);
        assert_eq!(ran.code, EXIT_OK, "stderr: {}", ran.stderr);
        assert_eq!(ran.stdout, "The fixed text.\n");
        assert_eq!(ran.stderr, "");
        let request = seen.lock().unwrap().join("");
        assert!(request.ends_with("\r\n\r\nteh text"), "{request}");
        assert!(request.contains("Bearer secret-key-123"));
    }

    #[test]
    fn run_json_reports_the_command_backend_and_text() {
        let dir = TempDir::new("run-json");
        let (url, _) = fake_backend("Done.");
        write_config(&dir.0, &url, true);
        let ran = run_cli(&["run", "fix", "--text", "x", "--json"], &dir.0, "", true);
        assert_eq!(ran.code, EXIT_OK, "stderr: {}", ran.stderr);
        assert!(ran.stdout.ends_with('\n') && ran.stdout.lines().count() == 1);
        let body: Value = serde_json::from_str(&ran.stdout).unwrap();
        assert_eq!(body["ok"], true);
        assert_eq!(body["text"], "Done.");
        assert_eq!(body["html"], Value::Null);
        assert_eq!(body["sent"], false);
        assert_eq!(body["command"], json!({"id": "fix", "name": "Fix grammar"}));
        assert_eq!(body["backend"], json!({"id": "hook", "name": "Local hook"}));
        assert!(body["backendMs"].is_u64());
    }

    #[test]
    fn a_send_only_backend_prints_nothing_and_says_so_on_stderr() {
        let dir = TempDir::new("run-sent");
        let (url, _) = fake_backend("ignored");
        write_config(&dir.0, &url, false);
        let ran = run_cli(&["run", "fix"], &dir.0, "x", false);
        assert_eq!(ran.code, EXIT_OK);
        assert_eq!(ran.stdout, "");
        assert!(ran.stderr.contains("returns no text"));
    }

    #[test]
    fn a_failing_backend_exits_1_with_a_plain_error_and_json_on_request() {
        let dir = TempDir::new("run-fail");
        write_config(&dir.0, "http://127.0.0.1:9/hook", true);
        let ran = run_cli(&["run", "fix", "--backend", "nowhere"], &dir.0, "x", false);
        assert_eq!(ran.code, EXIT_FAILED);
        assert_eq!(ran.stdout, "");
        assert!(ran.stderr.starts_with("clipboard-ai: Nowhere failed:"), "{}", ran.stderr);

        let ran = run_cli(&["run", "fix", "--backend", "nowhere", "--json"], &dir.0, "x", false);
        assert_eq!(ran.code, EXIT_FAILED);
        let body: Value = serde_json::from_str(&ran.stdout).unwrap();
        assert_eq!(body["ok"], false);
        assert_eq!(body["error"]["code"], "backend-error");
        assert!(!ran.stderr.is_empty());
    }

    #[test]
    fn unknown_commands_and_missing_config_exit_3() {
        let dir = TempDir::new("run-config");
        let ran = run_cli(&["list"], &dir.0, "", true);
        assert_eq!(ran.code, EXIT_CONFIG);
        assert!(ran.stderr.contains("there is no config file at"), "{}", ran.stderr);

        write_config(&dir.0, "http://127.0.0.1:9/hook", true);
        let ran = run_cli(&["run", "nope", "--json"], &dir.0, "x", false);
        assert_eq!(ran.code, EXIT_CONFIG);
        let body: Value = serde_json::from_str(&ran.stdout).unwrap();
        assert_eq!(body["error"]["code"], "unknown-command");

        fs::write(dir.0.join("config.json"), "{broken").unwrap();
        let ran = run_cli(&["list"], &dir.0, "", true);
        assert_eq!(ran.code, EXIT_CONFIG);
        assert!(ran.stderr.contains("cannot be used"), "{}", ran.stderr);
    }

    #[test]
    fn usage_problems_exit_2_and_point_at_help() {
        let dir = TempDir::new("usage");
        write_config(&dir.0, "http://127.0.0.1:9/hook", true);
        let ran = run_cli(&["run", "fix", "--frob"], &dir.0, "", true);
        assert_eq!(ran.code, EXIT_USAGE);
        assert!(ran.stderr.contains("unknown option --frob"));
        assert!(ran.stderr.contains("clipboard-ai help run"));

        let ran = run_cli(&["run", "fix"], &dir.0, "", true);
        assert_eq!(ran.code, EXIT_USAGE);
        assert!(ran.stderr.contains("no input text"));
    }

    #[test]
    fn the_cli_never_writes_into_the_config_folder() {
        let dir = TempDir::new("read-only");
        let v1 = r#"{"version":1,"backends":[{"id":"b","name":"B","kind":"http","baseUrl":"http://127.0.0.1:9/x","timeoutMs":10000}],
                     "prompts":[{"id":"p","name":"P","backendId":"b","hotkey":"Ctrl+Alt+P","systemPrompt":"x"}]}"#;
        fs::write(dir.0.join("config.json"), v1).unwrap();
        fs::write(dir.0.join(history::FILE_NAME), "{not json").unwrap();
        run_cli(&["list"], &dir.0, "", true);
        run_cli(&["history"], &dir.0, "", true);
        run_cli(&["run", "p"], &dir.0, "x", false);
        assert_eq!(fs::read_to_string(dir.0.join("config.json")).unwrap(), v1);
        let mut names: Vec<String> =
            fs::read_dir(&dir.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, vec!["config.json".to_string(), history::FILE_NAME.to_string()]);
    }

    #[test]
    fn list_shows_tables_without_secrets() {
        let dir = TempDir::new("list");
        write_config(&dir.0, "http://127.0.0.1:9/hook", true);
        let ran = run_cli(&["list"], &dir.0, "", true);
        assert_eq!(ran.code, EXIT_OK);
        assert_eq!(
            ran.stdout,
            "Commands\n\
             \x20 NAME         ID   HOTKEY      BACKEND\n\
             \x20 Fix grammar  fix  Ctrl+Alt+J  Local hook\n\
             \n\
             Backends\n\
             \x20 NAME        ID       KIND               MODEL\n\
             \x20 Local hook  hook     http               -\n\
             \x20 Nowhere     nowhere  openai-compatible  m\n"
        );
        assert!(!ran.stdout.contains("secret-key-123"));

        let ran = run_cli(&["list", "--json"], &dir.0, "", true);
        let body: Value = serde_json::from_str(&ran.stdout).unwrap();
        assert_eq!(body["ok"], true);
        assert_eq!(body["commands"][0]["id"], "fix");
        assert_eq!(body["backends"][1]["kind"], "openai-compatible");
        assert!(!ran.stdout.contains("secret-key-123") && !ran.stdout.contains("127.0.0.1"));
    }

    fn entry(name: &str, outcome: Outcome, input: Option<&str>, result: Option<&str>) -> HistoryEntry {
        HistoryEntry {
            timestamp_ms: 1_795_132_800_000,
            command_id: name.to_lowercase(),
            command_name: name.into(),
            backend_name: "Local".into(),
            outcome,
            backend_ms: 800,
            total_cycle_ms: Some(1_250),
            input_text: input.map(Into::into),
            result_text: result.map(Into::into),
        }
    }

    #[test]
    fn history_shows_newest_first_up_to_the_limit() {
        let dir = TempDir::new("history");
        write_config(&dir.0, "http://127.0.0.1:9/hook", true);
        let ran = run_cli(&["history"], &dir.0, "", true);
        assert_eq!(ran.stdout, "No runs recorded yet.\n");

        let entries = vec![
            entry("Fix", Outcome::Succeeded, Some("teh\ncat"), Some("The cat")),
            entry("Shorten", Outcome::aborted("focus-changed"), Some("x"), None),
            entry("Old", Outcome::failed("backend-error: boom"), None, None),
        ];
        fs::write(dir.0.join(history::FILE_NAME), serde_json::to_string(&entries).unwrap()).unwrap();

        let ran = run_cli(&["history", "--limit", "2"], &dir.0, "", true);
        assert_eq!(ran.code, EXIT_OK);
        let stamp = format_timestamp(1_795_132_800_000);
        assert_eq!(
            ran.stdout,
            format!(
                "{stamp}  Fix - Local - ok - 1.2 s\n  in   teh cat\n  out  The cat\n\n\
                 {stamp}  Shorten - Local - aborted: focus-changed - 1.2 s\n  in   x\n"
            )
        );

        let ran = run_cli(&["history", "--json", "--limit", "1"], &dir.0, "", true);
        let body: Value = serde_json::from_str(&ran.stdout).unwrap();
        assert_eq!(body["entries"].as_array().unwrap().len(), 1);
        assert_eq!(body["entries"][0]["commandName"], "Fix");
        assert_eq!(body["entries"][0]["outcome"]["status"], "succeeded");
    }

    #[test]
    fn help_and_version_print_to_stdout_and_exit_0() {
        let dir = TempDir::new("help");
        let ran = run_cli(&["--help"], &dir.0, "", true);
        assert_eq!(ran.code, EXIT_OK);
        assert!(ran.stdout.starts_with(&format!("clipboard-ai {VERSION}\n")));
        assert!(ran.stdout.contains("Exit codes:"));
        let ran = run_cli(&["help", "run"], &dir.0, "", true);
        assert!(ran.stdout.starts_with("Usage: clipboard-ai run"));
        let ran = run_cli(&["--version"], &dir.0, "", true);
        assert_eq!(ran.stdout, format!("clipboard-ai {VERSION}\n"));
        for text in [MAIN_HELP, RUN_HELP, LIST_HELP, HISTORY_HELP] {
            assert!(!text.contains('\u{2014}') && !text.contains('\u{2013}'), "plain hyphens only");
            assert!(text.lines().all(|l| l.chars().count() <= 80), "help lines fit 80 columns");
        }
    }

    #[test]
    fn default_config_dir_prefers_the_override_then_appdata() {
        let env = Env { config_dir: None, appdata: Some(PathBuf::from("C:\\Users\\u\\AppData\\Roaming")) };
        assert_eq!(
            default_config_dir(&env),
            Some(PathBuf::from("C:\\Users\\u\\AppData\\Roaming\\com.clipboardai.app"))
        );
        let env = Env { config_dir: Some(PathBuf::from("D:\\cfg")), appdata: env.appdata };
        assert_eq!(default_config_dir(&env), Some(PathBuf::from("D:\\cfg")));
        assert_eq!(default_config_dir(&Env { config_dir: None, appdata: None }), None);
    }
}
