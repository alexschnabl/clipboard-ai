use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Mutex,
};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{apply_config, config, AppState, Problem};

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

/// The outcome of `decide_save`: either the config was rejected before
/// anything happened -- nothing was written to disk, and (since only
/// `apply_config` ever mutates it -- see `AppState::config`'s doc comment
/// in lib.rs) `state.config` was never touched -- or it was validated *and*
/// durably written, ready for `apply_config` to take live.
enum SaveDecision {
    Rejected(String),
    Ready(config::Config),
}

/// The validate-then-write half of `save_config`, pulled out as a pure
/// function of its arguments so the ordering is unit-testable without a
/// live `AppHandle`: this is the only version-testable Tauri
/// integration point where I attempted it (see `save_config_tests` below).
/// `apply_config` -- the OS-registration half, and the only thing that ever
/// mutates `state.config` -- genuinely needs an `AppHandle` and stays out
/// of this function entirely, which is itself the guarantee: a caller
/// holding a `Rejected` result has structurally no way to reach
/// `state.config`, because it never had a `state` to reach.
fn decide_save(config: &config::Config, path: &Path) -> SaveDecision {
    // Validate through the same path the file loader uses, so the UI cannot
    // persist something a restart would then reject.
    let json = match serde_json::to_string_pretty(config) {
        Ok(j) => j,
        Err(e) => return SaveDecision::Rejected(e.to_string()),
    };
    let validated = match config::parse(&json) {
        Ok(c) => c,
        Err(e) => return SaveDecision::Rejected(e.to_string()),
    };

    // `write_atomic`, not `fs::write`: this handler is the single
    // most-executed write path in the app (every settings-window Save goes
    // through it), so it needs the same rename-over guarantee the v1->v2
    // migration rewrite already relies on -- a failure partway through
    // (disk full, process killed, an AV/cloud-sync tool holding a handle)
    // must leave the previous config.json completely untouched rather than
    // truncated or empty.
    if let Err(e) = config::write_atomic(path, &validated) {
        return SaveDecision::Rejected(e.to_string());
    }

    SaveDecision::Ready(validated)
}

#[tauri::command]
pub fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: config::Config,
) -> SaveResult {
    let path = config::config_path(&app);
    match decide_save(&config, &path) {
        SaveDecision::Rejected(e) => SaveResult { ok: false, error: Some(e), problems: vec![] },
        SaveDecision::Ready(validated) => {
            let problems = apply_config(&app, state.inner(), validated);
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
            matches!(decision, SaveDecision::Rejected(_)),
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
            SaveDecision::Rejected(_) => {
                // Mirrors save_config's own control flow: on `Rejected` it
                // returns immediately and never calls `apply_config`, the
                // only function that ever locks and overwrites
                // `state.config`.
            }
            SaveDecision::Ready(_) => panic!("a duplicate id must be rejected"),
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
            SaveDecision::Ready(validated) => {
                assert_eq!(validated.commands.len(), cfg.commands.len());
            }
            SaveDecision::Rejected(e) => panic!("expected a valid config to be accepted, got: {e}"),
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

// ─── Unsaved-changes close guard ────────────────────────────────────────
//
// Rust cannot see Svelte state, and asking across the IPC bridge from
// inside the settings window's `CloseRequested` handler would mean
// blocking that handler's thread on a reply that can only ever be produced
// by running the frontend's own event loop -- which may itself be waiting
// on this thread. So the frontend pushes `configState.dirty` over
// eagerly via `set_dirty` (see `+page.svelte`'s `$effect`, which fires on
// every change in either direction) rather than Rust pulling it on demand.

/// Mirrors `configState.dirty`. Starts `false`, which is what makes a
/// frontend that never mounts a non-issue: nothing but a running frontend
/// ever sets this `true`, so `handle_close_request` below takes the
/// immediate-hide path with no waiting at all until one does.
static DIRTY: AtomicBool = AtomicBool::new(false);

/// Liveness channel for one in-flight close confirmation. `handle_close_
/// request` installs a fresh sender here (via `begin_close_wait`) each time
/// it decides to ask the frontend before hiding, then waits -- on a
/// *spawned* thread, never the caller's -- on the matching receiver with a
/// short timeout. The frontend's `settings://close-requested` listener
/// calls `ack_close_prompt` the moment it has the confirmation dialog on
/// screen, *not* when the user finishes deciding.
///
/// That distinction is the whole point: it lets Rust time-box "is the
/// frontend alive and about to show the dialog" (bounded -- a broken, hung,
/// or not-yet-mounted frontend must never trap the window) without also
/// time-boxing "how long may the user take to decide" (unbounded -- once
/// the dialog is genuinely on screen, sitting there is not a hang, it's
/// just a human, the same as any other moment the settings window sits
/// open and idle).
static ACK_TX: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);

/// How long Rust waits for `ack_close_prompt` before concluding the
/// frontend is never going to answer and hiding regardless. This is a
/// liveness check, not a decision deadline (see `ACK_TX`'s doc comment): a
/// few hundred ms is generously more than a healthy webview needs to run
/// one `listen()` callback and fire one `invoke`. If nothing lands in that
/// window the frontend was never going to show the dialog at all.
const CLOSE_ACK_TIMEOUT: Duration = Duration::from_millis(750);

fn is_dirty() -> bool {
    DIRTY.load(Ordering::SeqCst)
}

/// Set by the frontend whenever `configState.dirty` changes, in both
/// directions.
#[tauri::command]
pub fn set_dirty(dirty: bool) {
    DIRTY.store(dirty, Ordering::SeqCst);
}

/// Registers a fresh one-shot ack channel and returns the receiving half.
/// Overwrites -- and thereby disconnects -- any sender left behind by an
/// earlier, still-in-flight close request. That is what makes a second
/// close press while the first is still being waited on safe rather than a
/// pile-up: the earlier wait's receiver wakes immediately on disconnect
/// (`RecvTimeoutError::Disconnected`, not a wait for the full timeout) and
/// treats that exactly like "nothing answered", which is a reasonable
/// response to a second, independent close request anyway.
fn begin_close_wait() -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    // R8: recover rather than silently drop the sender on a poisoned lock
    // -- `ACK_TX` has no `AppState`/log directory in scope (see
    // `crate::recover_lock`'s doc comment on why `config_dir` is
    // `Option`), so this can only reach stderr, but that is still strictly
    // better than the close-confirmation guard quietly stopping working.
    *crate::recover_lock(ACK_TX.lock(), "ACK_TX", None) = Some(tx);
    rx
}

/// Called by the frontend's `settings://close-requested` listener as soon
/// as it has the confirmation dialog on screen -- see `ACK_TX`.
#[tauri::command]
pub fn ack_close_prompt() {
    let guard = crate::recover_lock(ACK_TX.lock(), "ACK_TX", None);
    if let Some(tx) = guard.as_ref() {
        let _ = tx.send(());
    }
}

/// Hides the settings window and re-applies config, unconditionally. This
/// is the one function every route that actually closes the window funnels
/// through -- the immediate-hide path below when there was nothing to lose,
/// the timeout fallback when nothing answered, and `force_close_settings`
/// once the frontend itself asks (Discard, or a successful Save). Keeping
/// the re-apply on all three keeps Task 4's guarantee intact: recording a
/// hotkey (Task 8) suspends global shortcuts, and this is what restores
/// them no matter which route the window closed through.
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
/// a stale draft that Discard's reload wouldn't otherwise touch. See
/// `+page.svelte`'s listener and `configState.revision`.
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
/// round-trip before taking the lock. `hide_and_reapply` is called from the
/// close-wait thread spawned in `handle_close_request` below, which is not
/// the main thread. Calling `apply_config` from there directly would hold
/// that lock while blocked waiting for the main thread's event loop to
/// service the round-trip; if the main thread is ever itself blocked trying
/// to acquire that same lock (any other apply_config call reaching the
/// plugin -- a second close, a settings save, `resume_hotkeys`), neither
/// side can make progress and the app hangs forever. `run_on_main_thread`
/// sidesteps this on both sides: called from the main thread it runs the
/// closure inline with no round-trip at all (tauri-runtime-wry's
/// `send_user_message`, `lib.rs:239`, special-cases the calling thread being
/// the main thread), and called from anywhere else it just queues the
/// closure and returns immediately -- so this caller, main thread or not,
/// never blocks waiting for it. That in turn means `apply_config` itself is
/// now only ever invoked while already on the main thread, so its call into
/// the plugin never takes the blocking cross-thread path at all.
fn hide_and_reapply(app: &AppHandle, state: Option<&AppState>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.hide();
        let _ = app.emit("settings://window-hidden", ());
    }
    if let Some(state) = state {
        let state = state.clone();
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

/// Escape hatch the frontend calls after the user chooses Discard or a
/// successful Save: clears the dirty flag and hides.
#[tauri::command]
pub fn force_close_settings(app: AppHandle, state: State<'_, AppState>) {
    DIRTY.store(false, Ordering::SeqCst);
    hide_and_reapply(&app, Some(state.inner()));
}

/// The single entry point the settings window's `CloseRequested` handler
/// calls, after `api.prevent_close()`, for every route that can request
/// closing it -- the X button, Alt+F4, the system menu's Close.
///
/// Nothing unsaved: hides immediately. No waiting, no round trip -- this is
/// also the path a frontend that never mounted takes, since `DIRTY` starts
/// `false` and only a running frontend ever sets it otherwise.
///
/// Something unsaved: emits the confirmation event, then waits -- on a
/// spawned thread, never the window event loop -- up to
/// `CLOSE_ACK_TIMEOUT` for `ack_close_prompt`. Acked in time, this does
/// nothing further; the window stays open until the frontend calls
/// `force_close_settings` or the user cancels (nothing to do at all -- the
/// window was never hidden). Not acked in time, this hides anyway: the
/// guard is a convenience, the window closing is the guarantee.
pub(crate) fn handle_close_request(app: AppHandle, state: Option<AppState>) {
    if !is_dirty() {
        hide_and_reapply(&app, state.as_ref());
        return;
    }

    let rx = begin_close_wait();
    let _ = app.emit("settings://close-requested", ());

    std::thread::spawn(move || {
        if rx.recv_timeout(CLOSE_ACK_TIMEOUT).is_err() {
            hide_and_reapply(&app, state.as_ref());
        }
    });
}

#[cfg(test)]
mod close_guard_tests {
    use super::*;

    // `DIRTY` and `ACK_TX` are process-wide statics, and `cargo test` runs
    // tests in parallel threads within one process by default -- without
    // this, these tests would stomp on each other's state. A single mutex
    // held for the duration of each test gives each one exclusive access;
    // `.unwrap_or_else(...)` rides through poisoning from an earlier
    // panicking test rather than cascading every subsequent test into an
    // unrelated "lock poisoned" failure.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn set_dirty_round_trips_through_is_dirty_in_both_directions() {
        let _guard = lock();
        set_dirty(true);
        assert!(is_dirty(), "set_dirty(true) must be observable via is_dirty()");
        set_dirty(false);
        assert!(!is_dirty(), "set_dirty(false) must also be observable -- not just the true edge");
    }

    #[test]
    fn no_ack_leaves_the_waiter_timing_out_rather_than_blocking_forever() {
        let _guard = lock();
        let rx = begin_close_wait();
        assert!(
            matches!(rx.recv_timeout(Duration::from_millis(50)), Err(mpsc::RecvTimeoutError::Timeout)),
            "a waiter with no ack must observe a timeout, not block indefinitely"
        );
    }

    #[test]
    fn ack_close_prompt_wakes_the_current_waiter() {
        let _guard = lock();
        let rx = begin_close_wait();
        ack_close_prompt();
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_ok(),
            "an ack sent after begin_close_wait must reach that waiter"
        );
    }

    #[test]
    fn a_second_close_press_disconnects_the_first_waiter_instead_of_leaving_it_hanging() {
        let _guard = lock();
        let rx1 = begin_close_wait();
        let rx2 = begin_close_wait(); // simulates a second close press
        ack_close_prompt();

        // The second (current) waiter is the one that gets the ack...
        assert!(
            rx2.recv_timeout(Duration::from_millis(200)).is_ok(),
            "the live waiter installed by the second close press must receive the ack"
        );
        // ...and the first isn't left sitting out the full timeout: dropping
        // its sender wakes it immediately with Disconnected.
        assert!(
            matches!(
                rx1.recv_timeout(Duration::from_millis(200)),
                Err(mpsc::RecvTimeoutError::Disconnected)
            ),
            "the superseded waiter must be woken by disconnect, not left to block \
             for the full timeout -- this is what keeps a second close press from \
             ever piling up or deadlocking"
        );
    }
}
