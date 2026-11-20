use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::config::{self, Config, Formatting};
use crate::cycle::{self, Captured};
use crate::hud::{HudView, Look, REASON_PALETTE_FOCUS_FAILED};
use crate::overlay::{self, Anchor};
use crate::runner::{self, Job, PALETTE_INSTRUCTION_ID};
use crate::{backend, recover_lock, status, AppState};

pub const WINDOW_LABEL: &str = "palette";
pub const OPEN_EVENT: &str = "palette://open";
const STATE_FILE: &str = "palette.json";
const GAP_LOGICAL: f64 = 6.0;
const OPEN_RETRY_MS: u64 = 600;
const OPEN_GIVE_UP_MS: u64 = 900;
const FOCUS_VERIFY_MS: u64 = 450;
const BLUR_SETTLE_MS: u64 = 60;
const MAX_INSTRUCTION_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaletteCommand {
    pub id: String,
    pub name: String,
    pub hotkey: String,
    pub backend_name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaletteBackend {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenEvent {
    session: u64,
    #[serde(flatten)]
    look: Look,
    commands: Vec<PaletteCommand>,
    backends: Vec<PaletteBackend>,
    backend_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PaletteChoice {
    Command { command_id: String },
    Instruction { text: String, backend_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Remembered {
    #[serde(default)]
    backend_id: Option<String>,
}

struct Session {
    id: u64,
    captured: Captured,
    anchor: Anchor,
    shown: bool,
    focused: bool,
    above: Option<bool>,
}

struct Inner {
    session: Mutex<Option<Session>>,
    next_id: AtomicU64,
}

#[derive(Clone)]
pub struct Palette {
    inner: Arc<Inner>,
}

impl Default for Palette {
    fn default() -> Self {
        Self { inner: Arc::new(Inner { session: Mutex::new(None), next_id: AtomicU64::new(0) }) }
    }
}

pub fn ai_backends(cfg: &Config) -> Vec<PaletteBackend> {
    cfg.backends
        .iter()
        .filter(|b| b.kind != config::KIND_HTTP)
        .map(|b| PaletteBackend { id: b.id.clone(), name: b.name.clone() })
        .collect()
}

pub fn default_backend(backends: &[PaletteBackend], remembered: Option<&str>) -> Option<String> {
    remembered
        .filter(|id| backends.iter().any(|b| b.id == *id))
        .map(str::to_string)
        .or_else(|| backends.first().map(|b| b.id.clone()))
}

pub fn palette_commands(cfg: &Config) -> Vec<PaletteCommand> {
    cfg.commands
        .iter()
        .map(|c| PaletteCommand {
            id: c.id.clone(),
            name: c.name.clone(),
            hotkey: c.hotkey.clone(),
            backend_name: cfg.backend(&c.backend_id).map(|b| b.name.clone()).unwrap_or_default(),
        })
        .collect()
}

pub fn instruction_name(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_INSTRUCTION_NAME_CHARS {
        return collapsed;
    }
    let mut cut: String = collapsed.chars().take(MAX_INSTRUCTION_NAME_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[derive(Debug, Clone)]
pub struct ResolvedChoice {
    pub command_id: String,
    pub command_name: String,
    pub backend: config::BackendConfig,
    pub system_prompt: String,
    pub formatting: Formatting,
}

pub fn resolve_choice(cfg: &Config, choice: &PaletteChoice) -> Result<ResolvedChoice, String> {
    match choice {
        PaletteChoice::Command { command_id } => {
            let command = cfg
                .commands
                .iter()
                .find(|c| c.id == *command_id)
                .ok_or_else(|| format!("command {command_id} no longer exists"))?;
            let backend = cfg
                .backend(&command.backend_id)
                .ok_or_else(|| format!("{}: unknown backend", command.name))?;
            Ok(ResolvedChoice {
                command_id: command.id.clone(),
                command_name: command.name.clone(),
                backend: backend.clone(),
                system_prompt: command.system_prompt.clone(),
                formatting: command.formatting,
            })
        }
        PaletteChoice::Instruction { text, backend_id } => {
            let text = text.trim();
            if text.is_empty() {
                return Err("the instruction is empty".into());
            }
            let backend = cfg
                .backend(backend_id)
                .filter(|b| b.kind != config::KIND_HTTP)
                .ok_or_else(|| format!("backend {backend_id} cannot take an instruction"))?;
            Ok(ResolvedChoice {
                command_id: PALETTE_INSTRUCTION_ID.to_string(),
                command_name: instruction_name(text),
                backend: backend.clone(),
                system_prompt: text.to_string(),
                formatting: Formatting::Auto,
            })
        }
    }
}

fn load_remembered(dir: &Path) -> Option<String> {
    let raw = fs::read_to_string(dir.join(STATE_FILE)).ok()?;
    serde_json::from_str::<Remembered>(&raw).ok()?.backend_id
}

fn save_remembered(dir: &Path, backend_id: &str) {
    let value = Remembered { backend_id: Some(backend_id.to_string()) };
    if let Ok(json) = serde_json::to_string_pretty(&value) {
        if let Err(e) = fs::write(dir.join(STATE_FILE), json) {
            status::append_log(dir, &format!("PALETTE could not remember the backend: {e}"));
        }
    }
}

fn log(state: &AppState, line: &str) {
    println!("{line}");
    status::append_log(&state.config_dir, line);
}

fn raw_window(app: &AppHandle) -> Option<isize> {
    app.get_webview_window(WINDOW_LABEL)?.hwnd().ok().map(|h| h.0 as isize)
}

fn hide_window(app: &AppHandle) {
    let app_for_main = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(raw) = raw_window(&app_for_main) {
            overlay::hide_without_activating(raw);
        }
    });
}

fn release_busy(state: &AppState) {
    state.busy.store(false, Ordering::SeqCst);
}

impl Palette {
    fn take(&self, id: Option<u64>) -> Option<Session> {
        let mut slot = recover_lock(self.inner.session.lock(), "palette session", None);
        match (&*slot, id) {
            (Some(session), Some(id)) if session.id != id => None,
            (Some(_), _) => slot.take(),
            (None, _) => None,
        }
    }

    pub fn is_open(&self) -> bool {
        recover_lock(self.inner.session.lock(), "palette session", None).is_some()
    }

    fn with_session<R>(&self, id: u64, f: impl FnOnce(&mut Session) -> R) -> Option<R> {
        let mut slot = recover_lock(self.inner.session.lock(), "palette session", None);
        slot.as_mut().filter(|s| s.id == id).map(f)
    }
}

pub fn toggle(app: &AppHandle, state: &AppState) {
    if state.palette.is_open() {
        close(app, state, None, true);
        return;
    }
    if state.busy.swap(true, Ordering::SeqCst) {
        log(state, "busy; palette hotkey ignored");
        return;
    }
    let app = app.clone();
    let state = state.clone();
    std::thread::spawn(move || {
        let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| open(&app, &state)));
        match opened {
            Ok(true) => {}
            Ok(false) => release_busy(&state),
            Err(payload) => {
                let _ = state.palette.take(None);
                hide_window(&app);
                release_busy(&state);
                log(&state, &format!("PANIC palette open panicked: {}", crate::panic_message(&payload)));
            }
        }
    });
}

fn open(app: &AppHandle, state: &AppState) -> bool {
    let anchor = overlay::caret_anchor();
    let cfg = recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone();
    let look = Look::from_settings(&cfg.settings);

    let captured = match cycle::capture_selection(state.ops.as_ref()) {
        Ok(captured) => captured,
        Err(outcome) => {
            log(state, &format!("PALETTE not opened reason={}", outcome.reason));
            state.hud.notice(anchor, HudView::Stopped { reason: outcome.reason.to_string() }, look);
            return false;
        }
    };

    let backends = ai_backends(&cfg);
    let remembered = load_remembered(&state.config_dir);
    let event = OpenEvent {
        session: state.palette.inner.next_id.fetch_add(1, Ordering::SeqCst) + 1,
        look,
        commands: palette_commands(&cfg),
        backend_id: default_backend(&backends, remembered.as_deref()),
        backends,
    };
    let id = event.session;
    *recover_lock(state.palette.inner.session.lock(), "palette session", None) = Some(Session {
        id,
        captured,
        anchor,
        shown: false,
        focused: false,
        above: None,
    });
    let _ = app.emit_to(WINDOW_LABEL, OPEN_EVENT, event.clone());

    let app = app.clone();
    let state = state.clone();
    std::thread::spawn(move || {
        let pending = || state.palette.with_session(id, |s| !s.shown).unwrap_or(false);
        std::thread::sleep(Duration::from_millis(OPEN_RETRY_MS));
        if !pending() {
            return;
        }
        let _ = app.emit_to(WINDOW_LABEL, OPEN_EVENT, event);
        std::thread::sleep(Duration::from_millis(OPEN_GIVE_UP_MS));
        if !pending() {
            return;
        }
        let anchor = state.palette.with_session(id, |s| s.anchor);
        if state.palette.take(Some(id)).is_some() {
            log(&state, "PALETTE page never answered; gave up without showing it");
            release_busy(&state);
            if let Some(anchor) = anchor {
                state.hud.notice(
                    anchor,
                    HudView::Stopped { reason: REASON_PALETTE_FOCUS_FAILED.into() },
                    look,
                );
            }
        }
    });
    true
}

pub fn measured(app: &AppHandle, state: &AppState, id: u64, width: f64, height: f64, inset: f64) {
    let Some(raw) = raw_window(app) else { return };
    let decision = state.palette.with_session(id, |session| {
        let screen = overlay::screen_for(session.anchor.rect);
        let s = screen.scale;
        let inset_px = overlay::to_physical(inset, s);
        let content = (
            overlay::to_physical(width, s) - 2 * inset_px,
            overlay::to_physical(height, s) - 2 * inset_px,
        );
        let placement = overlay::place(
            session.anchor.rect,
            screen.work,
            content,
            inset_px,
            overlay::to_physical(GAP_LOGICAL, s),
            session.above,
        );
        session.above = Some(placement.above);
        let first_show = !session.shown;
        session.shown = true;
        (placement, first_show)
    });
    let Some((placement, first_show)) = decision else { return };

    if !first_show {
        overlay::move_without_activating(raw, placement);
        return;
    }
    overlay::move_without_activating(raw, placement);
    overlay::show_and_activate(raw);
    let focused = overlay::force_foreground(raw);
    if focused {
        let _ = state.palette.with_session(id, |s| s.focused = true);
    }

    let app = app.clone();
    let state = state.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(FOCUS_VERIFY_MS));
        let in_front = overlay::foreground() == raw;
        let lost = state.palette.with_session(id, |s| !s.focused && !in_front).unwrap_or(false);
        if lost {
            log(&state, "PALETTE could not take the keyboard focus; closed it");
            let anchor = state.palette.with_session(id, |s| s.anchor);
            if state.palette.take(Some(id)).is_some() {
                hide_window(&app);
                release_busy(&state);
                if let Some(anchor) = anchor {
                    let look = Look::from_settings(
                        &recover_lock(state.config.lock(), "config", Some(&state.config_dir)).settings,
                    );
                    state.hud.notice(
                        anchor,
                        HudView::Stopped { reason: REASON_PALETTE_FOCUS_FAILED.into() },
                        look,
                    );
                }
            }
        }
    });
}

pub fn focus_changed(app: &AppHandle, state: &AppState, focused: bool) {
    if focused {
        let mut slot = recover_lock(state.palette.inner.session.lock(), "palette session", None);
        if let Some(session) = slot.as_mut().filter(|s| s.shown) {
            session.focused = true;
        }
        return;
    }
    let id = match recover_lock(state.palette.inner.session.lock(), "palette session", None).as_ref() {
        Some(session) if session.focused => session.id,
        _ => return,
    };
    let app = app.clone();
    let state = state.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(BLUR_SETTLE_MS));
        let raw = raw_window(&app);
        if raw.is_some_and(|raw| overlay::foreground() == raw) {
            return;
        }
        if state.palette.take(Some(id)).is_some() {
            log(&state, "PALETTE closed: focus moved elsewhere");
            hide_window(&app);
            release_busy(&state);
        }
    });
}

pub fn close(app: &AppHandle, state: &AppState, id: Option<u64>, restore_focus: bool) {
    let Some(session) = state.palette.take(id) else { return };
    let app = app.clone();
    let state = state.clone();
    std::thread::spawn(move || {
        if restore_focus {
            let _ = state.ops.activate_window(session.captured.window);
        }
        hide_window(&app);
        release_busy(&state);
    });
}

pub fn run(app: &AppHandle, state: &AppState, id: u64, choice: PaletteChoice) {
    let Some(session) = state.palette.take(Some(id)) else { return };
    let cfg = recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone();
    let resolved = resolve_choice(&cfg, &choice)
        .and_then(|r| backend::build(&r.backend).map(|b| (r, b)).map_err(|e| e.to_string()));
    let app = app.clone();
    runner::spawn_guarded(state, "palette run", move |state| {
        let restored = state.ops.activate_window(session.captured.window);
        hide_window(&app);
        let (resolved, built) = match resolved {
            Ok(pair) => pair,
            Err(e) => {
                log(state, &format!("PALETTE run refused: {e}"));
                let outcome = cycle::CycleOutcome {
                    pasted: false,
                    reason: "backend-error",
                    detail: Some(e),
                    captured_ms: session.captured.captured_ms,
                    backend_ms: 0,
                    captured_text: Some(session.captured.clip.text.clone()),
                    result_text: None,
                    result_html: None,
                };
                let (id, name) = match &choice {
                    PaletteChoice::Command { command_id } => (command_id.clone(), command_id.clone()),
                    PaletteChoice::Instruction { text, .. } => {
                        (PALETTE_INSTRUCTION_ID.to_string(), instruction_name(text))
                    }
                };
                crate::record_history(state, &id, &name, "", &outcome, 0);
                let look = Look::from_settings(
                    &recover_lock(state.config.lock(), "config", Some(&state.config_dir)).settings,
                );
                state.hud.notice(session.anchor, HudView::from_outcome(&outcome), look);
                return;
            }
        };
        if let PaletteChoice::Instruction { backend_id, .. } = &choice {
            save_remembered(&state.config_dir, backend_id);
        }
        if !restored {
            log(state, "PALETTE could not bring the target window back; the run will abort");
        }
        let job = Job {
            command_id: resolved.command_id,
            command_name: resolved.command_name,
            backend: built,
            system_prompt: resolved.system_prompt,
            formatting: resolved.formatting,
        };
        runner::run_job(&app, state, job, Some(session.captured), Some(session.anchor));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        let mut cfg = config::default_config();
        cfg.backends.push(config::BackendConfig {
            id: "hook".into(),
            name: "Notes hook".into(),
            kind: config::KIND_HTTP.into(),
            base_url: Some("http://localhost/x".into()),
            model: None,
            api_key: None,
            timeout_ms: 120_000,
            effort: config::Effort::Low,
            expects_response: false,
        });
        cfg
    }

    #[test]
    fn only_backends_that_read_a_prompt_are_offered_for_instructions() {
        let ids: Vec<String> = ai_backends(&cfg()).into_iter().map(|b| b.id).collect();
        assert_eq!(ids, vec!["ollama-local", "claude-sub"], "the HTTP hook never sees the prompt");
    }

    #[test]
    fn the_remembered_backend_wins_while_it_still_exists() {
        let backends = ai_backends(&cfg());
        assert_eq!(default_backend(&backends, Some("claude-sub")).as_deref(), Some("claude-sub"));
        assert_eq!(default_backend(&backends, None).as_deref(), Some("ollama-local"));
        assert_eq!(
            default_backend(&backends, Some("deleted")).as_deref(),
            Some("ollama-local"),
            "a deleted backend falls back to the first AI backend"
        );
        assert_eq!(default_backend(&backends, Some("hook")).as_deref(), Some("ollama-local"));
        assert_eq!(default_backend(&[], None), None);
    }

    #[test]
    fn commands_are_listed_in_config_order_with_their_backend_names() {
        let commands = palette_commands(&cfg());
        assert_eq!(commands[0].name, "Fix grammar");
        assert_eq!(commands[0].hotkey, "Ctrl+Alt+J");
        assert_eq!(commands[0].backend_name, "Local Ollama");
    }

    #[test]
    fn picking_a_command_runs_it_exactly_as_configured() {
        let resolved =
            resolve_choice(&cfg(), &PaletteChoice::Command { command_id: "fix-grammar".into() }).unwrap();
        assert_eq!(resolved.command_id, "fix-grammar");
        assert_eq!(resolved.system_prompt, "Fix grammar and spelling.");
        assert_eq!(resolved.backend.id, "ollama-local");
    }

    #[test]
    fn a_free_text_instruction_becomes_a_temporary_command_with_that_prompt() {
        let resolved = resolve_choice(
            &cfg(),
            &PaletteChoice::Instruction { text: "  make it   friendlier ".into(), backend_id: "claude-sub".into() },
        )
        .unwrap();
        assert_eq!(resolved.command_id, PALETTE_INSTRUCTION_ID);
        assert_eq!(resolved.command_name, "make it friendlier");
        assert_eq!(resolved.system_prompt, "make it   friendlier");
        assert_eq!(resolved.backend.id, "claude-sub");
        assert_eq!(resolved.formatting, Formatting::Auto);
    }

    #[test]
    fn instructions_are_refused_for_empty_text_unknown_or_http_backends() {
        let empty = PaletteChoice::Instruction { text: "  ".into(), backend_id: "claude-sub".into() };
        assert!(resolve_choice(&cfg(), &empty).is_err());
        let http = PaletteChoice::Instruction { text: "x".into(), backend_id: "hook".into() };
        assert!(resolve_choice(&cfg(), &http).is_err());
        let gone = PaletteChoice::Command { command_id: "deleted".into() };
        assert!(resolve_choice(&cfg(), &gone).is_err());
    }

    #[test]
    fn long_instructions_get_a_short_history_name() {
        let long = "word ".repeat(40);
        let name = instruction_name(&long);
        assert_eq!(name.chars().count(), MAX_INSTRUCTION_NAME_CHARS);
        assert!(name.ends_with('…'));
    }

    #[test]
    fn the_choice_deserializes_from_the_palette_page_payload() {
        let command: PaletteChoice =
            serde_json::from_value(serde_json::json!({"kind": "command", "commandId": "c1"})).unwrap();
        assert_eq!(command, PaletteChoice::Command { command_id: "c1".into() });
        let instruction: PaletteChoice = serde_json::from_value(
            serde_json::json!({"kind": "instruction", "text": "shorter", "backendId": "api"}),
        )
        .unwrap();
        assert_eq!(
            instruction,
            PaletteChoice::Instruction { text: "shorter".into(), backend_id: "api".into() }
        );
    }

    #[test]
    fn the_remembered_backend_survives_a_restart_and_a_broken_file_is_ignored() {
        let dir = std::env::temp_dir().join(format!("cai-test-palette-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(load_remembered(&dir), None);
        save_remembered(&dir, "claude-sub");
        assert_eq!(load_remembered(&dir).as_deref(), Some("claude-sub"));
        fs::write(dir.join(STATE_FILE), "{broken").unwrap();
        assert_eq!(load_remembered(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_session_id_cannot_close_or_run_a_newer_palette() {
        let palette = Palette::default();
        *palette.inner.session.lock().unwrap() = Some(Session {
            id: 2,
            captured: Captured {
                clip: crate::osops::Clip { text: "x".into(), html: None },
                window: 9,
                captured_ms: 0,
            },
            anchor: Anchor {
                rect: overlay::Rect { left: 0, top: 0, right: 1, bottom: 1 },
                from_caret: false,
            },
            shown: true,
            focused: true,
            above: None,
        });
        assert!(palette.take(Some(1)).is_none(), "a late message from session 1 is ignored");
        assert!(palette.is_open());
        assert!(palette.take(Some(2)).is_some());
        assert!(palette.take(Some(2)).is_none(), "only one of close, run and blur can win");
    }
}
