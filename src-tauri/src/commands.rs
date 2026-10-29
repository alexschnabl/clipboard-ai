use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{config, history, hotkeys::apply_config, AppState, Problem};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub ok: bool,
    pub error: Option<String>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLoadError {
    pub reason: String,
    pub preserved_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigResult {
    pub config: config::Config,
    pub load_error: Option<ConfigLoadError>,
    pub startup_problems: Vec<Problem>,
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> GetConfigResult {
    let config =
        crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone();
    let startup_problems =
        crate::recover_lock(state.startup_problems.lock(), "startup_problems", Some(&state.config_dir))
            .clone();
    GetConfigResult { config, load_error: state.config_load_error.clone(), startup_problems }
}

fn decide_save(config: &config::Config, path: &Path) -> Result<config::Config, String> {
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let validated = config::parse(&json).map_err(|e| e.to_string())?;

    config::write_atomic(path, &validated).map_err(|e| e.to_string())?;

    Ok(validated)
}

#[tauri::command]
pub fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: config::Config,
) -> SaveResult {
    let path = config::config_path(&app);
    match decide_save(&config, &path) {
        Err(e) => SaveResult { ok: false, error: Some(e), problems: vec![] },
        Ok(validated) => {
            let autostart_wanted = validated.settings.autostart;
            let mut problems = apply_config(&app, state.inner(), validated);
            if let Err(e) = crate::settings::apply_autostart(&app, autostart_wanted) {
                problems.push(Problem { command_id: None, message: e });
            }
            SaveResult { ok: true, error: None, problems }
        }
    }
}

#[cfg(test)]
mod save_config_tests {
    use super::*;
    use std::fs;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cai-test-save-config-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_invalid_config_is_rejected_before_anything_is_written_to_disk() {
        let cases: [(&str, fn(&mut config::Config)); 2] = [
            ("a command naming an unknown backend must be rejected", |bad| {
                bad.commands[0].backend_id = "does-not-exist".into();
            }),
            ("a duplicate command id must be rejected", |bad| {
                bad.commands.push(bad.commands[0].clone());
            }),
        ];

        for (description, mutate) in cases {
            let dir = temp_dir("rejected");
            let path = dir.join("config.json");
            let mut bad = config::default_config();
            mutate(&mut bad);

            let decision = decide_save(&bad, &path);

            assert!(decision.is_err(), "{description}");
            assert!(!path.exists(), "a rejected config must never be written to disk ({description})");

            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_valid_config_is_validated_and_then_written_to_disk() {
        let dir = temp_dir("accepted");
        let path = dir.join("config.json");
        let cfg = config::default_config();

        match decide_save(&cfg, &path) {
            Ok(validated) => {
                assert_eq!(validated.commands.len(), cfg.commands.len());
            }
            Err(e) => panic!("expected a valid config to be accepted, got: {e}"),
        }
        assert!(path.exists(), "a validated config must be written to disk");
        let on_disk = fs::read_to_string(&path).unwrap();
        config::parse(&on_disk).expect("the written file must itself be valid config");

        let _ = fs::remove_dir_all(&dir);
    }
}

#[tauri::command]
pub fn suspend_hotkeys(app: AppHandle) -> Result<(), String> {
    app.global_shortcut().unregister_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_hotkeys(app: AppHandle, state: State<'_, AppState>) -> Vec<Problem> {
    let config =
        crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone();
    apply_config(&app, &state, config)
}

pub(crate) fn handle_close_request(app: AppHandle, state: Option<AppState>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.hide();
        let _ = app.emit("settings://window-hidden", ());
    }
    if let Some(state) = state {
        let app_for_task = app.clone();
        if let Err(e) = app.run_on_main_thread(move || {
            let config =
                crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir))
                    .clone();
            let problems = apply_config(&app_for_task, &state, config);
            for problem in &problems {
                let line = format!("CONFIG reapply-on-close problem: {}", problem.message);
                eprintln!("{line}");
                crate::status::append_log(&state.config_dir, &line);
            }
        }) {
            eprintln!("failed to schedule config re-apply on settings close: {e}");
        }
    }
}

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Vec<history::HistoryEntry> {
    history::load(&state.config_dir.join(history::FILE_NAME))
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    history::clear(&state.config_dir.join(history::FILE_NAME)).map_err(|e| e.to_string())
}
