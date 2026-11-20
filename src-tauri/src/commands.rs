use std::{collections::HashMap, path::Path};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{
    config, health, history,
    hotkeys::apply_config,
    secrets::{self, SecretStore},
    AppState, Problem,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub ok: bool,
    pub error: Option<String>,
    pub problems: Vec<Problem>,
    pub stored_keys: Vec<String>,
    pub inline_keys: Vec<String>,
    pub is_default_setup: bool,
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
    pub stored_keys: Vec<String>,
    pub inline_keys: Vec<String>,
    pub is_default_setup: bool,
}

fn without_keys(mut config: config::Config) -> config::Config {
    for backend in &mut config.backends {
        backend.api_key = None;
    }
    config
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> GetConfigResult {
    let config =
        crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone();
    let startup_problems =
        crate::recover_lock(state.startup_problems.lock(), "startup_problems", Some(&state.config_dir))
            .clone();
    let stored_keys = secrets::stored_key_ids(&config, &secrets::CredentialManager);
    let inline_keys = secrets::inline_key_ids(&config);
    let is_default_setup = config::is_default_setup(&config);
    GetConfigResult {
        config: without_keys(config),
        load_error: state.config_load_error.clone(),
        startup_problems,
        stored_keys,
        inline_keys,
        is_default_setup,
    }
}

fn decide_save(
    config: &config::Config,
    previous: &config::Config,
    key_changes: &HashMap<String, Option<String>>,
    removed: &[String],
    path: &Path,
    store: &dyn SecretStore,
) -> Result<(config::Config, Vec<String>), String> {
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let mut validated = config::parse(&json).map_err(|e| e.to_string())?;

    let applied = secrets::apply_key_changes(&mut validated, previous, key_changes, store)?;
    if let Err(e) = config::write_atomic(path, &validated) {
        let failed = applied.roll_back(store);
        if !failed.is_empty() {
            if let Some(dir) = path.parent() {
                crate::status::append_log(
                    dir,
                    &format!("SECRETS roll-back-failed {}", failed.join("; ")),
                );
            }
        }
        return Err(e.to_string());
    }

    let mut warnings = applied.warnings;
    warnings.extend(secrets::delete_removed(previous, &validated, removed, store));
    Ok((validated, warnings))
}

#[tauri::command]
pub fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: config::Config,
    key_changes: Option<HashMap<String, Option<String>>>,
    removed_backends: Option<Vec<String>>,
) -> SaveResult {
    let path = config::config_path(&app);
    let store = secrets::CredentialManager;
    let previous =
        crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone();
    let key_changes = key_changes.unwrap_or_default();
    let removed = removed_backends.unwrap_or_default();
    match decide_save(&config, &previous, &key_changes, &removed, &path, &store) {
        Err(e) => SaveResult {
            ok: false,
            error: Some(e),
            problems: vec![],
            stored_keys: secrets::stored_key_ids(&previous, &store),
            inline_keys: secrets::inline_key_ids(&previous),
            is_default_setup: config::is_default_setup(&previous),
        },
        Ok((validated, warnings)) => {
            let autostart_wanted = validated.settings.autostart;
            let stored_keys = secrets::stored_key_ids(&validated, &store);
            let inline_keys = secrets::inline_key_ids(&validated);
            let is_default_setup = config::is_default_setup(&validated);
            let mut problems = apply_config(&app, state.inner(), validated);
            if let Err(e) = crate::settings::apply_autostart(&app, autostart_wanted) {
                problems.push(Problem { command_id: None, message: e });
            }
            for warning in warnings {
                crate::status::append_log(&state.config_dir, &format!("SECRETS {warning}"));
                problems.push(Problem { command_id: None, message: warning });
            }
            SaveResult { ok: true, error: None, problems, stored_keys, inline_keys, is_default_setup }
        }
    }
}

#[tauri::command]
pub fn get_api_key(state: State<'_, AppState>, backend_id: String) -> Result<Option<String>, String> {
    let inline = crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir))
        .backend(&backend_id)
        .and_then(|b| b.api_key.clone());
    secrets::resolve_api_key(&backend_id, inline.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_backend(
    state: State<'_, AppState>,
    backend_id: String,
) -> Result<health::Health, String> {
    let backend = crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir))
        .backend(&backend_id)
        .cloned()
        .ok_or_else(|| format!("no backend with id {backend_id}"))?;
    tauri::async_runtime::spawn_blocking(move || health::check(&backend))
        .await
        .map_err(|e| e.to_string())
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

            let decision = decide_save(
                &bad,
                &config::default_config(),
                &HashMap::new(),
                &[],
                &path,
                &secrets::fake::FakeStore::default(),
            );

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

        match decide_save(&cfg, &cfg, &HashMap::new(), &[], &path, &secrets::fake::FakeStore::default()) {
            Ok((validated, _)) => {
                assert_eq!(validated.commands.len(), cfg.commands.len());
            }
            Err(e) => panic!("expected a valid config to be accepted, got: {e}"),
        }
        assert!(path.exists(), "a validated config must be written to disk");
        let on_disk = fs::read_to_string(&path).unwrap();
        config::parse(&on_disk).expect("the written file must itself be valid config");

        let _ = fs::remove_dir_all(&dir);
    }

    fn keyed(id: &str) -> config::BackendConfig {
        config::BackendConfig {
            id: id.into(),
            name: id.to_uppercase(),
            kind: config::KIND_ANTHROPIC.into(),
            base_url: None,
            model: None,
            api_key: None,
            timeout_ms: 120_000,
            effort: config::Effort::Low,
            expects_response: true,
        }
    }

    #[test]
    fn a_new_key_lands_in_the_store_and_never_in_the_file() {
        let dir = temp_dir("key-to-store");
        let path = dir.join("config.json");
        let mut cfg = config::default_config();
        cfg.backends.push(keyed("api"));
        let store = secrets::fake::FakeStore::default();
        let changes = HashMap::from([("api".to_string(), Some("sk-test-123".to_string()))]);

        let (_, warnings) = decide_save(&cfg, &cfg, &changes, &[], &path, &store).unwrap();

        assert!(warnings.is_empty());
        assert!(!fs::read_to_string(&path).unwrap().contains("sk-test-123"));
        assert_eq!(store.get(&secrets::target_for("api")).as_deref(), Some("sk-test-123"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_file_write_puts_the_previous_key_back() {
        let dir = temp_dir("rollback");
        let path = dir.join("missing-dir").join("config.json");
        let mut cfg = config::default_config();
        cfg.backends.push(keyed("api"));
        let store = secrets::fake::FakeStore::default();
        store.put(&secrets::target_for("api"), "sk-old");
        let changes = HashMap::from([("api".to_string(), Some("sk-new".to_string()))]);

        assert!(decide_save(&cfg, &cfg, &changes, &[], &path, &store).is_err());
        assert_eq!(store.get(&secrets::target_for("api")).as_deref(), Some("sk-old"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn removing_a_backend_removes_its_credential_after_the_file_is_written() {
        let dir = temp_dir("delete-backend");
        let path = dir.join("config.json");
        let mut previous = config::default_config();
        previous.backends.push(keyed("api"));
        let next = config::default_config();
        let store = secrets::fake::FakeStore::default();
        store.put(&secrets::target_for("api"), "sk-old");

        decide_save(&next, &previous, &HashMap::new(), &["api".to_string()], &path, &store).unwrap();

        assert!(store.get(&secrets::target_for("api")).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rejected_config_touches_no_credential() {
        let dir = temp_dir("rejected-keys");
        let path = dir.join("config.json");
        let mut bad = config::default_config();
        bad.backends.push(keyed("api"));
        bad.commands[0].backend_id = "nope".into();
        let store = secrets::fake::FakeStore::default();
        let changes = HashMap::from([("api".to_string(), Some("sk-new".to_string()))]);

        assert!(decide_save(&bad, &bad, &changes, &[], &path, &store).is_err());
        assert!(store.get(&secrets::target_for("api")).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_config_never_hands_out_a_key() {
        let mut cfg = config::default_config();
        let mut b = keyed("api");
        b.api_key = Some("sk-inline".into());
        cfg.backends.push(b);
        let stripped = without_keys(cfg);
        assert!(stripped.backends.iter().all(|b| b.api_key.is_none()));
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

#[tauri::command]
pub fn quit_app(app: AppHandle, state: State<'_, AppState>) {
    crate::quit_app(&app, &state);
}

#[tauri::command]
pub fn hud_measured(state: State<'_, AppState>, id: u64, width: f64, height: f64, inset: f64) {
    state.hud.measured(id, width, height, inset);
}

#[tauri::command]
pub fn palette_measured(
    app: AppHandle,
    state: State<'_, AppState>,
    session: u64,
    width: f64,
    height: f64,
    inset: f64,
) {
    crate::palette::measured(&app, &state, session, width, height, inset);
}

#[tauri::command]
pub fn palette_run(
    app: AppHandle,
    state: State<'_, AppState>,
    session: u64,
    choice: crate::palette::PaletteChoice,
) {
    crate::palette::run(&app, &state, session, choice);
}

#[tauri::command]
pub fn palette_close(app: AppHandle, state: State<'_, AppState>, session: u64) {
    crate::palette::close(&app, &state, Some(session), true);
}
