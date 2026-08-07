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
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Per-hotkey dispatch table plus the machinery a cycle needs to run.
///
/// `prompts` is keyed by the parsed [`Shortcut`] struct rather than by a
/// string. See `build_prompt_map` for why: it avoids reconciling the
/// config's hotkey spelling against the plugin's own textual rendering of
/// the same shortcut.
struct AppState {
    prompts: Mutex<HashMap<Shortcut, (String, Arc<dyn backend::Backend>)>>,
    ops: Arc<dyn osops::OsOps>,
    busy: AtomicBool,
    /// Set after every cycle. Not yet surfaced in the UI — a hook for a
    /// future "last status" tray item/tooltip, which lands with the
    /// settings UI in v2 alongside live config reload.
    #[allow(dead_code)]
    last_status: Mutex<String>,
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

/// Build the hotkey -> (system prompt, backend) map from config.
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
fn build_prompt_map(
    cfg: &config::Config,
) -> (HashMap<Shortcut, (String, Arc<dyn backend::Backend>)>, Vec<String>) {
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
            Ok(b) => {
                map.insert(shortcut, (prompt.system_prompt.clone(), b));
            }
            Err(e) => problems.push(format!("{}: {e}", prompt.name)),
        }
    }
    (map, problems)
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
            let cfg = config::load_or_create(&path).map_err(|e| {
                eprintln!("config error: {e}");
                e
            })?;
            let (prompt_map, mut problems) = build_prompt_map(&cfg);
            let hotkeys: Vec<Shortcut> = prompt_map.keys().copied().collect();

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
                        let key = *shortcut;
                        let state = handler_state.clone();

                        if state.busy.swap(true, Ordering::SeqCst) {
                            println!("busy; hotkey ignored");
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
                            if let Some((system_prompt, backend)) = entry {
                                let outcome =
                                    cycle::run(backend.as_ref(), &system_prompt, state.ops.as_ref());
                                let status = format!(
                                    "{} — {} [{}] (capture {} ms, backend {} ms)",
                                    backend.name(),
                                    outcome.reason,
                                    if outcome.pasted { "pasted" } else { "not pasted" },
                                    outcome.captured_ms,
                                    outcome.backend_ms
                                );
                                println!("{status}");
                                if let Ok(mut last) = state.last_status.lock() {
                                    *last = status;
                                }
                            }
                        });
                    })
                    .build(),
            )?;

            problems.extend(register_hotkeys(&handle, &hotkeys));
            for problem in &problems {
                eprintln!("config problem: {problem}");
            }

            let open_config =
                MenuItem::with_id(app, "open-config", "Open config file", true, None::<&str>)?;
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
