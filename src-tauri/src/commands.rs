use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{config, hotkeys::apply_config, AppState, Problem};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub ok: bool,
    pub error: Option<String>,
    pub problems: Vec<Problem>,
}

/// Set once at startup if `config::load_or_create` failed to read the
/// existing config.json -- see `AppState::config_load_error` in lib.rs.
/// `get_config` hands this to the settings window so it can never mistake
/// the empty startup fallback for the user's real, saved configuration
/// (C2): the frontend has nothing else that would tell them apart, since
/// an empty-but-successfully-loaded config and this fallback look
/// identical once they're just a `Config` value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLoadError {
    /// `ConfigError`'s `Display` text -- same text the log line uses.
    pub reason: String,
    /// Where the original, unreadable file was moved to (see
    /// `config::preserve_unreadable`), so the user can recover it by hand.
    /// `None` if there was nothing to preserve (fresh install) or the
    /// preserve attempt itself failed (logged separately either way).
    pub preserved_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigResult {
    pub config: config::Config,
    pub load_error: Option<ConfigLoadError>,
    /// R10: `Problem`s produced at startup (a hotkey the OS refused to
    /// register, an unparseable hotkey, ...) -- without this a hotkey that
    /// failed to register at launch was invisible in the UI until an
    /// unrelated Save happened to call `apply_config` again and produce a
    /// fresh problem list for *that* response.
    pub startup_problems: Vec<Problem>,
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> GetConfigResult {
    // R8: this used to map a poisoned lock to an `Err` string returned to
    // the frontend, which -- since nothing in this app ever clears a
    // poisoned `Mutex`'s poison flag -- would have meant every future
    // `get_config` call failing forever after one unrelated panic anywhere
    // else that happened to hold this same lock. Recover instead (see
    // `crate::recover_lock`'s doc comment): the config itself is never left
    // mid-mutation by a panic elsewhere.
    let config =
        crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone();
    let startup_problems =
        crate::recover_lock(state.startup_problems.lock(), "startup_problems", Some(&state.config_dir))
            .clone();
    GetConfigResult { config, load_error: state.config_load_error.clone(), startup_problems }
}

/// The validate-then-write half of `save_config`, pulled out as a pure
/// function of its arguments so the ordering is unit-testable without a
/// live `AppHandle`: this is the only version-testable Tauri
/// integration point where I attempted it (see `save_config_tests` below).
/// `apply_config` -- the OS-registration half, and the only thing that ever
/// mutates `state.config` -- genuinely needs an `AppHandle` and stays out
/// of this function entirely, which is itself the guarantee: a caller
/// holding an `Err` result has structurally no way to reach `state.config`,
/// because it never had a `state` to reach. `Err` means the config was
/// rejected before anything happened -- nothing was written to disk, and
/// `state.config` was never touched; `Ok` means it was validated *and*
/// durably written, ready for `apply_config` to take live.
fn decide_save(config: &config::Config, path: &Path) -> Result<config::Config, String> {
    // Validate through the same path the file loader uses, so the UI cannot
    // persist something a restart would then reject.
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let validated = config::parse(&json).map_err(|e| e.to_string())?;

    // `write_atomic`, not `fs::write`: this handler is the single
    // most-executed write path in the app (every settings-window Save goes
    // through it), so it needs the same rename-over guarantee the v1->v2
    // migration rewrite already relies on -- a failure partway through
    // (disk full, process killed, an AV/cloud-sync tool holding a handle)
    // must leave the previous config.json completely untouched rather than
    // truncated or empty.
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
            let problems = apply_config(&app, state.inner(), validated);
            SaveResult { ok: true, error: None, problems }
        }
    }
}

#[cfg(test)]
mod save_config_tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cai-test-save-config-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // 4b (spec Testing section): validate-then-apply ordering. A config
    // that is well-typed as `config::Config` but semantically invalid (a
    // command naming a backend that doesn't exist) must be rejected before
    // anything reaches disk -- the write in `decide_save` only ever runs on
    // the `Ready` branch.
    #[test]
    fn an_invalid_config_is_rejected_before_anything_is_written_to_disk() {
        let dir = temp_dir("rejected");
        let path = dir.join("config.json");
        let mut bad = config::default_config();
        bad.commands[0].backend_id = "does-not-exist".into();

        let decision = decide_save(&bad, &path);

        assert!(
            matches!(decision, Err(_)),
            "a command naming an unknown backend must be rejected"
        );
        assert!(!path.exists(), "a rejected config must never be written to disk");

        let _ = fs::remove_dir_all(&dir);
    }

    // 4c: the in-memory config invariant. `state.config` is only ever
    // mutated inside `apply_config` (see its doc comment in lib.rs), and
    // `save_config` only calls `apply_config` on the `Ready` branch -- so a
    // `Rejected` decision structurally cannot reach it. This test stands in
    // an `Arc<Mutex<Config>>` for `state.config` and confirms directly that
    // nothing here would have touched it.
    #[test]
    fn a_rejected_config_leaves_a_stand_in_for_the_in_memory_config_untouched() {
        let dir = temp_dir("in-memory");
        let path = dir.join("config.json");
        let state_config = std::sync::Arc::new(Mutex::new(config::default_config()));
        let original_commands = state_config.lock().unwrap().commands.len();

        let mut bad = config::default_config();
        bad.commands.push(bad.commands[0].clone()); // duplicate id -> rejected

        match decide_save(&bad, &path) {
            Err(_) => {
                // Mirrors save_config's own control flow: on `Err` it
                // returns immediately and never calls `apply_config`, the
                // only function that ever locks and overwrites
                // `state.config`.
            }
            Ok(_) => panic!("a duplicate id must be rejected"),
        }

        assert_eq!(
            state_config.lock().unwrap().commands.len(),
            original_commands,
            "a rejected save must never mutate the in-memory config"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // The other half of the ordering: a valid config really is written to
    // disk (so `apply_config` has a `Ready` value to work with), not just
    // accepted in memory.
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
    // R8: recover a poisoned lock instead of giving up on this call (and
    // every future one -- see `crate::recover_lock`'s doc comment) forever.
    let config =
        crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone();
    apply_config(&app, &state, config)
}

/// The single entry point the settings window's `CloseRequested` handler
/// calls, after `api.prevent_close()`, for every route that can request
/// closing it -- the X button, Alt+F4, the system menu's Close. Hides the
/// window and re-applies config, unconditionally. Re-applying keeps Task
/// 4's guarantee intact: recording a hotkey (Task 8) suspends global
/// shortcuts, and this is what restores them whenever the window closes.
///
/// Every mutation to the config now persists immediately and durably (see
/// `ConfigState`'s `commit` helper in `config.svelte.ts`) -- there is no
/// such thing as an unsaved edit by the time this fires, so there is
/// nothing left to ask the user about. This used to branch on a
/// frontend-reported dirty flag and, if set, block briefly for a
/// confirmation round trip before hiding regardless; that whole guard is
/// gone along with the state it existed to protect. Closing is just hiding
/// the window and re-applying whatever's already on disk.
///
/// `state` is optional so the hide half of this can never be held hostage
/// by the config-reapply half: `AppState` is `.manage()`d before any window
/// event can fire in practice, so `None` should be unreachable, but if it
/// somehow isn't, the window must still be able to close.
///
/// Emits `settings://window-hidden` right after the hide -- this, not
/// `document.visibilitychange` (WebView2 firing that on a native window
/// hide isn't guaranteed), is what tells the frontend to drop every piece
/// of transient UI state a background webview would otherwise carry
/// straight through the hide: a `HotkeyRecorder` stuck mid-recording (still
/// showing "Press a combination…" over hotkeys this same call just
/// re-registered), and a `BackendForm`/`CommandForm` dialog left open with
/// a stale draft. See `+page.svelte`'s listener and `configState.revision`.
///
/// R3: the config-reapply half is dispatched through
/// `AppHandle::run_on_main_thread` rather than calling `apply_config`
/// inline, and its result is logged from inside the dispatched closure
/// rather than returned. This is not a style choice -- `apply_config` ends
/// up calling the global-shortcut plugin's `unregister_all`, which (verified
/// against tauri-plugin-global-shortcut-2.3.2's `lib.rs:219-227`) locks its
/// internal `shortcuts` mutex *before* the blocking main-thread round-trip
/// and holds it across the whole wait -- unlike `unregister_multiple` a few
/// lines above it in that same file, which deliberately scopes the
/// round-trip before taking the lock. This function's only caller runs on
/// the main thread today (window events do), but an earlier version of it
/// also had a close-confirmation wait that ran on a *spawned* thread
/// instead -- calling `apply_config` from there directly held that lock
/// while blocked waiting for the main thread's event loop to service the
/// round-trip; if the main thread was ever itself blocked trying to
/// acquire that same lock (any other apply_config call reaching the plugin
/// -- a settings save, `resume_hotkeys`), neither side could make progress
/// and the app hung forever. `run_on_main_thread` sidesteps this on both
/// sides: called from the main thread it runs the closure inline with no
/// round-trip at all (tauri-runtime-wry's `send_user_message`, `lib.rs:239`,
/// special-cases the calling thread being the main thread), and called from
/// anywhere else it just queues the closure and returns immediately -- so
/// this caller, main thread or not, never blocks waiting for it. Keeping the
/// dispatch here (rather than inlining now that there's only one, main-
/// thread caller) means a future second caller off the main thread stays
/// safe by construction instead of by remembering to re-add this.
pub(crate) fn handle_close_request(app: AppHandle, state: Option<AppState>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.hide();
        let _ = app.emit("settings://window-hidden", ());
    }
    if let Some(state) = state {
        let app_for_task = app.clone();
        if let Err(e) = app.run_on_main_thread(move || {
            // R8: recover rather than silently skip the reapply on a
            // poisoned lock (see `crate::recover_lock`'s doc comment).
            let config =
                crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir))
                    .clone();
            // R7: this used to be `let _ = apply_config(...)`, silently
            // discarding every Problem a settings-window close produced
            // (an OS-registration failure, a stale unregister). Log them
            // the same way startup does, so they are at least
            // discoverable after the fact.
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
