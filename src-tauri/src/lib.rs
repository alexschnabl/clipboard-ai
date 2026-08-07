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
    time::{Duration, Instant},
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
    /// The hotkey exactly as the config spells it. Kept alongside the parsed
    /// `Shortcut` because the two do not render the same: `Shortcut`'s
    /// `Display` goes through `global-hotkey`'s `HotKey::into_string()`,
    /// which lowercases the modifiers, emits them in its own fixed order and
    /// prints the raw key code -- so `"Ctrl+Alt+J"` comes back out as
    /// `"control+alt+KeyJ"`. Anything the user reads (the tray menu, a
    /// problem message) must use this field, never the parsed form, or it
    /// shows a spelling they never typed.
    hotkey: String,
    system_prompt: String,
    backend: Arc<dyn backend::Backend>,
}

/// One command's hotkey, ready to hand to the OS. Carries the command's
/// identity so a registration failure can be attributed to it without
/// matching on message text.
struct Registration {
    shortcut: Shortcut,
    command_id: String,
    command_name: String,
    hotkey: String,
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
    /// R10: every `Problem` produced during startup (a failed OS-layer
    /// init, an unparseable hotkey, a hotkey the OS refused to register --
    /// see `run`'s `setup`) -- distinct from `config_load_error` because
    /// these don't prevent a config from loading, they just mean part of it
    /// didn't take effect. `Mutex`-wrapped and written once, shortly after
    /// `setup`'s own `apply_config` call finishes, rather than being a
    /// plain field set at construction time like `config_load_error` --
    /// unlike a load failure, these problems are not known until *after*
    /// `apply_config` has run, which is after this struct already exists
    /// and has been handed to `app.manage`. `commands::get_config` clones
    /// it out on every settings-window load; without this a hotkey that
    /// failed to register at launch was invisible in the UI until an
    /// unrelated Save happened to call `apply_config` again.
    startup_problems: Arc<Mutex<Vec<Problem>>>,
}

/// R4: how long tray Quit waits for an in-flight cycle to reach its own
/// `ClipboardGuard`/chord-release cleanup before tearing the process down
/// regardless. `app.exit(0)` does not unwind, so anything relying on `Drop`
/// to run (restoring the user's clipboard, releasing a synthetic Ctrl held
/// down mid-chord) is simply skipped if the process dies while a cycle is
/// still using it. A couple of seconds covers a normal cycle (capture,
/// backend call, paste) comfortably without making Quit itself feel hung;
/// it does *not* cover a slow backend call still inside its own multi-second
/// timeout -- that case still loses the clipboard, and the wait below logs
/// rather than pretending otherwise.
const QUIT_CYCLE_WAIT: Duration = Duration::from_secs(2);

/// R8: recovers a poisoned mutex guard instead of propagating the poison,
/// and logs that it happened. Mirrors `osops.rs`'s `chord`/`read_clipboard`
/// (`.unwrap_or_else(|e| e.into_inner())`) and the same reasoning: a panic
/// elsewhere while this lock was held does not corrupt the data it guards,
/// so the default `?`/`if let Ok(..)` behaviour -- silently treating every
/// *later* access to this exact state as "do nothing" forever -- is worse
/// than recovering it. This matters most for `state.commands`: swallowed
/// via `.ok()`, a poisoned lock there would make every hotkey press quietly
/// no-op for the rest of the process's life, with nothing anywhere to say
/// why. `config_dir` is `Option` so a call site with no `AppState`, and so
/// no log directory, in scope can still recover a poisoned lock -- it just
/// loses the `status::append_log` half and falls back to `eprintln!` alone.
fn recover_lock<'a, T>(
    result: Result<
        std::sync::MutexGuard<'a, T>,
        std::sync::PoisonError<std::sync::MutexGuard<'a, T>>,
    >,
    what: &str,
    config_dir: Option<&Path>,
) -> std::sync::MutexGuard<'a, T> {
    result.unwrap_or_else(|poisoned| {
        let line = format!("{what} mutex was poisoned by an earlier panic; recovering");
        eprintln!("{line}");
        if let Some(dir) = config_dir {
            status::append_log(dir, &line);
        }
        poisoned.into_inner()
    })
}

/// Best-effort extraction of a message from a `catch_unwind` payload. Most
/// panics in this codebase carry a `&str` (a `panic!("literal")`) or a
/// `String` (a `panic!("{formatted}")`); anything else (a custom payload
/// from a dependency) has no guaranteed shape at all, so this falls back to
/// a fixed string rather than guessing further.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// Writes `text` to both halves of the status hook: the `last_status`
/// mutex, and — read back from that same mutex, not from a second copy of
/// the string threaded through separately — the tray's status menu item.
/// A tray-update failure must never take down a cycle that otherwise
/// succeeded, so `set_text`'s result is swallowed deliberately.
fn set_status(state: &AppState, text: String) {
    {
        let mut last = recover_lock(state.last_status.lock(), "last_status", Some(&state.config_dir));
        *last = text;
    }
    let last = recover_lock(state.last_status.lock(), "last_status", Some(&state.config_dir));
    let _ = state.status_item.set_text(&*last);
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
                    hotkey: command.hotkey.clone(),
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

/// Attempts to register every `Registration` with the OS, reporting a
/// `Problem` attributed to that command's id for each one the OS refuses
/// (e.g. already owned by another application).
///
/// Thin wrapper around `register_hotkeys_with` -- the actual OS call is the
/// one part of this that needs a live `AppHandle` and so cannot run in a
/// unit test; `register_hotkeys_with` is the attribution logic pulled out
/// so it can be exercised with a fake, always-failing registration function
/// instead. See `register_hotkeys_tests` below.
fn register_hotkeys(app: &tauri::AppHandle, registrations: &[Registration]) -> Vec<Problem> {
    register_hotkeys_with(registrations, |shortcut| {
        app.global_shortcut().register(*shortcut)
    })
}

fn register_hotkeys_with<F, E>(registrations: &[Registration], mut register: F) -> Vec<Problem>
where
    F: FnMut(&Shortcut) -> Result<(), E>,
    E: std::fmt::Display,
{
    let mut problems = Vec::new();
    for reg in registrations {
        if let Err(e) = register(&reg.shortcut) {
            problems.push(Problem {
                command_id: Some(reg.command_id.clone()),
                // Leads with the command name and the config's own spelling,
                // matching every other problem message -- and specifically
                // not `Shortcut`'s `Display`, which would print a spelling
                // the user never typed (see `CommandEntry::hotkey`).
                message: format!(
                    "{}: hotkey {:?} is already in use ({e})",
                    reg.command_name, reg.hotkey
                ),
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
    fn registration(hotkey: &str, command_id: &str, command_name: &str) -> Registration {
        Registration {
            shortcut: hotkey.parse().unwrap(),
            command_id: command_id.to_string(),
            command_name: command_name.to_string(),
            hotkey: hotkey.to_string(),
        }
    }

    #[test]
    fn a_hotkey_that_fails_to_register_is_attributed_to_its_command_id() {
        let registrations = vec![registration("Ctrl+Alt+J", "cmd-1", "Fix grammar")];

        let problems =
            register_hotkeys_with(&registrations, |_| Err("already registered by another app"));

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].command_id.as_deref(), Some("cmd-1"));
        assert!(
            problems[0].message.contains("already in use"),
            "got: {}",
            problems[0].message
        );
    }

    // The message the user actually reads must use the config's spelling.
    // `Shortcut`'s `Display` would render this same hotkey as
    // "control+alt+KeyJ" -- lowercased, reordered, raw key code -- which is
    // a spelling that appears nowhere in their config.
    #[test]
    fn the_problem_message_uses_the_config_spelling_and_names_the_command() {
        let registrations = vec![registration("Ctrl+Alt+J", "cmd-1", "Fix grammar")];

        let problems = register_hotkeys_with(&registrations, |_| Err("in use"));

        let message = &problems[0].message;
        assert!(message.contains("Fix grammar"), "must name the command, got: {message}");
        assert!(message.contains("Ctrl+Alt+J"), "must use the config spelling, got: {message}");
        assert!(
            !message.contains("control+alt+KeyJ"),
            "must not leak the parsed Shortcut's Display form, got: {message}"
        );
    }

    #[test]
    fn a_hotkey_that_registers_successfully_produces_no_problem() {
        let registrations = vec![registration("Ctrl+Alt+K", "cmd-2", "Summarise")];

        let problems = register_hotkeys_with::<_, &str>(&registrations, |_| Ok(()));

        assert!(problems.is_empty(), "expected no problems, got: {problems:?}");
    }

    #[test]
    fn only_the_failing_shortcut_in_a_batch_is_attributed() {
        let bad: Shortcut = "Ctrl+Alt+J".parse().unwrap();
        let registrations = vec![
            registration("Ctrl+Alt+K", "cmd-ok", "Summarise"),
            registration("Ctrl+Alt+J", "cmd-bad", "Fix grammar"),
        ];

        let problems = register_hotkeys_with(&registrations, |s| {
            if *s == bad {
                Err("in use")
            } else {
                Ok(())
            }
        });

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
/// `Arc<dyn Backend>` -- out of `state.commands` *inside* the spawned cycle
/// thread, once, right at its start and before `cycle::run` itself is ever
/// invoked (R15: this used to say "before spawning the cycle thread",
/// which described a different mechanism than the code -- the clone always
/// happened on the spawned thread, not on the handler thread beforehand;
/// the substance below was correct even though that mechanism claim
/// wasn't). `cycle::run` then only ever touches that owned clone, and never
/// looks at the map again. So swapping `state.commands` here, even while a
/// cycle is mid-flight, cannot reach it: a save during a 16-second
/// cold-model cycle takes effect starting with the *next* press, not the
/// one already running.
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
    let registrations: Vec<Registration> = map
        .iter()
        .map(|(s, e)| Registration {
            shortcut: *s,
            command_id: e.id.clone(),
            command_name: e.name.clone(),
            hotkey: e.hotkey.clone(),
        })
        .collect();

    *recover_lock(state.commands.lock(), "commands", Some(&state.config_dir)) = map;
    *recover_lock(state.config.lock(), "config", Some(&state.config_dir)) = config;

    problems.extend(register_hotkeys(app, &registrations));
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
                startup_problems: Arc::new(Mutex::new(Vec::new())),
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
                            // R11: this used to only reach the log file --
                            // invisible in a release build with no console
                            // (see set_status's doc comment) -- so a dropped
                            // press looked like nothing happened at all
                            // rather than "busy, try again".
                            set_status(&state, line);
                            return;
                        }

                        // The cycle takes seconds. Running it here would stall
                        // the shortcut listener for its whole duration.
                        std::thread::spawn(move || {
                            let _busy_guard = BusyGuard(&state.busy);
                            // R9: a panic anywhere in here used to simply
                            // vanish -- a spawned thread's panic prints to
                            // stderr (invisible in a release build with no
                            // console) and unwinds only this thread; nothing
                            // caught it, logged it, or told the tray. This
                            // cannot "handle" the panic in any deeper sense
                            // -- something already went wrong that the code
                            // below didn't anticipate -- but it at least
                            // gets one line into the log and the tray status
                            // instead of a silent, permanently-stuck-looking
                            // hotkey.
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                // R8: `.ok()` on a poisoned `commands` lock
                                // used to mean every hotkey press after an
                                // unrelated panic silently did nothing,
                                // forever, with no signal anywhere --
                                // recover instead (see `recover_lock`'s doc
                                // comment: the map itself is never left
                                // mid-mutation by a panic elsewhere).
                                let entry = recover_lock(
                                    state.commands.lock(),
                                    "commands",
                                    Some(&state.config_dir),
                                )
                                .get(&key)
                                .cloned();
                                if let Some(CommandEntry { name, system_prompt, backend, .. }) = entry
                                {
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
                            }));
                            if let Err(payload) = result {
                                let message = panic_message(&payload);
                                let line = format!("PANIC hotkey-thread panicked: {message}");
                                eprintln!("{line}");
                                status::append_log(&state.config_dir, &line);
                                set_status(&state, "internal error \u{2014} see log".to_string());
                            }
                        });
                    })
                    .build(),
            )?;

            let backends_configured = cfg.backends.len();
            let commands_configured = cfg.commands.len();
            problems.extend(apply_config(&handle, &state, cfg));
            // R10: record the full startup problem list where
            // `commands::get_config` can hand it to the settings window on
            // every load -- not just log it, which is where the rest of
            // this block already sends it. Cloned rather than moved: the
            // log lines and `initial_status` below still need `problems`.
            *recover_lock(state.startup_problems.lock(), "startup_problems", Some(&config_dir)) =
                problems.clone();
            let hotkeys_attempted =
                recover_lock(state.commands.lock(), "commands", Some(&config_dir)).len();

            // One disabled, informational entry per successfully-registered
            // command, built from `state.commands` rather than all of
            // `cfg.commands` -- a command whose hotkey didn't parse, whose
            // backend is unknown, or whose hotkey collided with another
            // (see build_command_map) is not in this map, and the tray must
            // not advertise a hotkey that does nothing.
            let mut command_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
            {
                let guard = recover_lock(state.commands.lock(), "commands", Some(&config_dir));
                for (i, (_shortcut, entry)) in guard.iter().enumerate() {
                    // `entry.hotkey`, not the parsed `Shortcut`: the latter's
                    // `Display` renders "Ctrl+Alt+J" as "control+alt+KeyJ",
                    // and the tray menu is the most visible place in the app
                    // to show a user a spelling they never typed.
                    let label = format!("{} \u{2014} {}", entry.name, entry.hotkey);
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
            let menu_state = state.clone();
            let mut tray_builder = TrayIconBuilder::new().tooltip("Clipboard AI").menu(&menu);
            // R17: `.unwrap()` on a missing bundled icon used to panic
            // during `setup`, and `windows_subsystem = "windows"` means a
            // release build has no console to show that panic on -- the app
            // would just never reach a tray icon, with nothing to tell the
            // user why. `.icon(...)` is optional on the builder; skipping it
            // still yields a working tray (menu, click handling, status
            // text) with no glyph rather than no tray at all.
            match app.default_window_icon() {
                Some(icon) => tray_builder = tray_builder.icon(icon.clone()),
                None => {
                    let line = "STARTUP tray-icon-missing; continuing without one".to_string();
                    eprintln!("{line}");
                    status::append_log(&config_dir, &line);
                }
            }
            tray_builder
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
                        // R6: this used to be a `println!` stub that did not
                        // reload anything and, being `println!` rather than
                        // going through `status::append_log`, was invisible
                        // in a release build entirely. Menu events run on
                        // the main thread (same as window events -- see the
                        // `CloseRequested` handler below and R3's doc
                        // comment on `hide_and_reapply`), so it is safe to
                        // call `apply_config` here directly, the same way
                        // `setup` and the settings-window Save path do.
                        let line = match config::load_or_create(&menu_path) {
                            Ok(cfg) => {
                                let problems = apply_config(app, &menu_state, cfg);
                                for problem in &problems {
                                    let line = format!("RELOAD problem: {}", problem.message);
                                    eprintln!("{line}");
                                    status::append_log(&menu_state.config_dir, &line);
                                }
                                if problems.is_empty() {
                                    "reloaded config".to_string()
                                } else {
                                    format!("reloaded with {} problem(s) \u{2014} see log", problems.len())
                                }
                            }
                            Err(e) => {
                                format!("reload failed: {e} \u{2014} see log")
                            }
                        };
                        println!("RELOAD {line}");
                        status::append_log(&menu_state.config_dir, &format!("RELOAD {line}"));
                        set_status(&menu_state, line);
                    }
                    "quit" => {
                        // R4: `app.exit(0)` terminates the process without
                        // unwinding, so `cycle::ClipboardGuard`'s `Drop`
                        // never runs if a cycle is still in flight on the
                        // spawned hotkey thread -- the user's original
                        // clipboard is gone for good, and a kill mid-`chord`
                        // (osops.rs) can strand a synthetic Ctrl held down
                        // OS-wide. Give it a bounded window to reach its own
                        // restore path first. This is a mitigation, not a
                        // guarantee: a cycle blocked inside a slow backend
                        // call can still outlast `QUIT_CYCLE_WAIT`, and that
                        // case still loses the clipboard -- there is no way
                        // to force a `Drop` to run early without unwinding,
                        // and Quit must never hang waiting for a 120-second
                        // timeout to expire on the user's behalf.
                        if menu_state.busy.load(Ordering::SeqCst) {
                            let start = Instant::now();
                            while menu_state.busy.load(Ordering::SeqCst)
                                && start.elapsed() < QUIT_CYCLE_WAIT
                            {
                                std::thread::sleep(Duration::from_millis(20));
                            }
                            if menu_state.busy.load(Ordering::SeqCst) {
                                let line = "QUIT cycle still in flight after \
                                            bounded wait; exiting anyway"
                                    .to_string();
                                eprintln!("{line}");
                                status::append_log(&menu_state.config_dir, &line);
                            }
                        }
                        app.exit(0)
                    }
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
            // `commands::handle_close_request`'s call: every config mutation
            // now persists to disk as it happens (see `ConfigState.commit`
            // in config.svelte.ts), so there is no unsaved state left to ask
            // the frontend about -- it just hides and re-applies. That
            // re-apply is the Task-4 defensive config re-apply (hotkey
            // recording, Task 8, suspends global shortcuts while it listens;
            // this is the one exit path the frontend cannot reliably
            // intercept to resume them itself), run on every route that
            // actually hides the window, never on a route that doesn't.
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
