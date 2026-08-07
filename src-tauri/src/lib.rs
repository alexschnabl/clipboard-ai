mod backend;
mod config;
mod cycle;
mod osops;
mod sentinel;
mod status;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use tauri::{
    menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// A single hotkey's dispatch data: the display name (used in the log and
/// tray status), the system prompt to send, and the backend to send it to.
#[derive(Clone)]
struct CommandEntry {
    name: String,
    system_prompt: String,
    backend: Arc<dyn backend::Backend>,
}

/// Per-hotkey dispatch table plus the machinery a cycle needs to run.
///
/// `commands` is keyed by the parsed [`Shortcut`] struct rather than by a
/// string. See `build_command_map` for why: it avoids reconciling the
/// config's hotkey spelling against the plugin's own textual rendering of
/// the same shortcut.
struct AppState {
    /// The in-memory source of truth for the running config. `apply_config`
    /// is the only writer, at startup and (later) on every settings-window
    /// save; it replaces this wholesale rather than mutating it in place,
    /// the same way it replaces `commands` below wholesale rather than
    /// patching it.
    config: Mutex<config::Config>,
    commands: Mutex<HashMap<Shortcut, CommandEntry>>,
    ops: Arc<dyn osops::OsOps>,
    busy: AtomicBool,
    /// The last status line, mirrored onto the disabled "status" tray menu
    /// item below. This is the only console a release build has — see
    /// `set_status`.
    last_status: Mutex<String>,
    /// Handle to the disabled tray menu item that always shows
    /// `last_status`. Kept as a live handle (not rebuilt) because
    /// `MenuItem::set_text` can update it in place from any thread — it
    /// hops to the main thread internally.
    status_item: MenuItem<tauri::Wry>,
    /// Directory holding `config.json`; `clipboard-ai.log` lives alongside
    /// it — see `status::append_log`.
    config_dir: PathBuf,
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
fn build_command_map(cfg: &config::Config) -> (HashMap<Shortcut, CommandEntry>, Vec<String>) {
    let mut map = HashMap::new();
    let mut problems = Vec::new();
    for command in &cfg.commands {
        let Some(backend_cfg) = cfg.backend(&command.backend_id) else {
            problems.push(format!("{}: unknown backend", command.name));
            continue;
        };
        let shortcut = match command.hotkey.parse::<Shortcut>() {
            Ok(s) => s,
            Err(e) => {
                problems.push(format!(
                    "{}: unparseable hotkey {:?} ({e})",
                    command.name, command.hotkey
                ));
                continue;
            }
        };
        match backend::build(backend_cfg) {
            Ok(backend) => {
                let entry = CommandEntry {
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
                    problems.push(format!(
                        "{}: hotkey {:?} collides with \"{}\"'s hotkey after parsing ({shortcut}); \
                         only \"{}\" is registered",
                        command.name, command.hotkey, previous.name, previous.name
                    ));
                }
            }
            Err(e) => problems.push(format!("{}: {e}", command.name)),
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
            problems.iter().any(|p| p.contains("Second")),
            "the dropped command's collision must be reported as a problem, got: {problems:?}"
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

fn register_hotkeys(app: &tauri::AppHandle, shortcuts: &[Shortcut]) -> Vec<String> {
    let mut problems = Vec::new();
    for shortcut in shortcuts {
        if let Err(e) = app.global_shortcut().register(*shortcut) {
            problems.push(format!("{shortcut}: already in use ({e})"));
        }
    }
    problems
}

/// Unregisters every hotkey, rebuilds the command map from `config`, swaps
/// both `state.commands` and `state.config` to the new values, then
/// re-registers with the OS. Returns per-command problems that did not
/// prevent the apply (an unparseable hotkey, an unknown backend, a hotkey
/// collision -- see `build_command_map` -- or a hotkey the OS refused to
/// register, e.g. one already owned by another application).
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
fn apply_config(app: &tauri::AppHandle, state: &AppState, config: config::Config) -> Vec<String> {
    let _ = app.global_shortcut().unregister_all();

    let (map, mut problems) = build_command_map(&config);
    let shortcuts: Vec<Shortcut> = map.keys().copied().collect();

    if let Ok(mut guard) = state.commands.lock() {
        *guard = map;
    }
    if let Ok(mut guard) = state.config.lock() {
        *guard = config;
    }

    problems.extend(register_hotkeys(app, &shortcuts));
    problems
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
            let cfg = match config::load_or_create(&path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    config_load_failed = true;
                    let line = format!("STARTUP config-load-failed reason={e}");
                    eprintln!("{line}");
                    status::append_log(&config_dir, &line);
                    config::Config { version: 1, backends: Vec::new(), commands: Vec::new() }
                }
            };

            let mut problems: Vec<String> = Vec::new();

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
                    problems.push(format!(
                        "input/clipboard system failed to initialize ({e}); hotkeys will do \
                         nothing until the app is restarted"
                    ));
                    Arc::new(osops::NullOps::new())
                }
            };

            let status_item =
                MenuItem::with_id(app, "status", "starting…", false, None::<&str>)?;

            // Built empty: `apply_config` below is the sole writer of both
            // `commands` and `config`, at startup here and (later) on every
            // settings-window save.
            let state = Arc::new(AppState {
                config: Mutex::new(config::Config {
                    version: 1,
                    backends: Vec::new(),
                    commands: Vec::new(),
                }),
                commands: Mutex::new(HashMap::new()),
                ops,
                busy: AtomicBool::new(false),
                last_status: Mutex::new("starting…".into()),
                status_item: status_item.clone(),
                config_dir: config_dir.clone(),
            });

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
                            if let Some(CommandEntry { name, system_prompt, backend }) = entry {
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
                let line = format!("STARTUP problem: {problem}");
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

            let open_config =
                MenuItem::with_id(app, "open-config", "Open config file", true, None::<&str>)?;
            let reload = MenuItem::with_id(app, "reload", "Reload config", true, None::<&str>)?;
            let show_log = MenuItem::with_id(app, "show-log", "Show log", true, None::<&str>)?;
            let sep_after_status = PredefinedMenuItem::separator(app)?;
            let sep_after_commands = PredefinedMenuItem::separator(app)?;
            let sep_before_quit = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let mut items: Vec<&dyn IsMenuItem<_>> = vec![&status_item, &sep_after_status];
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
                .on_menu_event(move |app, event| match event.id.as_ref() {
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
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
