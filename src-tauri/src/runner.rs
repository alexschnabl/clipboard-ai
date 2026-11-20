use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::config::Formatting;
use crate::cycle::{self, Captured};
use crate::hud::{HudView, Look, REASON_NO_LAST_RESULT};
use crate::overlay::{self, Anchor};
use crate::{backend, recover_lock, status, tray_anim, AppState, BusyGuard};

pub const PALETTE_INSTRUCTION_ID: &str = "palette-instruction";

const TRAY_MENU_SETTLE_MS: u64 = 150;

pub struct Job {
    pub command_id: String,
    pub command_name: String,
    pub backend: Arc<dyn backend::Backend>,
    pub system_prompt: String,
    pub formatting: Formatting,
}

fn log(state: &AppState, line: &str) {
    println!("{line}");
    status::append_log(&state.config_dir, line);
}

pub fn spawn_guarded<F>(state: &AppState, what: &'static str, work: F)
where
    F: FnOnce(&AppState) + Send + 'static,
{
    let state = state.clone();
    std::thread::spawn(move || {
        let _busy_guard = BusyGuard(&state.busy);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&state)));
        if let Err(payload) = result {
            let message = crate::panic_message(&payload);
            let line = format!("PANIC {what} panicked: {message}");
            eprintln!("{line}");
            status::append_log(&state.config_dir, &line);
            crate::set_status(&state, "internal error \u{2014} see log".to_string());
        }
    });
}

pub fn request_cancel(state: &AppState) {
    if let Some(cancel) = recover_lock(state.cancel.lock(), "cancel", Some(&state.config_dir)).as_ref() {
        cancel.store(true, Ordering::SeqCst);
    }
}

fn settings_snapshot(state: &AppState) -> crate::config::Settings {
    recover_lock(state.config.lock(), "config", Some(&state.config_dir)).settings.clone()
}

pub fn run_job(
    app: &AppHandle,
    state: &AppState,
    job: Job,
    captured: Option<Captured>,
    anchor: Option<Anchor>,
) {
    let settings = settings_snapshot(state);
    let look = Look::from_settings(&settings);

    let cancel = Arc::new(AtomicBool::new(false));
    *recover_lock(state.cancel.lock(), "cancel", Some(&state.config_dir)) = Some(cancel.clone());
    let escape_registered = look.enabled && app.global_shortcut().register(state.escape).is_ok();
    let mut cleanup = RunCleanup { app, state, escape: escape_registered, finished: false };

    if look.enabled {
        let anchor = anchor.unwrap_or_else(overlay::caret_anchor);
        state.hud.begin(
            anchor,
            HudView::Working {
                command: job.command_name.clone(),
                backend: job.backend.name().to_string(),
                cancellable: escape_registered,
            },
            look,
        );
    }

    let indicator =
        tray_anim::TrayAnimation::begin(app.clone(), state.config_dir.clone(), settings.busy_cursor);
    let cycle_start = state.ops.now_ms();
    let outcome = match &captured {
        None => cycle::run_cancellable(
            &job.backend,
            &job.system_prompt,
            job.formatting,
            state.ops.as_ref(),
            &cancel,
        ),
        Some(captured) => cycle::run_on_captured(
            &job.backend,
            &job.system_prompt,
            job.formatting,
            state.ops.as_ref(),
            captured,
            &cancel,
        ),
    };
    let total_cycle_ms = state.ops.now_ms().saturating_sub(cycle_start);
    indicator.finish();

    cleanup.release();

    let line = status::log_line(&outcome, &job.command_name, job.backend.name());
    log(state, &line);
    crate::set_status(state, status::summary(&outcome, &job.command_name));
    crate::record_history(
        state,
        &job.command_id,
        &job.command_name,
        job.backend.name(),
        &outcome,
        total_cycle_ms,
    );
    if let Some(clip) = outcome.result_clip() {
        *recover_lock(state.last_result.lock(), "last_result", Some(&state.config_dir)) = Some(clip);
    }

    state.hud.finish(HudView::from_outcome(&outcome), look);
    cleanup.finished = true;
}

struct RunCleanup<'a> {
    app: &'a AppHandle,
    state: &'a AppState,
    escape: bool,
    finished: bool,
}

impl RunCleanup<'_> {
    fn release(&mut self) {
        *recover_lock(self.state.cancel.lock(), "cancel", Some(&self.state.config_dir)) = None;
        if std::mem::take(&mut self.escape) {
            let _ = self.app.global_shortcut().unregister(self.state.escape);
        }
    }
}

impl Drop for RunCleanup<'_> {
    fn drop(&mut self) {
        self.release();
        if !self.finished {
            self.state.hud.dismiss();
        }
    }
}

pub fn reregister_escape_if_running(app: &AppHandle, state: &AppState) {
    let running = recover_lock(state.cancel.lock(), "cancel", Some(&state.config_dir)).is_some();
    if running && settings_snapshot(state).hud {
        let _ = app.global_shortcut().register(state.escape);
    }
}

pub fn paste_last(state: &AppState, from_tray: bool) {
    if state.busy.swap(true, Ordering::SeqCst) {
        log(state, "busy; paste-again ignored");
        return;
    }
    spawn_guarded(state, "paste-again", move |state| {
        let look = Look::from_settings(&settings_snapshot(state));
        let last = recover_lock(state.last_result.lock(), "last_result", Some(&state.config_dir)).clone();
        let Some(clip) = last else {
            log(state, "PASTE-AGAIN nothing to paste yet");
            state.hud.notice(
                overlay::caret_anchor(),
                HudView::Stopped { reason: REASON_NO_LAST_RESULT.into() },
                look,
            );
            return;
        };

        let expected = if from_tray {
            std::thread::sleep(Duration::from_millis(TRAY_MENU_SETTLE_MS));
            let target = overlay::last_app_window();
            if !state.ops.activate_window(target) {
                log(state, "PASTE-AGAIN could not bring the last window back to the front");
                state.hud.notice(
                    overlay::caret_anchor(),
                    HudView::Stopped { reason: cycle::REASON_FOCUS_RESTORE_FAILED.into() },
                    look,
                );
                return;
            }
            Some(target)
        } else {
            None
        };

        let anchor = if look.enabled { Some(overlay::caret_anchor()) } else { None };
        let outcome = cycle::paste_clip(state.ops.as_ref(), &clip, expected);
        let verdict = if outcome.pasted { "PASTED" } else { "ABORTED" };
        log(state, &format!("PASTE-AGAIN {verdict} reason={}", outcome.reason));

        if let Some(anchor) = anchor {
            let view = if outcome.pasted {
                HudView::Done { words_changed: None, sent: false, again: true }
            } else {
                HudView::Stopped { reason: outcome.reason.to_string() }
            };
            state.hud.notice(anchor, view, look);
        }
    });
}
