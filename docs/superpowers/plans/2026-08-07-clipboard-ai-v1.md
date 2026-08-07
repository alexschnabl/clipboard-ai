# Clipboard AI v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A tray-resident Windows app where each configured prompt has its own hotkey and backend, transforming the current selection in place.

**Architecture:** One Rust process. Tauri owns the tray and global shortcuts. A `Backend` trait has three HTTP/subprocess implementations. A `cycle` engine takes every OS operation through an `OsOps` trait so the safety rules are unit-testable without a GUI — the structure that made the Python spike's rules provable.

**Tech Stack:** Rust 1.93, Tauri v2, `tauri-plugin-global-shortcut`, `arboard` (clipboard), `enigo` (synthetic input), `windows` (GetAsyncKeyState / GetForegroundWindow), `reqwest` blocking, `serde`/`serde_json`, `notify` (config watch).

## Global Constraints

Copied from `docs/superpowers/specs/2026-08-07-clipboard-ai-v1-design.md`. Every task's requirements implicitly include this section.

- **Windows only.** Platform calls stay behind traits so a port is additive, but no non-Windows code is written or tested.
- **No UI beyond the tray menu.** No settings window, no review popup, no formatting preservation, no sidecar. All are later cycles.
- Clipboard capture timeout **1000 ms**, polled every **20 ms**. Modifier-release timeout **2000 ms**, polled every **10 ms**. Clipboard restore delay after paste **300 ms**.
- Sentinel instruction appended to every system prompt, verbatim:
  `Return only the result, wrapped in <output></output> tags. Emit nothing after the closing tag.`
- **Never paste on abort.** Capture timeout, backend error, empty result, missing sentinel, stuck modifiers, changed focus — all restore the clipboard and paste nothing.
- **Always restore the original clipboard**, on every path including an unexpected error.
- **Never paste unextracted output.** No sentinel means refuse.
- Abort reasons, exact strings: `capture-timeout`, `modifiers-stuck`, `modifiers-stuck-before-paste`, `focus-changed`, `backend-error`, `no-sentinel`, `empty-result`.
- Every backend failure becomes a `BackendError`. No other error type may escape a backend.
- Config `kind` values, exact strings: `openai-compatible`, `anthropic-api`, `claude-cli`.

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/main.rs` | Entry point. Thin — calls `lib::run()`. |
| `src-tauri/src/lib.rs` | Tauri builder, tray, hotkey registration, cycle dispatch. The untestable shell. |
| `src-tauri/src/config.rs` | Config types, parse, validate, default file, path resolution |
| `src-tauri/src/sentinel.rs` | `<output>` extraction |
| `src-tauri/src/backend/mod.rs` | `Backend` trait, `BackendError`, `HttpClient` trait, construction from config |
| `src-tauri/src/backend/openai.rs` | OpenAI-compatible adapter |
| `src-tauri/src/backend/anthropic.rs` | Anthropic Messages API adapter |
| `src-tauri/src/backend/claude_cli.rs` | `claude -p` subprocess adapter |
| `src-tauri/src/osops.rs` | `OsOps` trait + Windows implementation |
| `src-tauri/src/cycle.rs` | The cycle engine. Safety-critical. |

`cycle.rs` is deliberately separate from `lib.rs`: it takes every dependency as a parameter and touches no globals, which is what makes the abort rules testable. `lib.rs` is the shell — tray, registration, threading.

---

### Task 1: Scaffold and prove the risky part

**Files:**
- Create: the Tauri v2 project under `src-tauri/`, plus `package.json`, `.gitignore` additions
- Create: `src-tauri/src/bin/smoke.rs`

**Interfaces:**
- Consumes: nothing
- Produces: a building Tauri v2 app; confirmed working `enigo` + `arboard` + `tauri-plugin-global-shortcut` on this machine

This task exists to fail fast. The Python spike proved the *approach* with `pynput` and `pyperclip`; nothing yet proves the Rust stack behaves the same. Everything else in this plan is built on top of it, so it goes first and it is verified by hand before anything else is written.

- [ ] **Step 1: Scaffold the Tauri app**

The frontend is irrelevant in v1 (no window), so scaffold with the vanilla template to keep it minimal.

```bash
npm create tauri-app@latest -- --yes --manager npm --template vanilla --identifier com.clipboardai.app clipboard-ai-app
```

If that produces a nested directory, move `src-tauri/` and the frontend files to the repo root so `src-tauri/` sits beside `docs/` and `spike/`. The repo root must remain the git root.

Add to `.gitignore`:

```
node_modules/
dist/
src-tauri/target/
```

- [ ] **Step 2: Add the dependencies**

```bash
cd src-tauri
cargo add tauri-plugin-global-shortcut
cargo add arboard enigo notify reqwest --features reqwest/blocking,reqwest/json
cargo add serde --features derive
cargo add serde_json thiserror
cargo add windows --features Win32_UI_Input_KeyboardAndMouse,Win32_UI_WindowsAndMessaging,Win32_Foundation
```

- [ ] **Step 3: Verify it builds**

Run: `cd src-tauri && cargo build`
Expected: compiles. If a crate fails, record the exact version resolved and the error in your report before working around it — a dependency that does not build on Rust 1.93 is information the rest of the plan needs.

- [ ] **Step 4: Write the smoke binary**

This is throwaway diagnostic code, kept as a `bin` target so it can be re-run later. It answers three questions: does a global shortcut fire, does `arboard` round-trip the clipboard, and does `enigo` deliver `Ctrl+C`/`Ctrl+V` to another application.

Create `src-tauri/src/bin/smoke.rs`:

```rust
use std::{thread, time::Duration};

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

fn main() {
    println!("Focus a window with selected text. Copying in 3 seconds...");
    thread::sleep(Duration::from_secs(3));

    let mut clipboard = Clipboard::new().expect("clipboard open");
    let original = clipboard.get_text().unwrap_or_default();
    println!("original clipboard: {original:?}");
    clipboard.set_text("").expect("clear");

    let mut enigo = Enigo::new(&Settings::default()).expect("enigo");
    enigo.key(Key::Control, Direction::Press).unwrap();
    enigo.key(Key::Unicode('c'), Direction::Click).unwrap();
    enigo.key(Key::Control, Direction::Release).unwrap();

    let mut captured = String::new();
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(20));
        let value = clipboard.get_text().unwrap_or_default();
        if !value.trim().is_empty() {
            captured = value;
            break;
        }
    }
    println!("captured: {captured:?}");

    if captured.trim().is_empty() {
        println!("FAIL: nothing captured — Ctrl+C did not reach the target window");
        return;
    }

    clipboard.set_text(format!("[{captured}]")).expect("set");
    enigo.key(Key::Control, Direction::Press).unwrap();
    enigo.key(Key::Unicode('v'), Direction::Click).unwrap();
    enigo.key(Key::Control, Direction::Release).unwrap();
    thread::sleep(Duration::from_millis(300));
    clipboard.set_text(original).expect("restore");
    println!("OK: pasted the captured text wrapped in brackets, clipboard restored");
}
```

- [ ] **Step 5: Run the smoke binary and verify by hand**

Open Notepad, type `hello world`, select it. Then:

```bash
cd src-tauri && cargo run --bin smoke
```

Click into Notepad within the 3-second window.

Expected: the selection is replaced by `[hello world]`, the console prints `OK`, and the clipboard holds whatever it held before.

Record what actually happened, including the capture latency implied by how many polls it took. If `enigo` fails to reach the target window, or `arboard` errors, **stop and report BLOCKED** — that finding changes the plan and is worth more than a workaround.

- [ ] **Step 6: Register a global shortcut and verify it fires**

Replace `src-tauri/src/lib.rs`'s `run()` body with a minimal tray-plus-shortcut app:

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri_plugin_global_shortcut::{
                Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
            };

            let hotkey = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyJ);
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |_app, shortcut, event| {
                        if shortcut == &hotkey && event.state() == ShortcutState::Released {
                            println!("hotkey released");
                        }
                    })
                    .build(),
            )?;
            app.global_shortcut().register(hotkey)?;

            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Delete the default window from `src-tauri/tauri.conf.json` by setting `"app": { "windows": [] }` so the app runs tray-only. Keep the rest of the config as generated.

- [ ] **Step 7: Verify the shortcut and tray by hand**

Run: `npm run tauri dev`

Expected: no window appears, a tray icon appears, pressing `Ctrl+Alt+J` prints `hotkey released` in the terminal, and the tray Quit item exits the app.

**Record specifically whether the handler fires with modifiers already released.** The plugin reports both `Pressed` and `Released`; if `Released` reliably arrives after the physical keys are up, it reduces the modifier-bleed problem that dominated the Python spike. Note what you observe — the cycle engine still keeps its explicit modifier wait either way, but this is useful data.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: scaffold Tauri v2 tray app and verify Rust input stack"
```

---

### Task 2: Config

**Files:**
- Create: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod config;`)

**Interfaces:**
- Consumes: nothing
- Produces:
  - `config::Config { version: u32, backends: Vec<BackendConfig>, prompts: Vec<PromptConfig> }`
  - `config::BackendConfig { id: String, name: String, kind: String, base_url: Option<String>, model: Option<String>, api_key: Option<String>, timeout_ms: u64 }`
  - `config::PromptConfig { id: String, name: String, backend_id: String, hotkey: String, system_prompt: String }`
  - `config::ConfigError` (thiserror enum)
  - `config::parse(json: &str) -> Result<Config, ConfigError>` — parses and validates
  - `config::default_config() -> Config`
  - `config::config_path(app: &tauri::AppHandle) -> PathBuf`
  - `config::load_or_create(path: &Path) -> Result<Config, ConfigError>`

Serde field names are camelCase on the wire (`baseUrl`, `timeoutMs`, `backendId`, `systemPrompt`) via `#[serde(rename_all = "camelCase")]`, matching the spec's example file.

- [ ] **Step 1: Write the failing tests**

Create the test module at the bottom of `src-tauri/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn valid_json() -> &'static str {
        r#"{
          "version": 1,
          "backends": [
            {"id":"ollama","name":"Local","kind":"openai-compatible",
             "baseUrl":"http://localhost:11434/v1","model":"qwen2.5:32b-instruct",
             "apiKey":null,"timeoutMs":120000}
          ],
          "prompts": [
            {"id":"fix","name":"Fix grammar","backendId":"ollama",
             "hotkey":"Ctrl+Alt+J","systemPrompt":"Fix grammar."}
          ]
        }"#
    }

    #[test]
    fn parses_a_valid_config() {
        let cfg = parse(valid_json()).expect("should parse");
        assert_eq!(cfg.backends.len(), 1);
        assert_eq!(cfg.prompts[0].backend_id, "ollama");
        assert_eq!(cfg.backends[0].timeout_ms, 120_000);
    }

    #[test]
    fn rejects_a_prompt_naming_an_unknown_backend() {
        let json = valid_json().replace(r#""backendId":"ollama""#, r#""backendId":"nope""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownBackend { .. })));
    }

    #[test]
    fn rejects_duplicate_backend_ids() {
        let json = valid_json().replace(
            r#""prompts""#,
            r#""_unused":0,"prompts""#,
        );
        // duplicate the single backend entry
        let json = json.replace(
            r#""apiKey":null,"timeoutMs":120000}"#,
            r#""apiKey":null,"timeoutMs":120000},
               {"id":"ollama","name":"Dup","kind":"openai-compatible",
                "baseUrl":"http://x/v1","model":"m","apiKey":null,"timeoutMs":1000}"#,
        );
        assert!(matches!(parse(&json), Err(ConfigError::DuplicateId { .. })));
    }

    #[test]
    fn rejects_duplicate_hotkeys() {
        let json = valid_json().replace(
            r#""systemPrompt":"Fix grammar."}"#,
            r#""systemPrompt":"Fix grammar."},
               {"id":"other","name":"Other","backendId":"ollama",
                "hotkey":"Ctrl+Alt+J","systemPrompt":"Other."}"#,
        );
        assert!(matches!(parse(&json), Err(ConfigError::DuplicateHotkey { .. })));
    }

    #[test]
    fn rejects_an_unknown_backend_kind() {
        let json = valid_json().replace(r#""kind":"openai-compatible""#, r#""kind":"telepathy""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownKind { .. })));
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(parse("{nonsense"), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn the_default_config_is_valid() {
        let json = serde_json::to_string(&default_config()).unwrap();
        parse(&json).expect("default config must round-trip through validation");
    }
}
```

The last test matters more than it looks: the default file is what a first-time user gets, and shipping one that fails its own validation would make the app unusable on first run.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test config`
Expected: compile error — `config` module does not exist yet.

- [ ] **Step 3: Write the implementation**

Create `src-tauri/src/config.rs` above the test module:

```rust
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const KIND_OPENAI: &str = "openai-compatible";
pub const KIND_ANTHROPIC: &str = "anthropic-api";
pub const KIND_CLAUDE_CLI: &str = "claude-cli";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config is not valid JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("prompt {prompt} names unknown backend {backend}")]
    UnknownBackend { prompt: String, backend: String },
    #[error("duplicate id: {id}")]
    DuplicateId { id: String },
    #[error("duplicate hotkey: {hotkey}")]
    DuplicateHotkey { hotkey: String },
    #[error("unknown backend kind: {kind}")]
    UnknownKind { kind: String },
    #[error("could not read or write config: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendConfig {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    120_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptConfig {
    pub id: String,
    pub name: String,
    pub backend_id: String,
    pub hotkey: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub backends: Vec<BackendConfig>,
    pub prompts: Vec<PromptConfig>,
}

impl Config {
    pub fn backend(&self, id: &str) -> Option<&BackendConfig> {
        self.backends.iter().find(|b| b.id == id)
    }
}

pub fn parse(json: &str) -> Result<Config, ConfigError> {
    let config: Config = serde_json::from_str(json)?;
    validate(&config)?;
    Ok(config)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    let mut ids = HashSet::new();
    for backend in &config.backends {
        if !ids.insert(&backend.id) {
            return Err(ConfigError::DuplicateId { id: backend.id.clone() });
        }
        match backend.kind.as_str() {
            KIND_OPENAI | KIND_ANTHROPIC | KIND_CLAUDE_CLI => {}
            other => return Err(ConfigError::UnknownKind { kind: other.to_string() }),
        }
    }

    let mut prompt_ids = HashSet::new();
    let mut hotkeys = HashSet::new();
    for prompt in &config.prompts {
        if !prompt_ids.insert(&prompt.id) {
            return Err(ConfigError::DuplicateId { id: prompt.id.clone() });
        }
        if !hotkeys.insert(prompt.hotkey.to_lowercase()) {
            return Err(ConfigError::DuplicateHotkey { hotkey: prompt.hotkey.clone() });
        }
        if config.backend(&prompt.backend_id).is_none() {
            return Err(ConfigError::UnknownBackend {
                prompt: prompt.id.clone(),
                backend: prompt.backend_id.clone(),
            });
        }
    }
    Ok(())
}

pub fn default_config() -> Config {
    Config {
        version: 1,
        backends: vec![
            BackendConfig {
                id: "ollama-local".into(),
                name: "Local Ollama".into(),
                kind: KIND_OPENAI.into(),
                base_url: Some("http://localhost:11434/v1".into()),
                model: Some("qwen2.5:32b-instruct".into()),
                api_key: None,
                timeout_ms: 120_000,
            },
            BackendConfig {
                id: "claude-sub".into(),
                name: "Claude (subscription)".into(),
                kind: KIND_CLAUDE_CLI.into(),
                base_url: None,
                model: None,
                api_key: None,
                timeout_ms: 120_000,
            },
        ],
        prompts: vec![PromptConfig {
            id: "fix-grammar".into(),
            name: "Fix grammar".into(),
            backend_id: "ollama-local".into(),
            hotkey: "Ctrl+Alt+J".into(),
            system_prompt: "Fix grammar and spelling.".into(),
        }],
    }
}

pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&default_config())?;
        fs::write(path, json)?;
    }
    parse(&fs::read_to_string(path)?)
}

pub fn config_path(app: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    app.path()
        .app_config_dir()
        .expect("app config dir")
        .join("config.json")
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test config`
Expected: PASS, 7 tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/lib.rs
git commit -m "feat: add config types, parsing and validation"
```

---

### Task 3: Sentinel extraction

**Files:**
- Create: `src-tauri/src/sentinel.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod sentinel;`)

**Interfaces:**
- Consumes: nothing
- Produces:
  - `sentinel::SENTINEL_INSTRUCTION: &str`
  - `sentinel::wrap_system_prompt(system_prompt: &str) -> String`
  - `sentinel::extract(raw: &str) -> Option<String>` — `None` means refuse

This is what makes `claude -p` usable despite it prepending `The corrected text:`, and it is the last line of defence before text reaches the user's document. `None` must be treated as a refusal by the caller — never as empty text.

- [ ] **Step 1: Write the failing tests**

Create the test module at the bottom of `src-tauri/src/sentinel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_the_tagged_content() {
        assert_eq!(extract("<output>Corrected.</output>").as_deref(), Some("Corrected."));
    }

    #[test]
    fn discards_preamble_before_the_tag() {
        let raw = "The corrected text:\n\n<output>Corrected.</output>";
        assert_eq!(extract(raw).as_deref(), Some("Corrected."));
    }

    #[test]
    fn discards_commentary_after_the_tag() {
        let raw = "<output>Corrected.</output>\n\nLet me know if you need anything else!";
        assert_eq!(extract(raw).as_deref(), Some("Corrected."));
    }

    #[test]
    fn discards_reasoning_markup_outside_the_tags() {
        let raw = "<think>the user wants grammar fixed</think><output>Corrected.</output>";
        assert_eq!(extract(raw).as_deref(), Some("Corrected."));
    }

    #[test]
    fn preserves_internal_newlines_and_trims_only_the_edges() {
        let raw = "<output>\nline one\nline two\n</output>";
        assert_eq!(extract(raw).as_deref(), Some("line one\nline two"));
    }

    #[test]
    fn refuses_when_there_is_no_tag() {
        assert!(extract("Corrected.").is_none());
    }

    #[test]
    fn refuses_when_the_closing_tag_is_missing() {
        assert!(extract("<output>Corrected.").is_none());
    }

    #[test]
    fn refuses_when_the_opening_tag_is_missing() {
        assert!(extract("Corrected.</output>").is_none());
    }

    #[test]
    fn refuses_when_the_content_is_only_whitespace() {
        assert!(extract("<output>   \n  </output>").is_none());
    }

    #[test]
    fn takes_the_first_block_when_several_are_present() {
        let raw = "<output>first</output> junk <output>second</output>";
        assert_eq!(extract(raw).as_deref(), Some("first"));
    }

    #[test]
    fn wrap_appends_the_instruction_to_the_prompt() {
        let wrapped = wrap_system_prompt("Fix grammar.");
        assert!(wrapped.starts_with("Fix grammar."));
        assert!(wrapped.contains(SENTINEL_INSTRUCTION));
    }
}
```

The "closing tag missing" case is the truncation shape — a model cut off by a token limit. It must refuse rather than paste a fragment, which is the same failure the Python spike hit twice.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test sentinel`
Expected: compile error — `sentinel` module does not exist yet.

- [ ] **Step 3: Write the implementation**

Create `src-tauri/src/sentinel.rs` above the test module:

```rust
pub const SENTINEL_INSTRUCTION: &str =
    "Return only the result, wrapped in <output></output> tags. \
     Emit nothing after the closing tag.";

const OPEN: &str = "<output>";
const CLOSE: &str = "</output>";

pub fn wrap_system_prompt(system_prompt: &str) -> String {
    format!("{system_prompt}\n\n{SENTINEL_INSTRUCTION}")
}

/// Extract the first `<output>…</output>` block.
///
/// Returns `None` when the tags are absent, unbalanced, or wrap only
/// whitespace. Callers must treat `None` as a refusal and paste nothing —
/// never as empty text.
pub fn extract(raw: &str) -> Option<String> {
    let start = raw.find(OPEN)? + OPEN.len();
    let rest = &raw[start..];
    let end = rest.find(CLOSE)?;
    let content = rest[..end].trim();
    if content.is_empty() {
        return None;
    }
    Some(content.to_string())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test sentinel`
Expected: PASS, 11 tests

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sentinel.rs src-tauri/src/lib.rs
git commit -m "feat: add sentinel output extraction"
```

---

### Task 4: Backend trait and the OpenAI-compatible adapter

**Files:**
- Create: `src-tauri/src/backend/mod.rs`
- Create: `src-tauri/src/backend/openai.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod backend;`)

**Interfaces:**
- Consumes: `config::BackendConfig`
- Produces:
  - `backend::BackendError` (thiserror enum)
  - `backend::HttpClient` trait — `fn post_json(&self, url: &str, headers: &[(String, String)], body: &serde_json::Value, timeout_ms: u64) -> Result<serde_json::Value, BackendError>`
  - `backend::ReqwestClient` — the real `HttpClient`
  - `backend::Backend` trait — `fn name(&self) -> &str` and `fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError>`
  - `backend::openai::OpenAiCompatible::new(config: &BackendConfig, http: Arc<dyn HttpClient>) -> OpenAiCompatible`

Every failure becomes a `BackendError`. The cycle engine keys its abort handling on that single type, so any other error escaping a backend defeats it.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/backend/openai.rs` with this test module at the bottom:

```rust
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

    #[test]
    fn a_response_with_no_choices_is_a_backend_error() {
        let backend = OpenAiCompatible::new(&cfg(), FakeHttp::ok(json!({"choices": []})));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))));
    }

    #[test]
    fn a_null_message_is_a_backend_error_not_a_panic() {
        let body = json!({"choices":[{"message": null}]});
        let backend = OpenAiCompatible::new(&cfg(), FakeHttp::ok(body));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))));
    }

    #[test]
    fn a_null_content_is_a_backend_error() {
        let body = json!({"choices":[{"message":{"content": null}}]});
        let backend = OpenAiCompatible::new(&cfg(), FakeHttp::ok(body));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))));
    }

    #[test]
    fn missing_base_url_is_a_backend_error() {
        let mut config = cfg();
        config.base_url = None;
        let backend = OpenAiCompatible::new(&config, FakeHttp::ok(chat("ok")));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Misconfigured(_))));
    }
}
```

The two null tests exist because the Python spike shipped exactly this defect twice — a present-but-null field raising an unexpected error type that slipped past the abort handler.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test backend`
Expected: compile error — `backend` module does not exist yet.

- [ ] **Step 3: Write the trait module**

Create `src-tauri/src/backend/mod.rs`:

```rust
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
```

- [ ] **Step 4: Write the OpenAI-compatible adapter**

Add to the top of `src-tauri/src/backend/openai.rs`, above the test module:

```rust
use std::sync::Arc;

use serde_json::json;

use crate::backend::{Backend, BackendError, HttpClient};
use crate::config::BackendConfig;

pub struct OpenAiCompatible {
    name: String,
    base_url: Option<String>,
    model: String,
    api_key: Option<String>,
    timeout_ms: u64,
    http: Arc<dyn HttpClient>,
}

impl OpenAiCompatible {
    pub fn new(config: &BackendConfig, http: Arc<dyn HttpClient>) -> Self {
        Self {
            name: config.name.clone(),
            base_url: config.base_url.clone(),
            model: config.model.clone().unwrap_or_default(),
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
        let base = self
            .base_url
            .as_deref()
            .ok_or_else(|| BackendError::Misconfigured("baseUrl is required".into()))?;
        let url = format!("{}/chat/completions", base.trim_end_matches('/'));

        let mut headers = Vec::new();
        if let Some(key) = &self.api_key {
            headers.push(("Authorization".to_string(), format!("Bearer {key}")));
        }

        let body = json!({
            "model": self.model,
            "stream": false,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": text},
            ],
        });

        let response = self.http.post_json(&url, &headers, &body, self.timeout_ms)?;

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
```

Note the chained `and_then` with a final `as_str()`: a null or missing field at any level yields `None` and becomes a `BadResponse`, never a panic or an unexpected error type.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test backend`
Expected: PASS, 8 tests

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/backend/ src-tauri/src/lib.rs
git commit -m "feat: add Backend trait and OpenAI-compatible adapter"
```

---

### Task 5: Anthropic API and Claude CLI adapters

**Files:**
- Create: `src-tauri/src/backend/anthropic.rs`
- Create: `src-tauri/src/backend/claude_cli.rs`
- Modify: `src-tauri/src/backend/mod.rs` (add the modules and a constructor)

**Interfaces:**
- Consumes: `backend::{Backend, BackendError, HttpClient}`, `config::BackendConfig`
- Produces:
  - `backend::anthropic::AnthropicApi::new(config: &BackendConfig, http: Arc<dyn HttpClient>) -> AnthropicApi`
  - `backend::claude_cli::CommandRunner` trait — `fn run(&self, program: &str, args: &[String], stdin: &str, timeout_ms: u64) -> Result<String, BackendError>`
  - `backend::claude_cli::ClaudeCli::new(config: &BackendConfig, runner: Arc<dyn CommandRunner>) -> ClaudeCli`
  - `backend::build(config: &BackendConfig) -> Result<Arc<dyn Backend>, BackendError>` — dispatches on `kind`

**The Anthropic model default is `claude-opus-5`** when config omits `model`. Requests use `anthropic-version: 2023-06-01`, `max_tokens: 16000`, and non-streaming. Do not set `temperature`, `top_p`, `top_k` or `budget_tokens` — they are rejected on current models.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/backend/anthropic.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use super::*;
    use crate::backend::{BackendError, HttpClient};
    use crate::config::{BackendConfig, KIND_ANTHROPIC};

    struct FakeHttp {
        response: serde_json::Value,
        seen: Mutex<Option<(String, Vec<(String, String)>, serde_json::Value)>>,
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
            _timeout_ms: u64,
        ) -> Result<serde_json::Value, BackendError> {
            *self.seen.lock().unwrap() = Some((url.into(), headers.to_vec(), body.clone()));
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
        let body = json!({"content": [], "stop_reason": "refusal"});
        let backend = AnthropicApi::new(&cfg(), FakeHttp::new(body));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))));
    }

    #[test]
    fn sends_the_documented_request_shape() {
        let http = FakeHttp::new(message(json!([{"type":"text","text":"ok"}])));
        let backend = AnthropicApi::new(&cfg(), http.clone());
        backend.transform("Fix it.", "input").unwrap();
        let (url, headers, body) = http.seen.lock().unwrap().clone().unwrap();
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

    #[test]
    fn a_missing_api_key_is_a_backend_error() {
        let mut config = cfg();
        config.api_key = None;
        let backend = AnthropicApi::new(&config, FakeHttp::new(json!({})));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Misconfigured(_))));
    }
}
```

Create `src-tauri/src/backend/claude_cli.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::backend::BackendError;
    use crate::config::{BackendConfig, KIND_CLAUDE_CLI};

    struct FakeRunner {
        result: Result<String, BackendError>,
        seen: Mutex<Option<(String, Vec<String>, String)>>,
    }

    impl FakeRunner {
        fn ok(out: &str) -> Arc<Self> {
            Arc::new(Self { result: Ok(out.into()), seen: Mutex::new(None) })
        }
        fn failing(err: BackendError) -> Arc<Self> {
            Arc::new(Self { result: Err(err), seen: Mutex::new(None) })
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(
            &self,
            program: &str,
            args: &[String],
            stdin: &str,
            _timeout_ms: u64,
        ) -> Result<String, BackendError> {
            *self.seen.lock().unwrap() = Some((program.into(), args.to_vec(), stdin.into()));
            self.result.clone()
        }
    }

    fn cfg() -> BackendConfig {
        BackendConfig {
            id: "claude-sub".into(),
            name: "Claude (subscription)".into(),
            kind: KIND_CLAUDE_CLI.into(),
            base_url: None,
            model: None,
            api_key: None,
            timeout_ms: 120_000,
        }
    }

    #[test]
    fn returns_the_trimmed_stdout() {
        let backend = ClaudeCli::new(&cfg(), FakeRunner::ok("Corrected.\n"));
        assert_eq!(backend.transform("s", "t").unwrap(), "Corrected.");
    }

    #[test]
    fn invokes_claude_p_with_the_system_prompt_and_text_on_stdin() {
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&cfg(), runner.clone());
        backend.transform("Fix it.", "input text").unwrap();
        let (program, args, stdin) = runner.seen.lock().unwrap().clone().unwrap();
        assert_eq!(program, "claude");
        assert!(args.contains(&"-p".to_string()));
        let idx = args.iter().position(|a| a == "--system-prompt").unwrap();
        assert_eq!(args[idx + 1], "Fix it.");
        assert_eq!(stdin, "input text");
    }

    #[test]
    fn empty_output_is_a_backend_error() {
        let backend = ClaudeCli::new(&cfg(), FakeRunner::ok("   \n "));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))));
    }

    #[test]
    fn a_runner_timeout_propagates_as_a_backend_error() {
        let backend = ClaudeCli::new(&cfg(), FakeRunner::failing(BackendError::Timeout));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Timeout)));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test backend`
Expected: compile error — `anthropic` and `claude_cli` modules do not exist yet.

- [ ] **Step 3: Write the Anthropic adapter**

Add above the test module in `src-tauri/src/backend/anthropic.rs`:

```rust
use std::sync::Arc;

use serde_json::json;

use crate::backend::{Backend, BackendError, HttpClient};
use crate::config::BackendConfig;

const DEFAULT_MODEL: &str = "claude-opus-5";
const API_URL: &str = "https://api.anthropic.com/v1/messages";

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
            model: config.model.clone().unwrap_or_else(|| DEFAULT_MODEL.to_string()),
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
            .ok_or_else(|| BackendError::Misconfigured("apiKey is required".into()))?;

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

        if response.get("stop_reason").and_then(|s| s.as_str()) == Some("refusal") {
            return Err(BackendError::BadResponse("model declined the request".into()));
        }

        let blocks = response
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| BackendError::BadResponse(format!("no content in {response}")))?;

        let joined: String = blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect();

        if joined.trim().is_empty() {
            return Err(BackendError::BadResponse("no text blocks in response".into()));
        }
        Ok(joined)
    }
}
```

- [ ] **Step 4: Write the Claude CLI adapter**

Add above the test module in `src-tauri/src/backend/claude_cli.rs`:

```rust
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::Arc,
};

use crate::backend::{Backend, BackendError};
use crate::config::BackendConfig;

pub trait CommandRunner: Send + Sync {
    fn run(
        &self,
        program: &str,
        args: &[String],
        stdin: &str,
        timeout_ms: u64,
    ) -> Result<String, BackendError>;
}

pub struct ProcessRunner;

impl CommandRunner for ProcessRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        stdin: &str,
        _timeout_ms: u64,
    ) -> Result<String, BackendError> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| BackendError::Transport(format!("could not start {program}: {e}")))?;

        child
            .stdin
            .as_mut()
            .ok_or_else(|| BackendError::Transport("no stdin handle".into()))?
            .write_all(stdin.as_bytes())
            .map_err(|e| BackendError::Transport(e.to_string()))?;
        drop(child.stdin.take());

        let output = child
            .wait_with_output()
            .map_err(|e| BackendError::Transport(e.to_string()))?;

        if !output.status.success() {
            return Err(BackendError::BadResponse(format!(
                "{program} exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

pub struct ClaudeCli {
    name: String,
    timeout_ms: u64,
    runner: Arc<dyn CommandRunner>,
}

impl ClaudeCli {
    pub fn new(config: &BackendConfig, runner: Arc<dyn CommandRunner>) -> Self {
        Self {
            name: config.name.clone(),
            timeout_ms: config.timeout_ms,
            runner,
        }
    }
}

impl Backend for ClaudeCli {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError> {
        let args = vec![
            "-p".to_string(),
            "--system-prompt".to_string(),
            system_prompt.to_string(),
        ];
        let output = self.runner.run("claude", &args, text, self.timeout_ms)?;
        let trimmed = output.trim();
        if trimmed.is_empty() {
            return Err(BackendError::BadResponse("claude returned no output".into()));
        }
        Ok(trimmed.to_string())
    }
}
```

`ProcessRunner` does not enforce `timeout_ms` — `std::process` has no built-in wait-with-timeout. This is a known, deliberate gap for v1: the `claude` CLI has its own internal limits, and the parameter is threaded through so a future implementation can honour it without a signature change. **Say so in your report** rather than leaving it implicit.

- [ ] **Step 5: Add the dispatcher**

Add to `src-tauri/src/backend/mod.rs`:

```rust
pub mod anthropic;
pub mod claude_cli;

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
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test backend`
Expected: PASS, 17 tests (8 from Task 4 plus 9 here)

- [ ] **Step 7: Verify both adapters against reality**

Create `src-tauri/examples/try_backend.rs`:

```rust
use std::time::Instant;

use clipboard_ai_lib::{backend, config};

fn main() {
    let cfg = config::default_config();
    for backend_cfg in &cfg.backends {
        print!("{:<24}", backend_cfg.name);
        let built = match backend::build(backend_cfg) {
            Ok(b) => b,
            Err(e) => {
                println!("build failed: {e}");
                continue;
            }
        };
        let start = Instant::now();
        match built.transform("Fix grammar and spelling.", "this sentance have two mistake") {
            Ok(text) => println!("{:>7} ms  {text:?}", start.elapsed().as_millis()),
            Err(e) => println!("{:>7} ms  ERROR {e}", start.elapsed().as_millis()),
        }
    }
}
```

The crate name in the `use` line must match the `name` under `[lib]` in `src-tauri/Cargo.toml` — the Tauri template sets it to `<app_name>_lib`. Check that file rather than assuming, and make `config` and `backend` public in `lib.rs` (`pub mod backend;`, `pub mod config;`) so the example can reach them.

Run: `cd src-tauri && cargo run --example try_backend`

Expected: both backends return a corrected sentence with a timing figure. Ollama should be fast if the model is resident; `claude-sub` will take a few seconds.

**Record whether `claude -p` still prepends preamble text.** The sentinel is not wired in at this layer, so raw output is what you will see. Confirming the preamble is still present is what validates that Task 3 exists for a real reason rather than a theoretical one.

Delete `src-tauri/examples/try_backend.rs` before committing — it is scaffolding, not a deliverable.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/backend/
git commit -m "feat: add Anthropic API and Claude CLI adapters"
```

---

### Task 6: OS operations

**Files:**
- Create: `src-tauri/src/osops.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod osops;`)

**Interfaces:**
- Consumes: nothing
- Produces:
  - `osops::OsError` (thiserror enum)
  - `osops::OsOps` trait:
    - `fn read_clipboard(&self) -> Result<String, OsError>`
    - `fn write_clipboard(&self, text: &str) -> Result<(), OsError>`
    - `fn clear_clipboard(&self) -> Result<(), OsError>`
    - `fn modifiers_down(&self) -> bool`
    - `fn foreground_window(&self) -> isize`
    - `fn send_copy(&self) -> Result<(), OsError>`
    - `fn send_paste(&self) -> Result<(), OsError>`
    - `fn sleep_ms(&self, ms: u64)`
    - `fn now_ms(&self) -> u64`
  - `osops::WindowsOps::new() -> WindowsOps`

Everything the cycle engine touches outside its own process goes through this trait. That is what lets Task 7's tests prove the safety rules without a GUI.

- [ ] **Step 1: Write the implementation**

There are no unit tests for this task — every method is a thin wrapper over an OS call, and a test would only assert that a mock was called. It is verified by the smoke binary from Task 1 and by hand in Task 9.

Create `src-tauri/src/osops.rs`:

```rust
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use thiserror::Error;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

#[derive(Debug, Error)]
pub enum OsError {
    #[error("clipboard error: {0}")]
    Clipboard(String),
    #[error("synthetic input error: {0}")]
    Input(String),
}

pub trait OsOps: Send + Sync {
    fn read_clipboard(&self) -> Result<String, OsError>;
    fn write_clipboard(&self, text: &str) -> Result<(), OsError>;
    fn clear_clipboard(&self) -> Result<(), OsError>;
    fn modifiers_down(&self) -> bool;
    fn foreground_window(&self) -> isize;
    fn send_copy(&self) -> Result<(), OsError>;
    fn send_paste(&self) -> Result<(), OsError>;
    fn sleep_ms(&self, ms: u64);
    fn now_ms(&self) -> u64;
}

pub struct WindowsOps {
    clipboard: Mutex<Clipboard>,
    enigo: Mutex<Enigo>,
    start: Instant,
}

impl WindowsOps {
    pub fn new() -> Result<Self, OsError> {
        Ok(Self {
            clipboard: Mutex::new(
                Clipboard::new().map_err(|e| OsError::Clipboard(e.to_string()))?,
            ),
            enigo: Mutex::new(
                Enigo::new(&Settings::default()).map_err(|e| OsError::Input(e.to_string()))?,
            ),
            start: Instant::now(),
        })
    }

    fn chord(&self, letter: char) -> Result<(), OsError> {
        let mut enigo = self.enigo.lock().map_err(|_| OsError::Input("poisoned".into()))?;
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| OsError::Input(e.to_string()))?;
        let click = enigo.key(Key::Unicode(letter), Direction::Click);
        // Release Control even if the letter failed, so a failure never leaves
        // a modifier logically stuck down for the user.
        let release = enigo.key(Key::Control, Direction::Release);
        click.map_err(|e| OsError::Input(e.to_string()))?;
        release.map_err(|e| OsError::Input(e.to_string()))?;
        Ok(())
    }
}

impl OsOps for WindowsOps {
    fn read_clipboard(&self) -> Result<String, OsError> {
        let mut cb = self.clipboard.lock().map_err(|_| OsError::Clipboard("poisoned".into()))?;
        match cb.get_text() {
            Ok(text) => Ok(text),
            // An empty or non-text clipboard is not an error for our purposes.
            Err(_) => Ok(String::new()),
        }
    }

    fn write_clipboard(&self, text: &str) -> Result<(), OsError> {
        let mut cb = self.clipboard.lock().map_err(|_| OsError::Clipboard("poisoned".into()))?;
        cb.set_text(text.to_string()).map_err(|e| OsError::Clipboard(e.to_string()))
    }

    fn clear_clipboard(&self) -> Result<(), OsError> {
        self.write_clipboard("")
    }

    fn modifiers_down(&self) -> bool {
        unsafe {
            [VK_SHIFT, VK_CONTROL, VK_MENU]
                .iter()
                .any(|vk| (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0)
        }
    }

    fn foreground_window(&self) -> isize {
        unsafe { GetForegroundWindow().0 as isize }
    }

    fn send_copy(&self) -> Result<(), OsError> {
        self.chord('c')
    }

    fn send_paste(&self) -> Result<(), OsError> {
        self.chord('v')
    }

    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}
```

`read_clipboard` deliberately maps an error to an empty string. A non-text clipboard (an image, a file list) is a normal state, not a failure, and the cycle already treats empty as "nothing captured". This is also the known limitation that a non-text clipboard cannot be restored — it will come back as empty text. Note it in your report.

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: compiles with no warnings. If the `windows` crate feature names are wrong for the resolved version, fix the `Cargo.toml` features rather than changing the API calls, and record what you changed.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/osops.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: add OsOps trait and Windows implementation"
```

---

### Task 7: The cycle engine

**Files:**
- Create: `src-tauri/src/cycle.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod cycle;`)

**Interfaces:**
- Consumes: `osops::OsOps`, `backend::{Backend, BackendError}`, `sentinel`
- Produces:
  - `cycle::CycleOutcome { pasted: bool, reason: &'static str, captured_ms: u64, backend_ms: u64 }`
  - `cycle::run(backend: &dyn Backend, system_prompt: &str, ops: &dyn OsOps) -> CycleOutcome`

**This is the safety-critical task.** Its tests are the only thing standing between the app and pasting wrong text into a real document.

- [ ] **Step 1: Write the failing tests**

Create the test module at the bottom of `src-tauri/src/cycle.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::backend::{Backend, BackendError};
    use crate::osops::{OsError, OsOps};

    #[derive(Default)]
    struct FakeOps {
        clipboard: Mutex<String>,
        calls: Mutex<Vec<String>>,
        pasted_content: Mutex<Option<String>>,
        now: Mutex<u64>,
        capture_yields: Mutex<Option<String>>,
        modifiers_sequence: Mutex<Vec<bool>>,
        focus: Mutex<isize>,
        focus_after_capture: Mutex<Option<isize>>,
    }

    impl FakeOps {
        fn new(original: &str, captured: Option<&str>) -> Self {
            let ops = Self::default();
            *ops.clipboard.lock().unwrap() = original.to_string();
            *ops.capture_yields.lock().unwrap() = captured.map(|c| c.to_string());
            *ops.focus.lock().unwrap() = 1;
            ops
        }
        fn log(&self, what: &str) {
            self.calls.lock().unwrap().push(what.to_string());
        }
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl OsOps for FakeOps {
        fn read_clipboard(&self) -> Result<String, OsError> {
            Ok(self.clipboard.lock().unwrap().clone())
        }
        fn write_clipboard(&self, text: &str) -> Result<(), OsError> {
            self.log(&format!("write({text})"));
            *self.clipboard.lock().unwrap() = text.to_string();
            Ok(())
        }
        fn clear_clipboard(&self) -> Result<(), OsError> {
            self.log("clear");
            *self.clipboard.lock().unwrap() = String::new();
            Ok(())
        }
        fn modifiers_down(&self) -> bool {
            let mut seq = self.modifiers_sequence.lock().unwrap();
            if seq.is_empty() { false } else { seq.remove(0) }
        }
        fn foreground_window(&self) -> isize {
            *self.focus.lock().unwrap()
        }
        fn send_copy(&self) -> Result<(), OsError> {
            self.log("copy");
            if let Some(text) = self.capture_yields.lock().unwrap().clone() {
                *self.clipboard.lock().unwrap() = text;
            }
            if let Some(next) = *self.focus_after_capture.lock().unwrap() {
                *self.focus.lock().unwrap() = next;
            }
            Ok(())
        }
        fn send_paste(&self) -> Result<(), OsError> {
            self.log("paste");
            *self.pasted_content.lock().unwrap() = Some(self.clipboard.lock().unwrap().clone());
            Ok(())
        }
        fn sleep_ms(&self, ms: u64) {
            self.log("sleep");
            *self.now.lock().unwrap() += ms;
        }
        fn now_ms(&self) -> u64 {
            let mut now = self.now.lock().unwrap();
            *now += 20;
            *now
        }
    }

    struct StubBackend {
        result: Result<String, BackendError>,
    }
    impl StubBackend {
        fn ok(text: &str) -> Self {
            Self { result: Ok(text.to_string()) }
        }
        fn failing() -> Self {
            Self { result: Err(BackendError::Timeout) }
        }
    }
    impl Backend for StubBackend {
        fn name(&self) -> &str { "stub" }
        fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
            self.result.clone()
        }
    }

    #[test]
    fn happy_path_pastes_the_extracted_text() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert!(outcome.pasted);
        assert_eq!(outcome.reason, "ok");
        assert_eq!(ops.pasted_content.lock().unwrap().clone().unwrap(), "Corrected.");
    }

    #[test]
    fn happy_path_restores_the_original_clipboard() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn happy_path_call_order_is_pinned() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert_eq!(
            ops.calls(),
            vec![
                "clear".to_string(),
                "copy".to_string(),
                "write(Corrected.)".to_string(),
                "paste".to_string(),
                "sleep".to_string(),
                "write(ORIGINAL)".to_string(),
            ]
        );
    }

    #[test]
    fn capture_timeout_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", None);
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "capture-timeout");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn backend_error_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::failing(), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "backend-error");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn missing_sentinel_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("The corrected text: Corrected."), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "no-sentinel");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn truncated_sentinel_never_pastes() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected."), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "no-sentinel");
        assert!(ops.pasted_content.lock().unwrap().is_none());
    }

    #[test]
    fn stuck_modifiers_abort_before_any_clipboard_change() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.modifiers_sequence.lock().unwrap() = vec![true; 500];
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "modifiers-stuck");
        assert!(!ops.calls().contains(&"clear".to_string()));
        assert!(!ops.calls().contains(&"copy".to_string()));
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn focus_change_between_capture_and_paste_never_pastes() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.focus_after_capture.lock().unwrap() = Some(999);
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "focus-changed");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn a_panic_mid_cycle_still_restores_the_clipboard() {
        struct Panicking;
        impl Backend for Panicking {
            fn name(&self) -> &str { "panicking" }
            fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
                panic!("backend exploded");
            }
        }
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(&Panicking, "Fix.", &ops)
        }));
        assert!(result.is_err(), "the panic should propagate, not be swallowed");
        assert_eq!(
            *ops.clipboard.lock().unwrap(),
            "ORIGINAL",
            "the guard must restore even when the cycle unwinds"
        );
        assert!(ops.pasted_content.lock().unwrap().is_none());
    }

    #[test]
    fn the_wrapped_system_prompt_reaches_the_backend() {
        struct Capturing(Mutex<Option<String>>);
        impl Backend for Capturing {
            fn name(&self) -> &str { "capturing" }
            fn transform(&self, s: &str, _t: &str) -> Result<String, BackendError> {
                *self.0.lock().unwrap() = Some(s.to_string());
                Ok("<output>x</output>".into())
            }
        }
        let backend = Capturing(Mutex::new(None));
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        run(&backend, "Fix grammar.", &ops);
        let seen = backend.0.lock().unwrap().clone().unwrap();
        assert!(seen.contains("Fix grammar."));
        assert!(seen.contains(crate::sentinel::SENTINEL_INSTRUCTION));
    }
}
```

`happy_path_call_order_is_pinned` is the most important test here. The Python spike proved that without it, deleting the clipboard clear or the restore delay leaves a full suite green — and those two lines are the whole mechanism preventing stale-clipboard capture.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test cycle`
Expected: compile error — `cycle` module does not exist yet.

- [ ] **Step 3: Write the implementation**

Create `src-tauri/src/cycle.rs` above the test module:

```rust
use crate::backend::Backend;
use crate::osops::OsOps;
use crate::sentinel;

const CAPTURE_TIMEOUT_MS: u64 = 1000;
const CAPTURE_POLL_MS: u64 = 20;
const MODIFIER_TIMEOUT_MS: u64 = 2000;
const MODIFIER_POLL_MS: u64 = 10;
const RESTORE_DELAY_MS: u64 = 300;

#[derive(Debug, Clone)]
pub struct CycleOutcome {
    pub pasted: bool,
    pub reason: &'static str,
    pub captured_ms: u64,
    pub backend_ms: u64,
}

fn wait_for_modifier_release(ops: &dyn OsOps) -> bool {
    let start = ops.now_ms();
    loop {
        if !ops.modifiers_down() {
            return true;
        }
        if ops.now_ms().saturating_sub(start) >= MODIFIER_TIMEOUT_MS {
            return false;
        }
        ops.sleep_ms(MODIFIER_POLL_MS);
    }
}

fn wait_for_clipboard(ops: &dyn OsOps) -> Option<String> {
    let start = ops.now_ms();
    loop {
        if let Ok(value) = ops.read_clipboard() {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
        if ops.now_ms().saturating_sub(start) >= CAPTURE_TIMEOUT_MS {
            return None;
        }
        ops.sleep_ms(CAPTURE_POLL_MS);
    }
}

/// Restores the clipboard when it goes out of scope, on every path.
///
/// This is the Rust equivalent of a `finally` block, and it is deliberately
/// unconditional: there is no `disarm`. Every exit from `run` — an early
/// return, or an unwinding panic from anywhere in the cycle — drops this
/// guard and restores what the user had. Making restoration a property of
/// scope rather than a step someone has to remember on each path removes the
/// entire class of "this abort forgot to restore" bug.
struct ClipboardGuard<'a> {
    ops: &'a dyn OsOps,
    original: String,
}

impl Drop for ClipboardGuard<'_> {
    fn drop(&mut self) {
        let _ = self.ops.write_clipboard(&self.original);
    }
}

/// One hotkey press, end to end.
///
/// Every exit restores the original clipboard, and no exit except the happy
/// path reaches `send_paste`.
pub fn run(backend: &dyn Backend, system_prompt: &str, ops: &dyn OsOps) -> CycleOutcome {
    let original = ops.read_clipboard().unwrap_or_default();
    let _guard = ClipboardGuard { ops, original };

    let abort = |reason: &'static str, captured_ms: u64, backend_ms: u64| CycleOutcome {
        pasted: false,
        reason,
        captured_ms,
        backend_ms,
    };

    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck", 0, 0);
    }

    let _ = ops.clear_clipboard();
    if ops.send_copy().is_err() {
        return abort("capture-timeout", 0, 0);
    }

    let capture_start = ops.now_ms();
    let captured = match wait_for_clipboard(ops) {
        Some(text) => text,
        None => return abort("capture-timeout", 0, 0),
    };
    let captured_ms = ops.now_ms().saturating_sub(capture_start);
    let focus_at_capture = ops.foreground_window();

    let wrapped = sentinel::wrap_system_prompt(system_prompt);
    let backend_start = ops.now_ms();
    let raw = match backend.transform(&wrapped, &captured) {
        Ok(text) => text,
        Err(_) => return abort("backend-error", captured_ms, 0),
    };
    let backend_ms = ops.now_ms().saturating_sub(backend_start);

    let extracted = match sentinel::extract(&raw) {
        Some(text) => text,
        None => return abort("no-sentinel", captured_ms, backend_ms),
    };
    if extracted.trim().is_empty() {
        return abort("empty-result", captured_ms, backend_ms);
    }

    if ops.foreground_window() != focus_at_capture {
        return abort("focus-changed", captured_ms, backend_ms);
    }
    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck-before-paste", captured_ms, backend_ms);
    }

    if ops.write_clipboard(&extracted).is_err() {
        return abort("backend-error", captured_ms, backend_ms);
    }
    if ops.send_paste().is_err() {
        return abort("backend-error", captured_ms, backend_ms);
    }

    // The guard restores after this sleep, when it drops at end of scope.
    ops.sleep_ms(RESTORE_DELAY_MS);

    CycleOutcome { pasted: true, reason: "ok", captured_ms, backend_ms }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test cycle`
Expected: PASS, 11 tests

The panic test prints a panic backtrace to stderr while passing. That is expected, not noise — `catch_unwind` re-raises after the guard has run.

- [ ] **Step 5: Verify the call-order test actually pins the order**

The Python spike proved that deleting the clipboard clear or the restore delay left a full suite green, so confirm this suite is not similarly blind:

1. Comment out `let _ = ops.clear_clipboard();` — run `cargo test cycle`, expect `happy_path_call_order_is_pinned` to FAIL. Restore it.
2. Comment out `ops.sleep_ms(RESTORE_DELAY_MS);` — run again, expect the same test to FAIL. Restore it.

Report what you observed. If either mutation leaves the suite green, the test is not doing its job and needs fixing before this task is done.

- [ ] **Step 6: Run the whole suite**

Run: `cd src-tauri && cargo test`
Expected: PASS, 46 tests (7 config + 11 sentinel + 17 backend + 11 cycle)

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/cycle.rs src-tauri/src/lib.rs
git commit -m "feat: add cycle engine with abort-without-pasting guarantees"
```

---

### Task 8: Application wiring

**Files:**
- Modify: `src-tauri/src/lib.rs` (replace the Task 1 smoke wiring entirely)

**Interfaces:**
- Consumes: everything from Tasks 2–7
- Produces: the running application

No unit tests. This is the untestable shell — tray, shortcut registration, threading — and keeping it thin is why `cycle.rs` exists separately. Do not put logic here; if you want a conditional that decides behaviour, it belongs in `cycle.rs`.

- [ ] **Step 1: Write the implementation**

Replace `src-tauri/src/lib.rs` entirely:

```rust
mod backend;
mod config;
mod cycle;
mod osops;
mod sentinel;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

struct AppState {
    prompts: Mutex<HashMap<String, (String, Arc<dyn backend::Backend>)>>,
    ops: Arc<dyn osops::OsOps>,
    busy: AtomicBool,
    last_status: Mutex<String>,
}

/// Build the hotkey -> (system prompt, backend) map from config.
///
/// A prompt whose hotkey will not parse, or whose backend will not build, is
/// skipped with a message rather than aborting the whole load — one bad entry
/// must not take the other prompts down with it.
fn build_prompt_map(
    cfg: &config::Config,
) -> (HashMap<String, (String, Arc<dyn backend::Backend>)>, Vec<String>) {
    let mut map = HashMap::new();
    let mut problems = Vec::new();
    for prompt in &cfg.prompts {
        let Some(backend_cfg) = cfg.backend(&prompt.backend_id) else {
            problems.push(format!("{}: unknown backend", prompt.name));
            continue;
        };
        match backend::build(backend_cfg) {
            Ok(b) => {
                map.insert(prompt.hotkey.clone(), (prompt.system_prompt.clone(), b));
            }
            Err(e) => problems.push(format!("{}: {e}", prompt.name)),
        }
    }
    (map, problems)
}

fn register_hotkeys(app: &tauri::AppHandle, hotkeys: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for hotkey in hotkeys {
        match hotkey.parse::<Shortcut>() {
            Ok(shortcut) => {
                if let Err(e) = app.global_shortcut().register(shortcut) {
                    problems.push(format!("{hotkey}: already in use ({e})"));
                }
            }
            Err(e) => problems.push(format!("{hotkey}: unparseable ({e})")),
        }
    }
    problems
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();

            let path = config::config_path(&handle);
            let cfg = config::load_or_create(&path).map_err(|e| {
                eprintln!("config error: {e}");
                e
            })?;
            let (prompt_map, mut problems) = build_prompt_map(&cfg);
            let hotkeys: Vec<String> = prompt_map.keys().cloned().collect();

            let state = Arc::new(AppState {
                prompts: Mutex::new(prompt_map),
                ops: Arc::new(osops::WindowsOps::new()?),
                busy: AtomicBool::new(false),
                last_status: Mutex::new("ready".into()),
            });

            let handler_state = state.clone();
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |_app, shortcut, event| {
                        if event.state() != ShortcutState::Released {
                            return;
                        }
                        let key = shortcut.to_string();
                        let state = handler_state.clone();

                        if state.busy.swap(true, Ordering::SeqCst) {
                            println!("busy; hotkey ignored");
                            return;
                        }

                        // The cycle takes seconds. Running it here would stall
                        // the shortcut listener for its whole duration.
                        std::thread::spawn(move || {
                            let entry = state
                                .prompts
                                .lock()
                                .ok()
                                .and_then(|m| m.get(&key).cloned());
                            if let Some((system_prompt, backend)) = entry {
                                let outcome =
                                    cycle::run(backend.as_ref(), &system_prompt, state.ops.as_ref());
                                let status = format!(
                                    "{} — {} (capture {} ms, backend {} ms)",
                                    backend.name(),
                                    outcome.reason,
                                    outcome.captured_ms,
                                    outcome.backend_ms
                                );
                                println!("{status}");
                                if let Ok(mut last) = state.last_status.lock() {
                                    *last = status;
                                }
                            }
                            state.busy.store(false, Ordering::SeqCst);
                        });
                    })
                    .build(),
            )?;

            problems.extend(register_hotkeys(&handle, &hotkeys));
            for problem in &problems {
                eprintln!("config problem: {problem}");
            }

            let open_config = MenuItem::with_id(app, "open-config", "Open config file", true, None::<&str>)?;
            let reload = MenuItem::with_id(app, "reload", "Reload config", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_config, &reload, &separator, &quit])?;

            let menu_path = path.clone();
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Clipboard AI")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open-config" => {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", &menu_path.to_string_lossy()])
                            .spawn();
                    }
                    "reload" => {
                        println!("reload requested — restart the app to apply config changes");
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Delete `src-tauri/src/bin/smoke.rs` — Task 1's diagnostic has served its purpose and the real app supersedes it.

**Verify `Shortcut: FromStr` before relying on it.** `register_hotkeys` calls `hotkey.parse::<Shortcut>()`, and the plugin's published docs demonstrate only `Shortcut::new(Some(Modifiers::CONTROL), Code::KeyJ)`. If `FromStr` is not implemented, or does not accept the `"Ctrl+Alt+J"` spelling the config uses, write a small parser that splits on `+`, maps `ctrl`/`control`/`alt`/`shift`/`super` to `Modifiers` flags and the final segment to a `Code`, and returns a `ConfigError` for anything unrecognised. Do not guess at the accepted string format — check the crate's source in `~/.cargo/registry` or its docs.rs page, and say in your report which you used and what you found.

**Note the deliberate narrowing:** the spec calls for a watched, hot-reloading config. This wiring registers hotkeys once at startup and the Reload item only prints a message. Live re-registration means unregistering the old shortcuts and rebuilding backends under a lock, and it is not worth the concurrency risk in the first pass. **Report this as a known deviation** so it is triaged rather than lost; it lands with the settings UI in v2, where reload has a real trigger.

- [ ] **Step 2: Verify it builds and the suite still passes**

Run: `cd src-tauri && cargo build && cargo test`
Expected: builds, 46 tests pass.

- [ ] **Step 3: Commit**

```bash
git add -A src-tauri/src
git commit -m "feat: wire tray, hotkey registration and cycle dispatch"
```

---

### Task 9: Status and logging

**Files:**
- Create: `src-tauri/src/status.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `cycle::CycleOutcome`, `config::Config`
- Produces:
  - `status::log_line(outcome: &CycleOutcome, prompt_name: &str, backend_name: &str) -> String`
  - `status::append_log(dir: &Path, line: &str)` — appends to `clipboard-ai.log`, truncating the file when it exceeds 1 MB
  - `status::summary(outcome: &CycleOutcome, prompt_name: &str) -> String` — the short form shown in the tray

**Why this is not optional polish.** Every diagnostic in Tasks 1–8 goes to stdout, and a released Tauri app has no console. Without this, a user whose Ollama is down sees a hotkey that does nothing, with no way to find out why — which is precisely the failure mode the Python spike kept producing until we made errors visible.

- [ ] **Step 1: Write the failing tests**

Create the test module at the bottom of `src-tauri/src/status.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cycle::CycleOutcome;

    fn outcome(pasted: bool, reason: &'static str) -> CycleOutcome {
        CycleOutcome { pasted, reason, captured_ms: 42, backend_ms: 1234 }
    }

    #[test]
    fn a_log_line_carries_prompt_backend_outcome_and_timings() {
        let line = log_line(&outcome(true, "ok"), "Fix grammar", "Local Ollama");
        assert!(line.contains("Fix grammar"));
        assert!(line.contains("Local Ollama"));
        assert!(line.contains("ok"));
        assert!(line.contains("42"));
        assert!(line.contains("1234"));
    }

    #[test]
    fn a_failed_cycle_is_visibly_marked_in_the_log() {
        let line = log_line(&outcome(false, "backend-error"), "Fix grammar", "Local Ollama");
        assert!(line.contains("backend-error"));
        assert!(line.contains("ABORTED"));
    }

    #[test]
    fn the_tray_summary_names_the_reason_on_failure() {
        let text = summary(&outcome(false, "capture-timeout"), "Fix grammar");
        assert!(text.contains("capture-timeout"));
    }

    #[test]
    fn the_tray_summary_is_short_enough_for_a_menu_item() {
        let text = summary(&outcome(true, "ok"), "Fix grammar");
        assert!(text.len() <= 60, "menu labels must stay short, got {}", text.len());
    }

    #[test]
    fn appending_creates_the_file_and_keeps_earlier_lines() {
        let dir = std::env::temp_dir().join(format!("clipboard-ai-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clipboard-ai.log");
        let _ = std::fs::remove_file(&path);

        append_log(&dir, "first");
        append_log(&dir, "second");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("first"));
        assert!(contents.contains("second"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test status`
Expected: compile error — `status` module does not exist yet.

- [ ] **Step 3: Write the implementation**

Create `src-tauri/src/status.rs` above the test module:

```rust
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use crate::cycle::CycleOutcome;

const MAX_LOG_BYTES: u64 = 1_048_576;

pub fn log_line(outcome: &CycleOutcome, prompt_name: &str, backend_name: &str) -> String {
    let verdict = if outcome.pasted { "PASTED" } else { "ABORTED" };
    format!(
        "{verdict} prompt={prompt_name:?} backend={backend_name:?} reason={} capture_ms={} backend_ms={}",
        outcome.reason, outcome.captured_ms, outcome.backend_ms
    )
}

pub fn summary(outcome: &CycleOutcome, prompt_name: &str) -> String {
    if outcome.pasted {
        format!("{prompt_name}: ok ({} ms)", outcome.backend_ms)
    } else {
        format!("{prompt_name}: {}", outcome.reason)
    }
}

/// Append one line to `clipboard-ai.log`, truncating first if it has grown
/// past MAX_LOG_BYTES. A logging failure is swallowed deliberately: losing a
/// log line must never take down a cycle that otherwise succeeded.
pub fn append_log(dir: &Path, line: &str) {
    let path = dir.join("clipboard-ai.log");
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = fs::remove_file(&path);
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test status`
Expected: PASS, 5 tests

- [ ] **Step 5: Wire it into the tray and the cycle**

In `src-tauri/src/lib.rs`:

1. Add `mod status;`.
2. Build a disabled menu item that shows the last outcome, and keep its handle:
   `let status_item = MenuItem::with_id(app, "status", "ready", false, None::<&str>)?;`
   Put it first in `Menu::with_items`, followed by a separator, then the existing items.
3. Add one informational, disabled menu item per configured prompt showing `"<name> — <hotkey>"`, so the user can see what is registered without opening the config.
4. Store a clone of `status_item` and the config directory in `AppState`.
5. In the cycle thread, after `cycle::run` returns, call `status::append_log` with `status::log_line(...)` and set the status item's text to `status::summary(...)` (the `MenuItem` handle exposes a `set_text` method — verify the exact name against the Tauri v2 menu API and say which you used).
6. Add a `Show log` menu item that opens the log file with the same `cmd /C start` approach used for the config file.

- [ ] **Step 6: Verify the whole suite still passes**

Run: `cd src-tauri && cargo build && cargo test`
Expected: builds, 51 tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/status.rs src-tauri/src/lib.rs
git commit -m "feat: add log file and tray status so failures are visible"
```

---

### Task 10: Manual verification

**Files:**
- Create: `docs/superpowers/notes/2026-08-07-v1-verification.md`

**Interfaces:**
- Consumes: the working app
- Produces: a record of what works

This task is the human's. Everything before it is machine-verifiable; this is not.

- [ ] **Step 1: Start the app**

```bash
npm run tauri dev
```

Expected: no window, a tray icon appears, the terminal prints any config problems.

- [ ] **Step 2: Verify the safe-abort path first**

Click into an empty Notepad with nothing selected. Press `Ctrl+Alt+J`.
Expected: the terminal prints `capture-timeout`, nothing is pasted, the clipboard is unchanged.

**If anything pastes here, stop.** That is the failure every design decision in this plan exists to prevent.

- [ ] **Step 3: Verify the happy path**

Type `this sentance have two mistake` in Notepad, select it, press `Ctrl+Alt+J`.
Expected: the selection is replaced with corrected text, the terminal prints `ok` with capture and backend timings, and the clipboard holds what it held before.

- [ ] **Step 4: Verify the sentinel rescues `claude -p`**

Edit the config file (tray → Open config file), point the `fix-grammar` prompt at `claude-sub`, and restart the app. Repeat Step 3.
Expected: the pasted text is the corrected sentence **with no `The corrected text:` preamble.** The Python spike showed the CLI adds one; if the sentinel is working, it is discarded here.

- [ ] **Step 5: Verify the focus guard**

Select text, press `Ctrl+Alt+J`, then immediately click into a different window.
Expected: `focus-changed` in the terminal, nothing pasted anywhere.

- [ ] **Step 6: Verify multiple prompts and hotkeys**

Add a second prompt to the config with a different hotkey (e.g. `Ctrl+Alt+K`) and a different system prompt such as `Rewrite this text to be more concise.`, pointing at a different backend. Restart. Verify both hotkeys work and each uses its own prompt and backend.

- [ ] **Step 7: Verify the app survives a dead backend**

Stop Ollama, then press the hotkey.
Expected: `backend-error` shown in the **tray status item** (not just the terminal), nothing pasted, clipboard intact, **and the app keeps running** — the next press retries. Check that the log file records the failure via tray → Show log.

- [ ] **Step 8: Verify it works as a built app, not just in dev**

```bash
npm run tauri build
```

Run the produced executable from `src-tauri/target/release/`. This is the configuration that matters: there is no console, so the tray status and log file are the only way to see what happened. Repeat Steps 2, 3 and 7 against the built binary.

Expected: identical behaviour, with failures visible in the tray rather than nowhere.

- [ ] **Step 9: Write up and commit**

Record in `docs/superpowers/notes/2026-08-07-v1-verification.md`: which steps passed, the capture and backend timings observed, whether the sentinel removed the CLI preamble, whether failures were visible in the built app, and anything that behaved differently from the Python spike — especially capture latency, since this is a different input stack.

```bash
git add docs/superpowers/notes/
git commit -m "docs: record v1 manual verification results"
```

---

## After this plan

v1 is a usable daily driver. v2 is the settings UI, for which the user has separate specs, and it also picks up the two items deliberately narrowed here: live config reload, and the review-before-replace write-back mode. Formatting preservation is its own cycle after that.
