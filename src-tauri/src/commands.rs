use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{apply_config, config, AppState};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub ok: bool,
    pub error: Option<String>,
    pub problems: Vec<String>,
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<config::Config, String> {
    state
        .config
        .lock()
        .map(|c| c.clone())
        .map_err(|_| "config lock poisoned".to_string())
}

#[tauri::command]
pub fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: config::Config,
) -> SaveResult {
    // Validate through the same path the file loader uses, so the UI cannot
    // persist something a restart would then reject.
    let json = match serde_json::to_string_pretty(&config) {
        Ok(j) => j,
        Err(e) => return SaveResult { ok: false, error: Some(e.to_string()), problems: vec![] },
    };
    let validated = match config::parse(&json) {
        Ok(c) => c,
        Err(e) => return SaveResult { ok: false, error: Some(e.to_string()), problems: vec![] },
    };

    let path = config::config_path(&app);
    if let Err(e) = std::fs::write(&path, &json) {
        return SaveResult { ok: false, error: Some(e.to_string()), problems: vec![] };
    }

    let problems = apply_config(&app, state.inner(), validated);
    SaveResult { ok: true, error: None, problems }
}

#[tauri::command]
pub fn suspend_hotkeys(app: AppHandle) -> Result<(), String> {
    app.global_shortcut().unregister_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_hotkeys(app: AppHandle, state: State<'_, AppState>) -> Vec<String> {
    let config = match state.config.lock() {
        Ok(c) => c.clone(),
        Err(_) => return vec!["config lock poisoned".into()],
    };
    apply_config(&app, &state, config)
}
