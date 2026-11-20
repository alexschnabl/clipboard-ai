use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_updater::{Update, UpdaterExt};

const STATE_FILE: &str = "update-check.json";
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const STARTUP_DELAY: Duration = Duration::from_secs(20);

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckState {
    #[serde(default)]
    last_check_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join(STATE_FILE)
}

fn last_check_ms(dir: &Path) -> u64 {
    fs::read_to_string(state_path(dir))
        .ok()
        .and_then(|raw| serde_json::from_str::<CheckState>(&raw).ok())
        .map(|s| s.last_check_ms)
        .unwrap_or(0)
}

fn record_check(dir: &Path, at_ms: u64) {
    if let Ok(json) = serde_json::to_string(&CheckState { last_check_ms: at_ms }) {
        let _ = fs::write(state_path(dir), json);
    }
}

pub(crate) fn check_is_due(last_ms: u64, now_ms: u64) -> bool {
    last_ms > now_ms || now_ms - last_ms >= DAY_MS
}

fn info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone().filter(|b| !b.trim().is_empty()),
    }
}

fn config_dir(app: &AppHandle) -> Option<PathBuf> {
    crate::config::config_path(app).parent().map(Path::to_path_buf)
}

async fn run_check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let dir = config_dir(app);
    let exit_dir = dir.clone();
    let exit_app = app.clone();
    let updater = app
        .updater_builder()
        .on_before_exit(move || {
            let _ = exit_app.global_shortcut().unregister_all();
            if let Some(state) = exit_app.try_state::<crate::AppState>() {
                crate::wait_for_running_cycle(&state);
            }
            if let Some(dir) = &exit_dir {
                let _ = crate::cursor::restore_if_left_busy(dir);
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    if let Some(dir) = &dir {
        record_check(dir, now_ms());
    }
    let summary = found.as_ref().map(info);
    *app.state::<PendingUpdate>().0.lock().unwrap_or_else(|p| p.into_inner()) = found;
    Ok(summary)
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let result = run_check(&app).await;
    if let (Err(e), Some(dir)) = (&result, config_dir(&app)) {
        crate::status::append_log(&dir, &format!("UPDATE check-failed reason={e}"));
    }
    result
}

#[tauri::command]
pub fn get_available_update(pending: State<'_, PendingUpdate>) -> Option<UpdateInfo> {
    pending.0.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(info)
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .state::<PendingUpdate>()
        .0
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .ok_or_else(|| "no update has been found yet".to_string())?;
    if let Some(dir) = config_dir(&app) {
        crate::status::append_log(
            &dir,
            &format!("UPDATE installing from={} to={}", update.current_version, update.version),
        );
    }
    let mut downloaded: u64 = 0;
    let progress_app = app.clone();
    let finish_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit("update://progress", UpdateProgress { downloaded, total });
            },
            move || crate::localapi::shutdown_for_app(&finish_app),
        )
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

pub(crate) fn wait_before_next_check(last_ms: u64, now_ms: u64) -> Duration {
    if check_is_due(last_ms, now_ms) {
        STARTUP_DELAY
    } else {
        Duration::from_millis(last_ms + DAY_MS - now_ms) + STARTUP_DELAY
    }
}

pub fn spawn_daily_check(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let Some(dir) = config_dir(&app) else { return };
    std::thread::spawn(move || loop {
        std::thread::sleep(wait_before_next_check(last_check_ms(&dir), now_ms()));
        if !check_is_due(last_check_ms(&dir), now_ms()) {
            continue;
        }
        match tauri::async_runtime::block_on(run_check(&app)) {
            Ok(Some(found)) => {
                crate::status::append_log(&dir, &format!("UPDATE available version={}", found.version));
                let _ = app.emit("update://available", found);
            }
            Ok(None) => {}
            Err(e) => {
                crate::status::append_log(&dir, &format!("UPDATE check-failed reason={e}"));
                std::thread::sleep(Duration::from_secs(60 * 60));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_is_due_once_a_day() {
        let now = 10 * DAY_MS;
        assert!(check_is_due(0, now), "never checked");
        assert!(!check_is_due(now - 1000, now), "checked a second ago");
        assert!(!check_is_due(now - DAY_MS + 1, now));
        assert!(check_is_due(now - DAY_MS, now));
        assert!(check_is_due(now + DAY_MS, now), "a clock that went backwards must not block checks");
    }

    #[test]
    fn the_next_check_waits_for_the_day_to_pass() {
        let now = 10 * DAY_MS;
        assert_eq!(wait_before_next_check(0, now), STARTUP_DELAY);
        assert_eq!(
            wait_before_next_check(now - 1000, now),
            Duration::from_millis(DAY_MS - 1000) + STARTUP_DELAY
        );
    }

    #[test]
    fn the_last_check_survives_a_restart_and_a_broken_file_reads_as_never() {
        let dir = std::env::temp_dir().join(format!("cai-test-update-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(last_check_ms(&dir), 0);
        record_check(&dir, 1234);
        assert_eq!(last_check_ms(&dir), 1234);
        fs::write(state_path(&dir), "{broken").unwrap();
        assert_eq!(last_check_ms(&dir), 0);
        let _ = fs::remove_dir_all(&dir);
    }
}
