mod backend;
mod commands;
mod config;
mod cycle;
mod osops;
mod sentinel;
mod status;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use serde::Serialize;
use tauri::{
    menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// A single hotkey's dispatch data: the command's id (needed only to
/// attribute a later OS-registration failure back to this command -- see
/// `register_hotkeys`), the display name (used in the log and tray status),
/// the system prompt to send, and the backend to send it to.
#[derive(Clone)]
struct CommandEntry {
    id: String,
    name: String,
    system_prompt: String,
    backend: Arc<dyn backend::Backend>,
}

/// A problem surfaced to the settings window: something in the config
/// didn't apply cleanly, but didn't prevent the rest of the config from
/// applying either.
///
/// `command_id` is `Some` whenever the problem is attributable to one
/// command, so the frontend can show it on that command's own row
/// (`CommandList.svelte`'s `problemsFor`) instead of matching against
/// `message`'s wording -- see `register_hotkeys`'s doc comment for why that
/// used to be impossible for OS-registration failures specifically. `None`
/// for a problem that isn't about any single command (e.g. the OS/input
/// layer failing to initialize, or a swallowed `unregister_all` failure).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub command_id: Option<String>,
    pub message: String,
}

/// Per-hotkey dispatch table plus the machinery a cycle needs to run.
///
/// `commands` is keyed by the parsed [`Shortcut`] struct rather than by a
/// string. See `build_command_map` for why: it avoids reconciling the
/// config's hotkey spelling against the plugin's own textual rendering of
/// the same shortcut.
///
/// `Clone`, with every mutable field individually `Arc`-wrapped rather than
/// the whole struct behind one outer `Arc` (see `state` in `run`'s `setup`,
/// which still keeps that outer `Arc` for the hotkey-handler closure).
/// Both exist for one reason: a second, independent `AppState` *value* is
/// handed to Tauri's own state container below (`app.manage`) so
/// `tauri::State<'_, AppState>` in `commands.rs`'s `#[tauri::command]`s and
/// `AppHandle::try_state::<AppState>()` (the settings-window close handler
/// below) can find it. Cloning `AppState` must produce a second *handle*
/// onto the same config/commands/busy/status -- not a second, independent
/// copy of them -- otherwise a save reaching this state via a
/// `tauri::command` and a read reaching it via the hotkey-handler closure
/// would silently drift apart.
#[derive(Clone)]
struct AppState {
    /// The in-memory source of truth for the running config. `apply_config`
    /// is the only writer, at startup and (later) on every settings-window
    /// save; it replaces this wholesale rather than mutating it in place,
    /// the same way it replaces `commands` below wholesale rather than
    /// patching it.
    config: Arc<Mutex<config::Config>>,
    commands: Arc<Mutex<HashMap<Shortcut, CommandEntry>>>,
    ops: Arc<dyn osops::OsOps>,
    busy: Arc<AtomicBool>,
    /// The last status line, mirrored onto the disabled "status" tray menu
    /// item below. This is the only console a release build has — see
    /// `set_status`.
    last_status: Arc<Mutex<String>>,
    /// Handle to the disabled tray menu item that always shows
    /// `last_status`. Kept as a live handle (not rebuilt) because
    /// `MenuItem::set_text` can update it in place from any thread — it
    /// hops to the main thread internally.
    status_item: MenuItem<tauri::Wry>,
    /// Directory holding `config.json`; `clipboard-ai.log` lives alongside
    /// it — see `status::append_log`.
    config_dir: PathBuf,
    /// Set once, here in `setup`, if `config::load_or_create` failed --
    /// never written again afterwards (a load failure only happens at
    /// startup), so unlike `config`/`commands` above this needs no `Mutex`
    /// of its own. `commands::get_config` clones it out to the settings
    /// window on every load, so a save-over-the-fallback (C2) is caught by
    /// the UI itself rather than relying on the user to notice an
    /// implausibly empty config.
    config_load_error: Option<commands::ConfigLoadError>,
}

/// Writes `text` to both halves of the status hook: the `last_status`
/// mutex, and — read back from that same mutex, not from a second copy of
/// the string threaded through separately — the tray's status menu item.
/// A tray-update failure must never take down a cycle that otherwise
/// succeeded, so `set_text`'s result is swallowed deliberately.
fn set_status(state: &AppState, text: String) {
    if let Ok(mut last) = state.last_status.lock() {
        *last = text;
    }
    if let Ok(last) = state.last_status.lock() {
        let _ = state.status_item.set_text(&*last);
    }
}

/// Resets the busy re-entrancy flag on drop, including when the spawned
/// cycle thread panics.
///
/// Mirrors `cycle::ClipboardGuard`: releasing the guard is a property of
/// scope, not a step an exit path has to remember. Without this, a panic
/// partway through `cycle::run` (or in the map lookup around it) would
/// leave `busy` stuck at `true` forever, silently disabling every hotkey
/// for the rest of the process's life.
struct BusyGuard<'a>(&'a AtomicBool);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Build the hotkey -> command-entry map from config.
///
/// A command whose hotkey will not parse, or whose backend will not build, is
/// skipped with a message rather than aborting the whole load — one bad
/// entry must not take the other commands down with it.
///
/// The map is keyed by the parsed `Shortcut`, not the raw config string.
/// `config::validate` dedups hotkeys case-insensitively but stores the raw
/// spelling (e.g. `"Ctrl+Alt+J"`), while the global-shortcut plugin hands
/// the handler back a `Shortcut` whose `Display` renders lowercase and
/// spells the modifier out in full (`"control+alt+j"`) — those two strings
/// never match. Parsing once here and reusing the identical `Shortcut`
/// value both for registration and as the map key sidesteps textual
/// normalization entirely: the plugin always hands the handler back the
/// exact value that was registered, and `Shortcut` derives `Eq`/`Hash`, so
/// lookup is structural rather than string-based.
fn build_command_map(cfg: &config::Config) -> (HashMap<Shortcut, CommandEntry>, Vec<Problem>) {
    let mut map = HashMap::new();
    let mut problems = Vec::new();
    for command in &cfg.commands {
        let Some(backend_cfg) = cfg.backend(&command.backend_id) else {
            problems.push(Problem {
                command_id: Some(command.id.clone()),
                message: format!("{}: unknown backend", command.name),
            });
            continue;
        };
        let shortcut = match command.hotkey.parse::<Shortcut>() {
            Ok(s) => s,
            Err(e) => {
                problems.push(Problem {
                    command_id: Some(command.id.clone()),
                    message: format!(
                        "{}: unparseable hotkey {:?} ({e})",
                        command.name, command.hotkey
                    ),
                });
                continue;
            }
        };
        match backend::build(backend_cfg) {
            Ok(backend) => {
                let entry = CommandEntry {
                    id: command.id.clone(),
                    name: command.name.clone(),
                    system_prompt: command.system_prompt.clone(),
                    backend,
                };
                // `config::validate` dedups hotkeys case-insensitively but
                // stores the raw spelling, so two entries that spell the
                // same modifier differently (e.g. "Ctrl+Alt+J" and
                // "Control+Alt+J") both pass validation even though they
                // parse to the identical `Shortcut`. An unchecked insert
                // would silently drop one command's binding with no signal
                // anywhere; report it as a problem instead.
                if let Some(previous) = map.insert(shortcut, entry) {
                    problems.push(Problem {
                        command_id: Some(command.id.clone()),
                        message: format!(
                            "{}: hotkey {:?} collides with \"{}\"'s hotkey after parsing ({shortcut}); \
                             only \"{}\" is registered",
                            command.name, command.hotkey, previous.name, previous.name
                        ),
                    });
                }
            }
            Err(e) => problems.push(Problem {
                command_id: Some(command.id.clone()),
                message: format!("{}: {e}", command.name),
            }),
        }
    }
    (map, problems)
}

#[cfg(test)]
mod build_command_map_tests {
    use super::*;
    use crate::config::{BackendConfig, CommandConfig, Config, KIND_CLAUDE_CLI};

    fn backend_cfg() -> BackendConfig {
        BackendConfig {
            id: "b".into(),
            name: "Backend".into(),
            kind: KIND_CLAUDE_CLI.into(),
            base_url: None,
            model: None,
            api_key: None,
            timeout_ms: 120_000,
        }
    }

    fn command(id: &str, name: &str, hotkey: &str) -> CommandConfig {
        CommandConfig {
            id: id.into(),
            name: name.into(),
            backend_id: "b".into(),
            hotkey: hotkey.into(),
            system_prompt: "x".into(),
        }
    }

    #[test]
    fn colliding_hotkeys_that_differ_only_in_modifier_spelling_are_reported_not_silently_dropped() {
        // Minor: config::validate dedups hotkeys case-insensitively but
        // stores the raw spelling, so "Ctrl+Alt+J" and "Control+Alt+J"
        // both pass validation even though `Shortcut::parse` normalizes
        // both to the identical value. Without an is_some() check on
        // insert, the second command's hotkey would silently replace the
        // first's in the map with no signal anywhere that it happened.
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            commands: vec![
                command("one", "First", "Ctrl+Alt+J"),
                command("two", "Second", "Control+Alt+J"),
            ],
        };
        let (map, problems) = build_command_map(&cfg);
        assert_eq!(
            map.len(),
            1,
            "both config entries parse to the same Shortcut, so only one hotkey can ever fire"
        );
        assert!(
            problems.iter().any(|p| p.message.contains("Second")),
            "the dropped command's collision must be reported as a problem, got: {problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.command_id.as_deref() == Some("two")),
            "the collision problem must carry the dropped command's id, got: {problems:?}"
        );
    }

    #[test]
    fn non_colliding_hotkeys_register_cleanly_with_no_problems() {
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            commands: vec![command("one", "First", "Ctrl+Alt+J"), command("two", "Second", "Ctrl+Alt+K")],
        };
        let (map, problems) = build_command_map(&cfg);
        assert_eq!(map.len(), 2);
        assert!(problems.is_empty(), "expected no problems, got: {problems:?}");
    }
}

/// Attempts to register every `(shortcut, command_id)` pair with the OS,
/// reporting a `Problem` attributed to that command's id for each one the OS
/// refuses (e.g. already owned by another application).
///
/// Thin wrapper around `register_hotkeys_with` -- the actual OS call is the
/// one part of this that needs a live `AppHandle` and so cannot run in a
/// unit test; `register_hotkeys_with` is the attribution logic pulled out
/// so it can be exercised with a fake, always-failing registration function
/// instead. See `register_hotkeys_tests` below.
fn register_hotkeys(app: &tauri::AppHandle, shortcuts: &[(Shortcut, String)]) -> Vec<Problem> {
    register_hotkeys_with(shortcuts, |shortcut| app.global_shortcut().register(*shortcut))
}

fn register_hotkeys_with<F, E>(shortcuts: &[(Shortcut, String)], mut register: F) -> Vec<Problem>
where
    F: FnMut(&Shortcut) -> Result<(), E>,
    E: std::fmt::Display,
{
    let mut problems = Vec::new();
    for (shortcut, command_id) in shortcuts {
        if let Err(e) = register(shortcut) {
            problems.push(Problem {
                command_id: Some(command_id.clone()),
                message: format!("{shortcut}: already in use ({e})"),
            });
        }
    }
    problems
}

#[cfg(test)]
mod register_hotkeys_tests {
    use super::*;

    // The headline fix: `register_hotkeys` used to take a bare `&[Shortcut]`
    // and format problems with `Shortcut`'s own `Display` impl, which
    // renders lowercase and in a fixed modifier order that never matches
    // the config's spelling -- so `CommandList.svelte`'s row-matching could
    // never succeed for this problem type. Attributing by `command_id`
    // instead sidesteps string matching entirely.
    #[test]
    fn a_hotkey_that_fails_to_register_is_attributed_to_its_command_id() {
        let shortcut: Shortcut = "Ctrl+Alt+J".parse().unwrap();
        let shortcuts = vec![(shortcut, "cmd-1".to_string())];

        let problems = register_hotkeys_with(&shortcuts, |_| Err("already registered by another app"));

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].command_id.as_deref(), Some("cmd-1"));
        assert!(
            problems[0].message.contains("already in use"),
            "got: {}",
            problems[0].message
        );
    }

    #[test]
    fn a_hotkey_that_registers_successfully_produces_no_problem() {
        let shortcut: Shortcut = "Ctrl+Alt+K".parse().unwrap();
        let shortcuts = vec![(shortcut, "cmd-2".to_string())];

        let problems = register_hotkeys_with::<_, &str>(&shortcuts, |_| Ok(()));

        assert!(problems.is_empty(), "expected no problems, got: {problems:?}");
    }

    #[test]
    fn only_the_failing_shortcut_in_a_batch_is_attributed() {
        let ok: Shortcut = "Ctrl+Alt+K".parse().unwrap();
        let bad: Shortcut = "Ctrl+Alt+J".parse().unwrap();
        let shortcuts = vec![(ok, "cmd-ok".to_string()), (bad, "cmd-bad".to_string())];

        let problems =
            register_hotkeys_with(&shortcuts, |s| if *s == bad { Err("in use") } else { Ok(()) });

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].command_id.as_deref(), Some("cmd-bad"));
    }
}

/// Unregisters every hotkey, rebuilds the command map from `config`, swaps
/// both `state.commands` and `state.config` to the new values, then
/// re-registers with the OS. Returns per-command problems that did not
/// prevent the apply (an unparseable hotkey, an unknown backend, a hotkey
/// collision -- see `build_command_map` -- or a hotkey the OS refused to
/// register, e.g. one already owned by another application), plus an
/// unattributed one (`command_id: None`) if the initial unregister itself
/// failed.
///
/// Called once at startup below, and later by a settings-window save on
/// every save -- that's why `config` arrives as an owned, already-loaded
/// value rather than a path this function reads for itself. v1 registered
/// hotkeys once in `setup` and never again, which made a settings UI a
/// lie; this is the fix.
///
/// A cycle already running is unaffected by this call. The hotkey handler
/// below clones the matching `CommandEntry` -- and, inside it, the
/// `Arc<dyn Backend>` -- out of `state.commands` *before* spawning the
/// cycle thread; `cycle::run` then only ever touches that owned clone, and
/// never looks at the map again. So swapping `state.commands` here, even
/// while a cycle is mid-flight, cannot reach it: a save during a
/// 16-second cold-model cycle takes effect starting with the *next* press,
/// not the one already running.
fn apply_config(app: &tauri::AppHandle, state: &AppState, config: config::Config) -> Vec<Problem> {
    let mut problems = Vec::new();
    // `suspend_hotkeys` (commands.rs) propagates this same call's Result;
    // this one used to discard it. The plugin clears its own bookkeeping
    // via `mem::take` *before* the OS-level unregister loop, and that loop
    // aborts on the first OS-level failure -- so a partial failure here
    // leaves stale OS registrations that `register_hotkeys` below would
    // then see as ordinary "already in use" collisions, with nothing
    // distinguishing them from a genuinely external app holding the key.
    // Surfacing it here at least tells whoever reads `problems` that the
    // unregister step itself was the one that went wrong.
    if let Err(e) = app.global_shortcut().unregister_all() {
        problems.push(Problem {
            command_id: None,
            message: format!(
                "failed to clear previous hotkeys ({e}); a hotkey reported as \"already in use\" \
                 below may actually be this app's own stale registration"
            ),
        });
    }

    let (map, build_problems) = build_command_map(&config);
    problems.extend(build_problems);
    let shortcuts: Vec<(Shortcut, String)> = map.iter().map(|(s, e)| (*s, e.id.clone())).collect();

    if let Ok(mut guard) = state.commands.lock() {
        *guard = map;
    }
    if let Ok(mut guard) = state.config.lock() {
        *guard = config;
    }

    problems.extend(register_hotkeys(app, &shortcuts));
    problems
}

/// Shows, restores and focuses the single `settings` webview window declared
/// in `tauri.conf.json`. Never creates one -- `get_webview_window` only ever
/// looks up the existing window by its label, so no caller of this function
/// (the tray menu item, the tray left click) can cause a second settings
/// window to come into existence.
fn show_settings(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Handles a failed `config::load_or_create` at startup: logs it (same
/// `STARTUP ...` style as every other startup line), preserves the original
/// file so it survives the empty fallback below, and returns both that
/// fallback and the details the settings window needs so it never mistakes
/// the fallback for the user's real, saved configuration (C2).
///
/// Pulled out of `setup` as a free function -- rather than inlined where
/// it's called, the only call site -- purely so it's unit-testable without
/// standing up a Tauri `AppHandle`; `setup`'s closure itself cannot be.
///
/// `path` may or may not still exist: `load_or_create` fails before the
/// file exists at all (its own directory couldn't be created, or the
/// first-run default write failed) just as easily as it fails on a file
/// that does exist but won't parse. Only the latter has anything to
/// preserve, hence the `path.exists()` check -- attempting to "preserve" a
/// path with nothing there would just fail the rename for an unrelated
/// reason and log a confusing message.
fn handle_config_load_failure(
    path: &Path,
    config_dir: &Path,
    e: config::ConfigError,
) -> (config::Config, commands::ConfigLoadError) {
    let line = format!("STARTUP config-load-failed reason={e}");
    eprintln!("{line}");
    status::append_log(config_dir, &line);

    let preserved_path = if path.exists() {
        match config::preserve_unreadable(path) {
            Ok(preserved) => {
                let line = format!(
                    "STARTUP config-preserved original={} preserved-as={}",
                    path.display(),
                    preserved.display()
                );
                eprintln!("{line}");
                status::append_log(config_dir, &line);
                Some(preserved.to_string_lossy().into_owned())
            }
            Err(preserve_err) => {
                // Never panic, never block startup over this -- a failed
                // preserve just means the user recovers less than they
                // otherwise would have; it must not mean the app never
                // reaches a tray icon.
                let line = format!("STARTUP config-preserve-failed reason={preserve_err}");
                eprintln!("{line}");
                status::append_log(config_dir, &line);
                None
            }
        }
    } else {
        None
    };

    (
        config::Config { version: 1, backends: Vec::new(), commands: Vec::new() },
        commands::ConfigLoadError { reason: e.to_string(), preserved_path },
    )
}

#[cfg(test)]
mod config_load_failure_tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cai-test-load-failure-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn preserves_an_existing_unparseable_file_and_reports_where_it_went() {
        let dir = temp_dir("existing");
        let path = dir.join("config.json");
        std::fs::write(&path, "{not valid json").unwrap();
        let err = config::parse("{not valid json").unwrap_err();

        let (fallback, load_error) = handle_config_load_failure(&path, &dir, err);

        assert_eq!(fallback.backends.len(), 0);
        assert_eq!(fallback.commands.len(), 0);
        assert!(!path.exists(), "the unparseable file must be moved aside, not left in place");
        let preserved = load_error
            .preserved_path
            .as_ref()
            .expect("an existing file must be preserved and its path reported");
        assert!(std::path::Path::new(preserved).exists());
        assert!(
            load_error.reason.contains("valid JSON") || !load_error.reason.is_empty(),
            "reason should describe the parse failure, got: {}",
            load_error.reason
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_no_preserved_path_when_there_was_nothing_to_preserve() {
        // A fresh install where the first-run write itself failed: nothing
        // ever landed at `path`, so there is nothing to move aside.
        let dir = temp_dir("nothing-there");
        let path = dir.join("config.json");
        let err = config::parse("{not valid json").unwrap_err();

        let (fallback, load_error) = handle_config_load_failure(&path, &dir, err);

        assert_eq!(fallback.backends.len(), 0);
        assert!(
            load_error.preserved_path.is_none(),
            "nothing existed at `path`, so there was nothing to preserve"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();

            let path = config::config_path(&handle);
            // The config directory doubles as the log directory. In every
            // realistic failure path below, `config::load_or_create` has
            // already created this directory (it does so before it ever
            // tries to parse anything) or the file already existed inside
            // it — so logging into it even from the config-error path
            // below is safe, not a chicken-and-egg problem.
            let config_dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));

            // I5: a config-load failure is otherwise the most silent
            // failure mode there is. `setup` returning `Err` makes
            // `.run()` below `.expect()`-panic, and `windows_subsystem =
            // "windows"` means a release build has no console to show that
            // panic message on -- the user double-clicks the exe and
            // *nothing happens*. Hand-editing config.json is the only
            // configuration mechanism in v1, so a trailing comma is a
            // likely outcome, not an exotic one. Do not propagate `?` here:
            // fall through with an empty config (no backends, no commands --
            // `apply_config` below then registers nothing rather than
            // guessing) so the app still reaches a tray icon, with the
            // failure surfaced in the log and the tray status text.
            let mut config_load_failed = false;
            let mut config_load_error: Option<commands::ConfigLoadError> = None;
            let cfg = match config::load_or_create(&path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    config_load_failed = true;
                    // C2: preserve the unreadable file (if any) and carry
                    // its whereabouts through to the settings window --
                    // see `handle_config_load_failure`.
                    let (fallback, load_error) = handle_config_load_failure(&path, &config_dir, e);
                    config_load_error = Some(load_error);
                    fallback
                }
            };

            let mut problems: Vec<Problem> = Vec::new();

            // Same treatment for a failed OS layer (no clipboard or input
            // subsystem available, e.g. a non-interactive session): fall
            // back to `NullOps` rather than take `?` and kill the app
            // before it ever shows a tray icon. Every cycle run against
            // `NullOps` reports `foreground_window() == 0`, which the I2
            // guard rejects immediately, before any clipboard mutation or
            // synthetic input -- so hotkeys harmlessly do nothing rather
            // than the process not existing at all.
            let ops: Arc<dyn osops::OsOps> = match osops::WindowsOps::new() {
                Ok(ops) => Arc::new(ops),
                Err(e) => {
                    let line = format!("STARTUP osops-init-failed reason={e}");
                    eprintln!("{line}");
                    status::append_log(&config_dir, &line);
                    problems.push(Problem {
                        command_id: None,
                        message: format!(
                            "input/clipboard system failed to initialize ({e}); hotkeys will do \
                             nothing until the app is restarted"
                        ),
                    });
                    Arc::new(osops::NullOps::new())
                }
            };

            let status_item =
                MenuItem::with_id(app, "status", "starting…", false, None::<&str>)?;

            // Built empty: `apply_config` below is the sole writer of both
            // `commands` and `config`, at startup here and (later) on every
            // settings-window save.
            let state = Arc::new(AppState {
                config: Arc::new(Mutex::new(config::Config {
                    version: 1,
                    backends: Vec::new(),
                    commands: Vec::new(),
                })),
                commands: Arc::new(Mutex::new(HashMap::new())),
                ops,
                busy: Arc::new(AtomicBool::new(false)),
                last_status: Arc::new(Mutex::new("starting…".into())),
                status_item: status_item.clone(),
                config_dir: config_dir.clone(),
                config_load_error,
            });

            // Hand Tauri's own state container a second *handle* onto this
            // exact AppState -- not a second, independent one; see the
            // struct's doc comment. Without this, `tauri::State<'_,
            // AppState>` in commands.rs and `try_state::<AppState>()` in the
            // settings-window close handler below would both silently find
            // nothing managed, no matter how the rest of this function is
            // wired.
            app.manage((*state).clone());

            let handler_state = state.clone();
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |_app, shortcut, event| {
                        if event.state() != ShortcutState::Released {
                            return;
                        }
                        let key = *shortcut;
                        let state = handler_state.clone();

                        if state.busy.swap(true, Ordering::SeqCst) {
                            let line = "busy; hotkey ignored".to_string();
                            println!("{line}");
                            status::append_log(&state.config_dir, &line);
                            return;
                        }

                        // The cycle takes seconds. Running it here would stall
                        // the shortcut listener for its whole duration.
                        std::thread::spawn(move || {
                            let _busy_guard = BusyGuard(&state.busy);
                            let entry = state
                                .commands
                                .lock()
                                .ok()
                                .and_then(|m| m.get(&key).cloned());
                            if let Some(CommandEntry { name, system_prompt, backend, .. }) = entry {
                                let outcome = cycle::run(
                                    backend.as_ref(),
                                    &system_prompt,
                                    state.ops.as_ref(),
                                );
                                let line = status::log_line(&outcome, &name, backend.name());
                                println!("{line}");
                                status::append_log(&state.config_dir, &line);

                                set_status(&state, status::summary(&outcome, &name));
                            }
                        });
                    })
                    .build(),
            )?;

            let backends_configured = cfg.backends.len();
            let commands_configured = cfg.commands.len();
            problems.extend(apply_config(&handle, &state, cfg));
            let hotkeys_attempted = state.commands.lock().map(|m| m.len()).unwrap_or(0);

            // One disabled, informational entry per successfully-registered
            // command, built from `state.commands` rather than all of
            // `cfg.commands` -- a command whose hotkey didn't parse, whose
            // backend is unknown, or whose hotkey collided with another
            // (see build_command_map) is not in this map, and the tray must
            // not advertise a hotkey that does nothing.
            let mut command_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
            if let Ok(guard) = state.commands.lock() {
                for (i, (shortcut, entry)) in guard.iter().enumerate() {
                    let label = format!("{} \u{2014} {shortcut}", entry.name);
                    let id = format!("command-{i}");
                    command_items.push(MenuItem::with_id(app, id, label, false, None::<&str>)?);
                }
            }

            // Hotkey-registration failures (a shortcut already owned by
            // another application) and config problems (unknown backend,
            // unparseable hotkey) are the highest-value thing to surface:
            // without this, the corresponding hotkey just does nothing,
            // forever, with no signal anywhere. Every one of them reaches
            // both the log file and the tray status, not just per-cycle
            // outcomes.
            status::append_log(
                &config_dir,
                &format!(
                    "STARTUP backends={} commands-configured={} hotkeys-attempted={} problems={}",
                    backends_configured,
                    commands_configured,
                    hotkeys_attempted,
                    problems.len()
                ),
            );
            for problem in &problems {
                let line = format!("STARTUP problem: {}", problem.message);
                eprintln!("{line}");
                status::append_log(&config_dir, &line);
            }
            let initial_status = if config_load_failed {
                "config error \u{2014} see log".to_string()
            } else if !problems.is_empty() {
                format!("{} startup problem(s) \u{2014} see log", problems.len())
            } else {
                "ready".to_string()
            };
            set_status(&state, initial_status);

            let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let open_config =
                MenuItem::with_id(app, "open-config", "Open config file", true, None::<&str>)?;
            let reload = MenuItem::with_id(app, "reload", "Reload config", true, None::<&str>)?;
            let show_log = MenuItem::with_id(app, "show-log", "Show log", true, None::<&str>)?;
            let sep_after_status = PredefinedMenuItem::separator(app)?;
            let sep_after_settings = PredefinedMenuItem::separator(app)?;
            let sep_after_commands = PredefinedMenuItem::separator(app)?;
            let sep_before_quit = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            // `Settings` is the first actionable entry, right after the
            // (disabled) status line -- before the per-hotkey list, which
            // can be empty or long depending on config.
            let mut items: Vec<&dyn IsMenuItem<_>> =
                vec![&status_item, &sep_after_status, &settings, &sep_after_settings];
            for item in &command_items {
                items.push(item);
            }
            if !command_items.is_empty() {
                items.push(&sep_after_commands);
            }
            items.push(&open_config);
            items.push(&reload);
            items.push(&show_log);
            items.push(&sep_before_quit);
            items.push(&quit);
            let menu = Menu::with_items(app, &items)?;

            let menu_path = path.clone();
            let log_path = config_dir.join("clipboard-ai.log");
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Clipboard AI")
                .menu(&menu)
                // The tray-icon crate shows its own native context menu on a
                // left click by default (true), independent of the
                // on_tray_icon_event handler below. Left click on Windows is
                // conventionally "open the app", not "open the app AND also
                // pop the menu" -- without turning this off, both would
                // happen on every left click.
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "settings" => show_settings(app),
                    "open-config" => {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", &menu_path.to_string_lossy()])
                            .spawn();
                    }
                    "show-log" => {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", &log_path.to_string_lossy()])
                            .spawn();
                    }
                    "reload" => {
                        // v1 narrowing, not yet lifted here: `apply_config`
                        // above is the live-reregistration mechanism (safe
                        // to call while a cycle is in flight -- see its doc
                        // comment), built so a settings-window save can take
                        // effect immediately. This tray button just isn't
                        // wired to it yet -- rereading config.json from disk
                        // and calling apply_config is the settings-window
                        // save flow's job, not this one's. For now, restart
                        // the app to pick up config changes.
                        println!("reload requested — restart the app to apply config changes");
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        show_settings(tray.app_handle());
                    }
                })
                .build(app)?;

            // Hide, don't close: the settings window is a panel the user
            // summons from the tray, not a document with its own lifecycle.
            // `prevent_close` stops the close from ever happening at all, so
            // this is not "close, but immediately reopen hidden" -- the
            // window is simply never destroyed, and the app (tray-resident,
            // no other windows) never sees a last-window-closed exit either.
            //
            // This handler is the single choke point for every route that
            // can request a close on this window -- the X button, Alt+F4,
            // the system menu's Close, and (once the frontend exists) a
            // programmatic `window.close()` -- all of them surface as the
            // same `CloseRequested` event; there is no route that hides this
            // window other than through here.
            //
            // `prevent_close` always runs, unconditionally -- native close
            // must never proceed on its own. What happens next is
            // `commands::handle_close_request`'s call: hide immediately if
            // there is nothing unsaved (which is also the path a frontend
            // that never mounted takes -- see that function's doc comment),
            // otherwise ask the frontend first and hide anyway if it never
            // answers. Either way that function is also where the
            // Task-4 defensive config re-apply now lives (hotkey recording,
            // Task 8, suspends global shortcuts while it listens; this is
            // the one exit path the frontend cannot reliably intercept to
            // resume them itself), run on every route that actually hides
            // the window, never on a route that doesn't.
            if let Some(window) = app.get_webview_window("settings") {
                let handle = handle.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        // `AppState` is `.manage()`d before any window event
                        // can fire in practice, so `None` here should be
                        // unreachable -- but `handle_close_request` treats it
                        // as optional precisely so that a failure to look it
                        // up still cannot prevent the window from hiding.
                        let state = handle.try_state::<AppState>().map(|s| (*s).clone());
                        commands::handle_close_request(handle.clone(), state);
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::suspend_hotkeys,
            commands::resume_hotkeys,
            commands::set_dirty,
            commands::ack_close_prompt,
            commands::force_close_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
