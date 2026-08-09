//! In-flight feedback via the tray icon.
//!
//! Replaces the earlier cursor-anchored overlay window. That approach had
//! two problems inherent to "move a separate always-on-top window to follow
//! the pointer": it lagged behind fast cursor motion (a window repositioned
//! through the message pump can never be glued to the hardware cursor), and
//! its placement was a DPI/physical-vs-logical rabbit hole. Animating the
//! tray icon sidesteps both by construction -- there is no position to get
//! wrong and nothing chasing anything. The cost is that the feedback lives
//! in the corner rather than next to the user's text; that trade was the
//! author's call.
//!
//! # Shape
//!
//! One [`TrayAnimation`] guard wraps `cycle::run`, exactly as
//! `cycle::ClipboardGuard` and the old overlay did:
//!
//! ```ignore
//! let anim = tray_anim::TrayAnimation::begin(app.clone());
//! let outcome = cycle::run(...);
//! anim.finish(); // stop the animation, restore the idle icon
//! ```
//!
//! `begin` spawns a delay-gated timer thread that breathes the icon's
//! opacity while the cycle runs. `finish` (and `Drop`, if `finish` is never
//! reached -- an early return or a panic unwinding out of `cycle::run`)
//! cancels that thread and restores the idle icon. There is no disarm: the
//! restore is a property of scope, so no exit path can leave the tray stuck
//! breathing. This mirrors the discipline the whole cycle engine is built
//! on.
//!
//! No cross-cycle machinery (the generation counter the overlay needed) is
//! required here: `lib.rs`'s `busy` re-entrancy flag already serialises the
//! hotkey handler so exactly one cycle -- and therefore one `TrayAnimation`
//! -- can exist at a time.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};
use std::thread::JoinHandle;
use std::time::Duration;

use tauri::image::Image;
use tauri::AppHandle;

/// Must match the id given to the tray in `lib.rs`'s `TrayIconBuilder`. The
/// animation thread reaches the (single) tray icon by this id rather than
/// holding a `TrayIcon` handle, so nothing new has to be threaded onto
/// `AppState`.
pub const TRAY_ID: &str = "clipboard-ai-tray";

/// Below this, a cycle is indistinguishable from instant (the spike measured
/// a warm local model at ~400 ms). Waiting this long before touching the
/// icon keeps a fast cycle -- or a burst of them -- from flickering the tray.
const SHOW_DELAY_MS: u64 = 250;

/// Time between animation frames. ~11 fps: a calm breath, not a strobe, and
/// far under any rate where the shell would coalesce updates.
const FRAME_INTERVAL_MS: u64 = 90;

/// How finely the cancellable sleeps below poll the cancel flag -- bounds
/// how long after `finish()` the thread can still be mid-sleep before it
/// notices and restores the idle icon.
const POLL_MS: u64 = 30;

/// The icon breathes between full opacity and this fraction of it. Kept well
/// above zero so the glyph dims rather than vanishing -- a pulse, not a
/// blink.
const MIN_ALPHA_SCALE: f64 = 0.4;

/// Frames in one breath cycle. A full loop is `FRAME_COUNT *
/// FRAME_INTERVAL_MS` ~= 1.3 s.
const FRAME_COUNT: usize = 14;

/// The idle icon and the pre-built breathing frames, computed once from the
/// app's own window icon and reused for every cycle. `None` if the app has
/// no window icon (the tray then simply shows nothing to animate -- see
/// `lib.rs`, where a missing icon is already a tolerated startup state).
struct Frames {
    idle: Image<'static>,
    busy: Vec<Image<'static>>,
}

static FRAMES: OnceLock<Option<Frames>> = OnceLock::new();

/// Scales one frame's alpha by `scale`, cloning the RGBA and multiplying
/// every 4th byte. Pure so it can be unit-tested without a tray.
fn scale_alpha(rgba: &[u8], scale: f64) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let mut i = 3;
    while i < out.len() {
        out[i] = (out[i] as f64 * scale).round().clamp(0.0, 255.0) as u8;
        i += 4;
    }
    out
}

/// The alpha multiplier for frame `i`: a cosine breath from 1.0 down to
/// `MIN_ALPHA_SCALE` and back, so the loop is seamless. Pure/unit-tested.
fn breath_scale(i: usize, count: usize) -> f64 {
    let phase = (i as f64 / count as f64) * std::f64::consts::TAU;
    // cos runs 1 -> -1 -> 1 over a full turn; remap to [MIN, 1].
    let unit = 0.5 * (1.0 + phase.cos()); // 1 at i=0, 0 at the midpoint
    MIN_ALPHA_SCALE + (1.0 - MIN_ALPHA_SCALE) * unit
}

fn build_frames(app: &AppHandle) -> Option<Frames> {
    let icon = app.default_window_icon()?;
    let (rgba, w, h) = (icon.rgba(), icon.width(), icon.height());
    let idle = Image::new_owned(rgba.to_vec(), w, h);
    let busy = (0..FRAME_COUNT)
        .map(|i| Image::new_owned(scale_alpha(rgba, breath_scale(i, FRAME_COUNT)), w, h))
        .collect();
    Some(Frames { idle, busy })
}

/// Borrows `frame`'s bytes into a transient `Image` rather than cloning them
/// -- `set_icon` copies into an OS icon synchronously, so the borrow is only
/// alive for the call, and this keeps the ~11-per-second updates from
/// churning a fresh RGBA `Vec` each tick.
fn set_tray_icon(app: &AppHandle, frame: &Image<'_>) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let borrowed = Image::new(frame.rgba(), frame.width(), frame.height());
        let _ = tray.set_icon(Some(borrowed));
    }
}

/// Sleeps up to `total_ms`, waking early if `cancel` is set. Returns `true`
/// if it was cancelled (the caller should stop), `false` if the full
/// duration elapsed.
fn sleep_cancellable(cancel: &AtomicBool, total_ms: u64) -> bool {
    let mut slept = 0;
    while slept < total_ms {
        if cancel.load(Ordering::SeqCst) {
            return true;
        }
        let step = POLL_MS.min(total_ms - slept);
        std::thread::sleep(Duration::from_millis(step));
        slept += step;
    }
    cancel.load(Ordering::SeqCst)
}

fn run_animation(app: AppHandle, cancel: std::sync::Arc<AtomicBool>) {
    // Delay gate: a cycle that finishes inside this window never touched the
    // icon, so there is nothing to restore -- return without setting idle.
    if sleep_cancellable(&cancel, SHOW_DELAY_MS) {
        return;
    }
    let Some(Some(frames)) = FRAMES.get() else { return };

    let mut i = 0;
    loop {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        set_tray_icon(&app, &frames.busy[i]);
        i = (i + 1) % frames.busy.len();
        if sleep_cancellable(&cancel, FRAME_INTERVAL_MS) {
            break;
        }
    }
    // We changed the icon, so restore it. This is the animation thread's
    // last act, and `Drop` joins it before returning, so no stray frame can
    // land after the idle icon.
    set_tray_icon(&app, &frames.idle);
}

/// Animates the tray icon for the lifetime of one cycle. See the module doc.
pub struct TrayAnimation {
    cancel: std::sync::Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TrayAnimation {
    pub fn begin(app: AppHandle) -> Self {
        let has_frames = FRAMES.get_or_init(|| build_frames(&app)).is_some();
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let handle = if has_frames {
            let cancel = cancel.clone();
            Some(std::thread::spawn(move || run_animation(app, cancel)))
        } else {
            None
        };
        Self { cancel, handle }
    }

    /// Stop animating and restore the idle icon. Consumes `self` so this
    /// reads as a deliberate end-of-cycle call; the work happens in `Drop`,
    /// which also covers the paths that never reach here.
    pub fn finish(self) {}
}

impl Drop for TrayAnimation {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        // Join so the idle-restore the thread performs on its way out has
        // completed before this returns -- otherwise a caller that starts a
        // new cycle immediately could race a lingering frame from this one.
        // Bounded: the thread checks `cancel` at least every POLL_MS.
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breath_scale_is_full_at_the_start_and_dimmest_at_the_midpoint() {
        assert!((breath_scale(0, FRAME_COUNT) - 1.0).abs() < 1e-9, "frame 0 is full opacity");
        let mid = breath_scale(FRAME_COUNT / 2, FRAME_COUNT);
        assert!(
            (mid - MIN_ALPHA_SCALE).abs() < 1e-9,
            "the midpoint frame is the dimmest, got {mid}"
        );
    }

    #[test]
    fn breath_scale_never_leaves_the_visible_range() {
        for i in 0..FRAME_COUNT {
            let s = breath_scale(i, FRAME_COUNT);
            assert!(
                (MIN_ALPHA_SCALE..=1.0).contains(&s),
                "frame {i} scale {s} escaped [{MIN_ALPHA_SCALE}, 1.0] -- the glyph would vanish or over-brighten"
            );
        }
    }

    #[test]
    fn scale_alpha_touches_only_the_alpha_byte_and_preserves_color() {
        // One opaque teal pixel: R,G,B must survive, A halves.
        let px = [10u8, 120, 90, 200];
        let out = scale_alpha(&px, 0.5);
        assert_eq!(&out[0..3], &[10, 120, 90], "color channels must be untouched");
        assert_eq!(out[3], 100, "alpha must scale");
    }

    #[test]
    fn scale_alpha_at_full_scale_is_the_identity() {
        let rgba = [1u8, 2, 3, 255, 4, 5, 6, 128];
        assert_eq!(scale_alpha(&rgba, 1.0), rgba.to_vec());
    }

    #[test]
    fn a_cancelled_sleep_returns_true_without_waiting_the_whole_duration() {
        let cancel = AtomicBool::new(true);
        assert!(sleep_cancellable(&cancel, 10_000), "an already-set cancel must short-circuit");
    }

    #[test]
    fn an_uncancelled_sleep_runs_to_completion() {
        let cancel = AtomicBool::new(false);
        assert!(!sleep_cancellable(&cancel, 20), "no cancel -> false (ran the full duration)");
    }
}
