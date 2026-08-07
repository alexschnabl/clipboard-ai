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
struct PromptEntry {
    name: String,
    system_prompt: String,
    backend: Arc<dyn backend::Backend>,
}

/// Per-hotkey dispatch table plus the machinery a cycle needs to run.
///
/// `prompts` is keyed by the parsed [`Shortcut`] struct rather than by a
/// string. See `build_prompt_map` for why: it avoids reconciling the
/// config's hotkey spelling against the plugin's own textual rendering of
/// the same shortcut.
struct AppState {
    prompts: Mutex<HashMap<Shortcut, PromptEntry>>,
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

/// Build the hotkey -> prompt-entry map from config.
///
/// A prompt whose hotkey will not parse, or whose backend will not build, is
/// skipped with a message rather than aborting the whole load — one bad
/// entry must not take the other prompts down with it.
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
fn build_prompt_map(cfg: &config::Config) -> (HashMap<Shortcut, PromptEntry>, Vec<String>) {
    let mut map = HashMap::new();
    let mut problems = Vec::new();
    for prompt in &cfg.prompts {
        let Some(backend_cfg) = cfg.backend(&prompt.backend_id) else {
            problems.push(format!("{}: unknown backend", prompt.name));
            continue;
        };
        let shortcut = match prompt.hotkey.parse::<Shortcut>() {
            Ok(s) => s,
            Err(e) => {
                problems.push(format!(
                    "{}: unparseable hotkey {:?} ({e})",
                    prompt.name, prompt.hotkey
                ));
                continue;
            }
        };
        match backend::build(backend_cfg) {
            Ok(backend) => {
                let entry = PromptEntry {
                    name: prompt.name.clone(),
                    system_prompt: prompt.system_prompt.clone(),
                    backend,
                };
                // `config::validate` dedups hotkeys case-insensitively but
                // stores the raw spelling, so two entries that spell the
                // same modifier differently (e.g. "Ctrl+Alt+J" and
                // "Control+Alt+J") both pass validation even though they
                // parse to the identical `Shortcut`. An unchecked insert
                // would silently drop one prompt's binding with no signal
                // anywhere; report it as a problem instead.
                if let Some(previous) = map.insert(shortcut, entry) {
                    problems.push(format!(
                        "{}: hotkey {:?} collides with \"{}\"'s hotkey after parsing ({shortcut}); \
                         only \"{}\" is registered",
                        prompt.name, prompt.hotkey, previous.name, previous.name
                    ));
                }
            }
            Err(e) => problems.push(format!("{}: {e}", prompt.name)),
        }
    }
    (map, problems)
}

#[cfg(test)]
mod build_prompt_map_tests {
    use super::*;
    use crate::config::{BackendConfig, Config, PromptConfig, KIND_CLAUDE_CLI};

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

    fn prompt(id: &str, name: &str, hotkey: &str) -> PromptConfig {
        PromptConfig {
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
        // insert, the second prompt's hotkey would silently replace the
        // first's in the map with no signal anywhere that it happened.
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            prompts: vec![
                prompt("one", "First", "Ctrl+Alt+J"),
                prompt("two", "Second", "Control+Alt+J"),
            ],
        };
        let (map, problems) = build_prompt_map(&cfg);
        assert_eq!(
            map.len(),
            1,
            "both config entries parse to the same Shortcut, so only one hotkey can ever fire"
        );
        assert!(
            problems.iter().any(|p| p.contains("Second")),
            "the dropped prompt's collision must be reported as a problem, got: {problems:?}"
        );
    }

    #[test]
    fn non_colliding_hotkeys_register_cleanly_with_no_problems() {
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            prompts: vec![prompt("one", "First", "Ctrl+Alt+J"), prompt("two", "Second", "Ctrl+Alt+K")],
        };
        let (map, problems) = build_prompt_map(&cfg);
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
            // fall through with an empty config (no backends, no prompts --
            // `build_prompt_map` below then registers nothing rather than
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
                    config::Config { version: 1, backends: Vec::new(), prompts: Vec::new() }
                }
            };

            let (prompt_map, mut problems) = build_prompt_map(&cfg);
            let hotkeys: Vec<Shortcut> = prompt_map.keys().copied().collect();

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

            // One disabled, informational entry per successfully-registered
            // prompt, built from `prompt_map` rather than all of
            // `cfg.prompts` -- a prompt whose hotkey didn't parse, whose
            // backend is unknown, or whose hotkey collided with another
            // (see build_prompt_map) is not in this map, and the tray must
            // not advertise a hotkey that does nothing.
            let mut prompt_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
            for (i, (shortcut, entry)) in prompt_map.iter().enumerate() {
                let label = format!("{} \u{2014} {shortcut}", entry.name);
                let id = format!("prompt-{i}");
                prompt_items.push(MenuItem::with_id(app, id, label, false, None::<&str>)?);
            }

            let state = Arc::new(AppState {
                prompts: Mutex::new(prompt_map),
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
                                .prompts
                                .lock()
                                .ok()
                                .and_then(|m| m.get(&key).cloned());
                            if let Some(PromptEntry { name, system_prompt, backend }) = entry {
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

            problems.extend(register_hotkeys(&handle, &hotkeys));

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
                    "STARTUP backends={} prompts-configured={} hotkeys-attempted={} problems={}",
                    cfg.backends.len(),
                    cfg.prompts.len(),
                    hotkeys.len(),
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
            let sep_after_prompts = PredefinedMenuItem::separator(app)?;
            let sep_before_quit = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let mut items: Vec<&dyn IsMenuItem<_>> = vec![&status_item, &sep_after_status];
            for item in &prompt_items {
                items.push(item);
            }
            if !prompt_items.is_empty() {
                items.push(&sep_after_prompts);
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
                        // Deliberate v1 narrowing: hotkeys and backends are
                        // built once here at startup, not watched. Live
                        // re-registration would mean unregistering shortcuts
                        // and rebuilding backends under a lock while a cycle
                        // might already be in flight on its own thread —
                        // real concurrency risk that isn't worth taking in
                        // this pass. It lands with the settings UI in v2,
                        // where reload has a real trigger. For now, restart
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
