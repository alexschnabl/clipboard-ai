//! In-flight indicator: a small, cursor-anchored, always-on-top overlay
//! window that tells the user a hotkey cycle is running. See
//! `docs/superpowers/notes/2026-08-07-concept-*.md` for the research this
//! was built from -- cursor anchoring specifically exists to sidestep the
//! "wrong monitor" bug class documented there across half a dozen
//! comparable tools (Alfred, Espanso, CopyQ, ShareX, Flameshot).
//!
//! # The window
//!
//! The `overlay` window is declared once, hidden, in `tauri.conf.json` --
//! not built here in code. Tauri's own startup (`app.rs::setup`,
//! source-verified against the installed tauri 2.11.5) creates every
//! `windows[]` entry with `create: true` (the default) *before* the app's
//! own `.setup()` closure runs, via
//! `WebviewWindowBuilder::from_config(...).build()` on the main thread
//! during normal startup -- not from a command or event handler, so the
//! documented Windows deadlock on `build()`
//! (`webview/mod.rs`'s "Known issues": "this function deadlocks when used
//! in a synchronous command or event handlers") never applies. This module
//! never calls `build()`; it only ever looks up the pre-existing window by
//! label (`get_webview_window`), and shows, hides or repositions it -- a
//! few Win32 calls each, ~16-30ms total per the tech note, comfortably
//! inside even a 400ms cycle. A window built fresh per cycle instead would
//! cost ~100ms+ (a new WebView2 controller/HWND) and eat a quarter of that
//! budget on its own.
//!
//! Transparency is real, not cosmetic-only: wry 0.55.1 sets the WebView2
//! controller's background to `(0, 0, 0, 0)` automatically whenever
//! `transparent: true` is set (`webview2/mod.rs`, `is_windows_7()`-gated,
//! irrelevant on any OS this app targets). What still has to be handled on
//! the frontend side is the page's own content painting an opaque color
//! over that transparent surface -- see `src/routes/overlay/+layout.svelte`.
//!
//! # Contract with the cycle
//!
//! `cycle.rs` and its caller are not this module's to edit; this is the
//! interface whoever wires them up must use. Exactly two calls wrap
//! `cycle::run`, mirroring `cycle::ClipboardGuard`'s own guard-plus-Drop
//! shape:
//!
//! ```ignore
//! let overlay = overlay::CycleOverlay::begin(app_handle.clone());
//! let outcome = cycle::run(backend, system_prompt, ops);
//! overlay.finish(overlay::Outcome::from_pasted(outcome.pasted));
//! ```
//!
//! `begin` arms a delay-gated, self-escalating timer and returns a guard.
//! There is no third "progress" call to make mid-cycle: escalation (quiet
//! -> stronger) runs entirely off `begin`'s own background timer, because
//! `cycle::run` has no natural checkpoint to report progress from while
//! blocked inside a capture wait or a backend call -- driving it from a
//! clock this module owns, rather than asking the cycle to remember a
//! second call, keeps the promised surface to "a guard and one call."
//!
//! `finish` reports how the cycle ended and is the only call that varies.
//! If it is never reached -- an early return added upstream that forgets
//! it, or, most importantly, a panic unwinding out of `cycle::run` --
//! `CycleOverlay`'s `Drop` still runs and still hides the window
//! unconditionally. There is no `disarm`, exactly as `ClipboardGuard` has
//! none: making "stop showing" a property of scope rather than a step on
//! each exit path removes the entire class of "this abort forgot to clear
//! the indicator" bug.
//!
//! # Frontend contract
//!
//! Every state change is one `emit_to(LABEL, "overlay://state", { state })`
//! call, `state` one of `"working" | "escalated" | "failure" | "hidden"`.
//! `src/routes/overlay/+page.svelte` renders purely from that event stream
//! and starts, and always eventually settles, on nothing visible -- see its
//! own doc comment and `src/routes/overlay/state.ts`.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

/// The label the statically-declared window in `tauri.conf.json` uses.
/// Another agent's capability change (`capabilities/default.json`) already
/// assumes this exact string.
pub const LABEL: &str = "overlay";

const EVENT: &str = "overlay://state";

/// Below this, a cycle is indistinguishable from instant. The spike
/// measured a warm local model at ~400ms; showing anything before this
/// gate elapses would arrive after the paste is already done -- a flash,
/// not feedback.
const SHOW_DELAY_MS: u64 = 250;
/// Total elapsed time from `begin()` before the indicator escalates to its
/// stronger state. Set above the spike's ~2.1s Claude-session band so that
/// path still reads as "normal", not "stuck".
const ESCALATE_AFTER_MS: u64 = 2500;
/// How long the failure glyph stays up before this module clears it
/// itself. `state.ts` on the frontend carries a second, longer fallback in
/// case this timer's own `hide_now` call is ever lost (e.g. the spawned
/// thread never getting scheduled before process exit).
const FAILURE_VISIBLE_MS: u64 = 1500;
/// Granularity of the cancellation check in the escalation thread -- bounds
/// how late a fast `finish()` can still suppress an about-to-fire show or
/// escalation.
const POLL_MS: u64 = 40;

/// Placed this far below-right of the raw cursor point (in physical
/// pixels) so the glyph doesn't render directly under the pointer
/// hot-spot.
const OFFSET_X: i32 = 18;
const OFFSET_Y: i32 = 24;

/// Fallback used only if `outer_size()` itself fails -- matches the
/// `width`/`height` declared for the `overlay` window in `tauri.conf.json`.
const FALLBACK_SIZE: (u32, u32) = (56, 56);

#[derive(Serialize, Clone)]
struct StatePayload {
    state: &'static str,
}

/// A dropped event is not fatal here: the frontend's own watchdog
/// (`state.ts`) and the fact that the *window* being hidden is the real
/// guarantee (see this module's doc comment) both cover for it. Matches
/// `set_status`'s "never let a UI update failure take down the cycle"
/// stance in `lib.rs`.
fn emit_state(app: &AppHandle, state: &'static str) {
    let _ = app.emit_to(LABEL, EVENT, StatePayload { state });
}

/// Hides the window and tells the frontend so, unconditionally. The single
/// choke point every "stop showing" path (success, the failure timer,
/// `Drop`, and the next cycle's defensive reset in `begin`) funnels
/// through, so there is exactly one place that has to get "actually hide
/// it" right.
fn hide_now(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
    emit_state(app, "hidden");
}

/// Clamps `(x, y)` -- a window's candidate top-left corner -- so a window
/// of size `(w, h)` stays inside the rect `(area_x, area_y, area_w,
/// area_h)`. Pure and unit-tested on purpose: this is the one piece of
/// overlay math with a documented history of getting shipped wrong
/// (Alfred, Espanso, CopyQ, ShareX and Flameshot all shipped a "wrong
/// monitor" bug -- see the prior-art note) and it needs no `AppHandle` to
/// verify, including the negative-origin case a monitor to the left of or
/// above the primary produces on Windows' virtual desktop.
fn clamp_to_area(x: i32, y: i32, w: i32, h: i32, area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (area_x, area_y, area_w, area_h) = area;
    // `.max(area_x)`/`.max(area_y)`: if the window is wider/taller than the
    // area, prefer aligning to the near edge over reporting a max below the
    // min, which would panic `i32::clamp` and be nonsensical besides.
    let max_x = (area_x + area_w - w).max(area_x);
    let max_y = (area_y + area_h - h).max(area_y);
    (x.clamp(area_x, max_x), y.clamp(area_y, max_y))
}

/// Positions the (already-hidden or about-to-show) window at the current
/// cursor, clamped to whichever monitor the cursor is actually on. Cursor
/// coordinates are already a single virtual-desktop space on Windows -- no
/// per-monitor origin math is needed to get this right; anchoring here
/// structurally avoids the wrong-monitor bug class rather than
/// special-casing it.
fn position_near_cursor(app: &AppHandle, window: &tauri::WebviewWindow) {
    let Ok(cursor) = app.cursor_position() else { return };
    let size = window.outer_size().unwrap_or(PhysicalSize::new(FALLBACK_SIZE.0, FALLBACK_SIZE.1));

    let mut x = cursor.x as i32 + OFFSET_X;
    let mut y = cursor.y as i32 + OFFSET_Y;

    if let Ok(Some(monitor)) = app.monitor_from_point(cursor.x, cursor.y) {
        let area = monitor.work_area();
        let (cx, cy) = clamp_to_area(
            x,
            y,
            size.width as i32,
            size.height as i32,
            (area.position.x, area.position.y, area.size.width as i32, area.size.height as i32),
        );
        x = cx;
        y = cy;
    }
    // A failed reposition still lets `show()` proceed at whatever position
    // the window last had -- offscreen-but-visible is a smaller failure
    // than never showing feedback at all.
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn show(app: &AppHandle, state: &'static str) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    // Emit before positioning/showing: the frontend has already re-rendered
    // for the new state by the time there's anything to see, on a webview
    // that's been sitting warm since startup rather than one just spinning
    // up.
    emit_state(app, state);
    position_near_cursor(app, &window);
    let _ = window.show();
}

/// What the frontend should end up showing once `finish` is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The cycle pasted. No failure glyph -- the pasted text is its own
    /// confirmation -- just hide.
    Success,
    /// The cycle aborted or the backend failed. Shown as a distinct glyph
    /// for `FAILURE_VISIBLE_MS`, then cleared automatically.
    Failure,
}

impl Outcome {
    /// The one place `cycle::CycleOutcome.pasted` (a plain `bool`, not this
    /// module's own type) turns into an `Outcome`. Takes a `bool` rather
    /// than `cycle::CycleOutcome` directly so this module has no
    /// compile-time dependency on a struct it isn't allowed to edit.
    pub fn from_pasted(pasted: bool) -> Self {
        if pasted { Outcome::Success } else { Outcome::Failure }
    }
}

/// A per-cycle guard. See the module doc comment for the full contract.
pub struct CycleOverlay {
    app: AppHandle,
    /// Flipped by `finish` (to stop the escalation thread firing into a
    /// cycle that's already done) and read by that thread between every
    /// sleep tick.
    cancel: Arc<AtomicBool>,
    /// Set by `finish`; read by `Drop` to decide whether it still owes the
    /// window a hide -- see `Drop`'s doc comment.
    finished: bool,
}

impl CycleOverlay {
    /// Starts a cycle. Defensively hides any indicator left over from a
    /// previous cycle first -- the bound on staleness if some earlier
    /// cycle's own failure-clear thread never got to run -- and then arms
    /// the delay-gated escalation timer.
    pub fn begin(app: AppHandle) -> Self {
        hide_now(&app);

        let cancel = Arc::new(AtomicBool::new(false));
        let thread_app = app.clone();
        let thread_cancel = cancel.clone();
        std::thread::spawn(move || run_escalation(thread_app, thread_cancel));

        CycleOverlay { app, cancel, finished: false }
    }

    /// Reports how the cycle ended. Consumes `self`: there is exactly one
    /// legal call, and the compiler enforces that rather than a runtime
    /// check.
    pub fn finish(mut self, outcome: Outcome) {
        self.cancel.store(true, Ordering::SeqCst);
        self.finished = true;
        match outcome {
            Outcome::Success => hide_now(&self.app),
            Outcome::Failure => {
                show(&self.app, "failure");
                let app = self.app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(FAILURE_VISIBLE_MS));
                    hide_now(&app);
                });
            }
        }
    }
}

impl Drop for CycleOverlay {
    fn drop(&mut self) {
        // Stop the escalation thread from showing/escalating into a cycle
        // that has already ended one way or another, `finish`ed or not.
        self.cancel.store(true, Ordering::SeqCst);
        // `finish` already decided the right terminal state (hidden, or a
        // failure flash that hides itself on its own timer) and that
        // decision must not be second-guessed here. This branch exists for
        // every path that *isn't* that: an early return added upstream
        // that never reaches `finish`, or a panic unwinding straight out
        // of `cycle::run` -- see the module doc comment. Either way,
        // something has already stopped running; a "working" spinner must
        // not still be on screen for it.
        if !self.finished {
            hide_now(&self.app);
        }
    }
}

/// Runs on its own thread for the lifetime of one cycle. Polls `cancel`
/// rather than sleeping through the full delay in one call so a fast
/// `finish()` can suppress an about-to-fire show/escalation within
/// `POLL_MS`, not just before the wait starts.
fn run_escalation(app: AppHandle, cancel: Arc<AtomicBool>) {
    if !wait_or_cancelled(&cancel, SHOW_DELAY_MS) {
        return;
    }
    show(&app, "working");

    if !wait_or_cancelled(&cancel, ESCALATE_AFTER_MS - SHOW_DELAY_MS) {
        return;
    }
    show(&app, "escalated");
}

/// Sleeps up to `total_ms`, checking `cancel` every `POLL_MS`. Returns
/// `false` the moment `cancel` is observed set (including if it was already
/// set before the first sleep), `true` if `total_ms` elapsed without it.
fn wait_or_cancelled(cancel: &AtomicBool, total_ms: u64) -> bool {
    let mut waited = 0;
    while waited < total_ms {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let step = POLL_MS.min(total_ms - waited);
        std::thread::sleep(Duration::from_millis(step));
        waited += step;
    }
    !cancel.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_leaves_a_position_that_already_fits_untouched() {
        assert_eq!(clamp_to_area(100, 100, 56, 56, (0, 0, 1920, 1080)), (100, 100));
    }

    #[test]
    fn clamp_pulls_back_from_the_right_edge() {
        assert_eq!(clamp_to_area(1900, 100, 56, 56, (0, 0, 1920, 1080)), (1864, 100));
    }

    #[test]
    fn clamp_pulls_back_from_the_bottom_edge() {
        assert_eq!(clamp_to_area(100, 1070, 56, 56, (0, 0, 1920, 1080)), (100, 1024));
    }

    #[test]
    fn clamp_pulls_back_from_the_top_left_edge_for_a_negative_offset() {
        // A hypothetical near-corner cursor with a placement offset that
        // would put the window off the top-left of its own monitor.
        assert_eq!(clamp_to_area(-10, -10, 56, 56, (0, 0, 1920, 1080)), (0, 0));
    }

    #[test]
    fn clamp_handles_a_monitor_left_of_the_primary_with_a_negative_origin() {
        // Windows' virtual desktop places a secondary monitor to the left
        // of the primary at negative x -- exactly the case a naive
        // `.max(0)` clamp (rather than clamping to the monitor's own
        // origin) gets wrong, reproducing the "wrong monitor" bug class the
        // prior-art note documents across half a dozen comparable tools.
        assert_eq!(clamp_to_area(-1900, 100, 56, 56, (-1920, 0, 1920, 1080)), (-1900, 100));
        assert_eq!(clamp_to_area(-10, 100, 56, 56, (-1920, 0, 1920, 1080)), (-56, 100));
    }

    #[test]
    fn clamp_prefers_the_near_edge_when_the_window_is_wider_than_the_area() {
        // Degenerate but must not panic (`i32::clamp` panics if min > max)
        // and must not report a position past the area's own left edge.
        assert_eq!(clamp_to_area(500, 0, 3000, 56, (0, 0, 1920, 1080)), (0, 0));
    }

    #[test]
    fn outcome_from_pasted_maps_true_to_success_and_false_to_failure() {
        assert_eq!(Outcome::from_pasted(true), Outcome::Success);
        assert_eq!(Outcome::from_pasted(false), Outcome::Failure);
    }

    #[test]
    fn wait_or_cancelled_returns_false_immediately_if_already_cancelled() {
        let cancel = AtomicBool::new(true);
        assert!(!wait_or_cancelled(&cancel, 250));
    }

    #[test]
    fn wait_or_cancelled_returns_true_once_the_full_duration_elapses_uncancelled() {
        let cancel = AtomicBool::new(false);
        assert!(wait_or_cancelled(&cancel, 20));
    }

    #[test]
    fn wait_or_cancelled_returns_false_if_cancelled_partway_through() {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(15));
            flag.store(true, Ordering::SeqCst);
        });
        assert!(!wait_or_cancelled(&cancel, 500));
    }
}
