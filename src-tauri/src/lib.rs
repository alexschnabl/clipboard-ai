mod backend;
mod cfhtml;
mod commands;
mod config;
mod cursor;
mod cycle;
mod hotkeys;
mod history;
mod osops;
mod richtext;
mod sentinel;
mod settings;
mod status;
mod tray_anim;

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

use hotkeys::apply_config;

#[derive(Clone)]
struct CommandEntry {
    id: String,
    name: String,
    hotkey: String,
    system_prompt: String,
    formatting: config::Formatting,
    backend: Arc<dyn backend::Backend>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub command_id: Option<String>,
    pub message: String,
}

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<config::Config>>,
    commands: Arc<Mutex<HashMap<Shortcut, CommandEntry>>>,
    ops: Arc<dyn osops::OsOps>,
    busy: Arc<AtomicBool>,
    last_status: Arc<Mutex<String>>,
    status_item: MenuItem<tauri::Wry>,
    config_dir: PathBuf,
    config_load_error: Option<commands::ConfigLoadError>,
    startup_problems: Arc<Mutex<Vec<Problem>>>,
}

fn record_history(
    state: &AppState,
    command_id: &str,
    command_name: &str,
    backend_name: &str,
    outcome: &cycle::CycleOutcome,
    total_cycle_ms: u64,
) {
    let recorded = if outcome.pasted {
        history::Outcome::succeeded()
    } else if let Some(detail) = &outcome.detail {
        history::Outcome::failed(format!("{}: {detail}", outcome.reason))
    } else {
        history::Outcome::aborted(outcome.reason)
    };

    let entry = history::HistoryEntry {
        timestamp_ms: 0,
        command_id: command_id.to_string(),
        command_name: command_name.to_string(),
        backend_name: backend_name.to_string(),
        outcome: recorded,
        backend_ms: outcome.backend_ms,
        total_cycle_ms: Some(total_cycle_ms),
        input_text: outcome.captured_text.clone(),
        result_text: outcome.result_text.clone(),
    };

    if let Err(e) = history::append(&state.config_dir.join(history::FILE_NAME), entry) {
        let line = format!("HISTORY append failed: {e}");
        eprintln!("{line}");
        status::append_log(&state.config_dir, &line);
    }
}

const QUIT_CYCLE_WAIT: Duration = Duration::from_secs(2);

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

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn set_status(state: &AppState, text: String) {
    {
        let mut last = recover_lock(state.last_status.lock(), "last_status", Some(&state.config_dir));
        *last = text;
    }
    let last = recover_lock(state.last_status.lock(), "last_status", Some(&state.config_dir));
    let _ = state.status_item.set_text(&*last);
}

struct BusyGuard<'a>(&'a AtomicBool);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn show_settings(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

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
        config::Config {
            version: 1,
            backends: Vec::new(),
            commands: Vec::new(),
            settings: config::Settings::default(),
        },
        commands::ConfigLoadError { reason: e.to_string(), preserved_path },
    )
}

#[cfg(test)]
mod recover_lock_tests {
    use super::*;

    #[test]
    fn recovers_a_poisoned_mutex_and_the_data_survives() {
        let mutex = Mutex::new(vec![1, 2, 3]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = mutex.lock().unwrap();
            panic!("simulated panic while holding the lock");
        }));
        assert!(result.is_err(), "the panic must have actually happened");
        assert!(mutex.is_poisoned(), "the mutex should now be poisoned");

        let guard = recover_lock(mutex.lock(), "test", None);
        assert_eq!(*guard, vec![1, 2, 3], "the data must survive the poisoning intact");
    }
}

#[cfg(test)]
mod panic_message_tests {
    use super::*;

    #[test]
    fn extracts_a_str_literal_panic_payload() {
        let result: std::thread::Result<()> = std::panic::catch_unwind(|| panic!("literal message"));
        let payload = result.unwrap_err();
        assert_eq!(panic_message(&*payload), "literal message");
    }

    #[test]
    fn extracts_a_formatted_string_panic_payload() {
        let detail = "detail";
        let result: std::thread::Result<()> =
            std::panic::catch_unwind(move || panic!("formatted: {detail}"));
        let payload = result.unwrap_err();
        assert_eq!(panic_message(&*payload), "formatted: detail");
    }

    #[test]
    fn falls_back_to_a_fixed_string_for_a_non_string_payload() {
        let result: std::thread::Result<()> =
            std::panic::catch_unwind(|| std::panic::panic_any(42i32));
        let payload = result.unwrap_err();
        assert_eq!(panic_message(&*payload), "non-string panic payload");
    }
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
            load_error.reason.contains("valid JSON"),
            "reason should describe the parse failure, got: {}",
            load_error.reason
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_no_preserved_path_when_there_was_nothing_to_preserve() {
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
            let config_dir = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));

            let mut config_load_failed = false;
            let mut config_load_error: Option<commands::ConfigLoadError> = None;
            let cfg = match config::load_or_create(&path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    config_load_failed = true;
                    let (fallback, load_error) = handle_config_load_failure(&path, &config_dir, e);
                    config_load_error = Some(load_error);
                    fallback
                }
            };

            let mut problems: Vec<Problem> = Vec::new();

            match cursor::restore_if_left_busy(&config_dir) {
                Some(Ok(())) => {
                    let line = "STARTUP busy cursor left behind by a previous run; restored"
                        .to_string();
                    eprintln!("{line}");
                    status::append_log(&config_dir, &line);
                }
                Some(Err(e)) => {
                    let line = format!("STARTUP busy cursor left behind; restore failed: {e}");
                    eprintln!("{line}");
                    status::append_log(&config_dir, &line);
                }
                None => {}
            }

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

            let state = Arc::new(AppState {
                config: Arc::new(Mutex::new(config::Config {
                    version: 1,
                    backends: Vec::new(),
                    commands: Vec::new(),
                    settings: config::Settings::default(),
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

            app.manage((*state).clone());

            app.handle().plugin(tauri_plugin_autostart::Builder::new().build())?;

            let handler_state = state.clone();
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_handler(move |app, shortcut, event| {
                        if event.state() != ShortcutState::Released {
                            return;
                        }
                        let key = *shortcut;
                        let state = handler_state.clone();
                        let app = app.clone();

                        if state.busy.swap(true, Ordering::SeqCst) {
                            let line = "busy; hotkey ignored".to_string();
                            println!("{line}");
                            status::append_log(&state.config_dir, &line);
                            set_status(&state, line);
                            return;
                        }

                        std::thread::spawn(move || {
                            let _busy_guard = BusyGuard(&state.busy);
                            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                let entry = recover_lock(
                                    state.commands.lock(),
                                    "commands",
                                    Some(&state.config_dir),
                                )
                                .get(&key)
                                .cloned();
                                if let Some(CommandEntry {
                                    id, name, system_prompt, formatting, backend, ..
                                }) = entry
                                {
                                    let indicator = tray_anim::TrayAnimation::begin(
                                        app.clone(),
                                        state.config_dir.clone(),
                                    );
                                    let cycle_start = state.ops.now_ms();
                                    let outcome = cycle::run(
                                        backend.as_ref(),
                                        &system_prompt,
                                        formatting,
                                        state.ops.as_ref(),
                                    );
                                    let total_cycle_ms =
                                        state.ops.now_ms().saturating_sub(cycle_start);
                                    indicator.finish();
                                    let line = status::log_line(&outcome, &name, backend.name());
                                    println!("{line}");
                                    status::append_log(&state.config_dir, &line);

                                    set_status(&state, status::summary(&outcome, &name));

                                    record_history(
                                        &state,
                                        &id,
                                        &name,
                                        backend.name(),
                                        &outcome,
                                        total_cycle_ms,
                                    );
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
            let autostart_wanted = cfg.settings.autostart;
            problems.extend(apply_config(&handle, &state, cfg));
            if let Err(e) = settings::apply_autostart(&handle, autostart_wanted) {
                problems.push(Problem { command_id: None, message: e });
            }
            *recover_lock(state.startup_problems.lock(), "startup_problems", Some(&config_dir)) =
                problems.clone();
            let hotkeys_attempted =
                recover_lock(state.commands.lock(), "commands", Some(&config_dir)).len();

            let mut command_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
            {
                let guard = recover_lock(state.commands.lock(), "commands", Some(&config_dir));
                for (i, (_shortcut, entry)) in guard.iter().enumerate() {
                    let label = format!("{} \u{2014} {}", entry.name, entry.hotkey);
                    let id = format!("command-{i}");
                    command_items.push(MenuItem::with_id(app, id, label, false, None::<&str>)?);
                }
            }

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
            let mut tray_builder = TrayIconBuilder::with_id(tray_anim::TRAY_ID)
                .tooltip("Clipboard AI")
                .menu(&menu);
            match tray_anim::tray_icon().or_else(|| app.default_window_icon().cloned()) {
                Some(icon) => tray_builder = tray_builder.icon(icon),
                None => {
                    let line = "STARTUP tray-icon-missing; continuing without one".to_string();
                    eprintln!("{line}");
                    status::append_log(&config_dir, &line);
                }
            }
            tray_builder
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
                        let _ = app.global_shortcut().unregister_all();
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
                        let _ = cursor::restore_if_left_busy(&menu_state.config_dir);
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

            if let Some(window) = app.get_webview_window("settings") {
                let handle = handle.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
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
            commands::get_history,
            commands::clear_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
