use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Mutex,
};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
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
    if let Ok(mut guard) = ACK_TX.lock() {
        *guard = Some(tx);
    }
    rx
}

/// Called by the frontend's `settings://close-requested` listener as soon
/// as it has the confirmation dialog on screen -- see `ACK_TX`.
#[tauri::command]
pub fn ack_close_prompt() {
    if let Ok(guard) = ACK_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(());
        }
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
fn hide_and_reapply(app: &AppHandle, state: Option<&AppState>) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.hide();
    }
    if let Some(state) = state {
        if let Ok(config) = state.config.lock().map(|c| c.clone()) {
            let _ = apply_config(app, state, config);
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
