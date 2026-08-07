use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use thiserror::Error;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

#[derive(Debug, Error)]
pub enum OsError {
    #[error("clipboard error: {0}")]
    Clipboard(String),
    #[error("synthetic input error: {0}")]
    Input(String),
}

/// R13: how many times `chord` retries a failed Control-release before
/// giving up and surfacing the failure. A stuck-down Control is bad enough
/// (see `chord`'s doc comment) that a couple of cheap extra attempts are
/// worth it even though a `SendInput` failure is not usually the kind of
/// thing that clears up on its own.
const CONTROL_RELEASE_RETRIES: u32 = 3;

pub trait OsOps: Send + Sync {
    fn read_clipboard(&self) -> Result<String, OsError>;
    fn write_clipboard(&self, text: &str) -> Result<(), OsError>;
    fn clear_clipboard(&self) -> Result<(), OsError>;
    fn modifiers_down(&self) -> bool;
    fn foreground_window(&self) -> isize;
    fn send_copy(&self) -> Result<(), OsError>;
    fn send_paste(&self) -> Result<(), OsError>;
    fn sleep_ms(&self, ms: u64);
    fn now_ms(&self) -> u64;
}

pub struct WindowsOps {
    clipboard: Mutex<Clipboard>,
    enigo: Mutex<Enigo>,
    start: Instant,
}

impl WindowsOps {
    pub fn new() -> Result<Self, OsError> {
        Ok(Self {
            clipboard: Mutex::new(
                Clipboard::new().map_err(|e| OsError::Clipboard(e.to_string()))?,
            ),
            enigo: Mutex::new(
                Enigo::new(&Settings::default()).map_err(|e| OsError::Input(e.to_string()))?,
            ),
            start: Instant::now(),
        })
    }

    fn chord(&self, letter: char) -> Result<(), OsError> {
        // A panic while holding this lock (e.g. inside `enigo`) poisons the
        // mutex; the default `?` on `.lock()` would then make every future
        // cycle abort forever while the app still looks alive, with no way
        // to recover short of a restart. There is no invariant of ours to
        // protect here -- `Enigo` holds no partially-mutated state of ours
        // across calls -- so recover the guard instead of propagating the
        // poison. Logged, matching `lib.rs`'s `recover_lock` (its own doc
        // comment states that logging every recovery, not just performing
        // it silently, was the intent) -- `status::append_log` isn't
        // reachable from here without threading a path through `OsOps`, but
        // the console line costs nothing.
        let mut enigo = self.enigo.lock().unwrap_or_else(|e| {
            eprintln!("enigo mutex was poisoned by an earlier panic; recovering");
            e.into_inner()
        });
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| OsError::Input(e.to_string()))?;
        let click = enigo.key(Key::Unicode(letter), Direction::Click);
        // Release Control even if the letter failed, so a failure never leaves
        // a modifier logically stuck down for the user.
        //
        // R13: a failed release here is worse than a failed click -- it can
        // leave Control synthetically held down OS-wide, so the user's very
        // next ordinary keystroke in *any* application fires as a Ctrl
        // shortcut, silently, until they happen to tap Control themselves.
        // Retry it a few times before giving up, and -- unlike before,
        // where a release failure alongside a click failure was never even
        // inspected, because the click's own `?` returned first and threw
        // the release's `Result` away unread -- always surface a release
        // failure, in preference to a click failure if both happened: a
        // missed paste is recoverable by pressing the hotkey again, a
        // stuck Control is not something the user can diagnose from inside
        // the app that caused it.
        let mut release = enigo.key(Key::Control, Direction::Release);
        for attempt in 1..CONTROL_RELEASE_RETRIES {
            if release.is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10 * attempt as u64));
            release = enigo.key(Key::Control, Direction::Release);
        }
        if let Err(e) = release {
            return Err(OsError::Input(format!(
                "Control release failed after {CONTROL_RELEASE_RETRIES} attempts -- it may \
                 still be held down: {e}"
            )));
        }
        click.map_err(|e| OsError::Input(e.to_string()))?;
        Ok(())
    }
}

impl OsOps for WindowsOps {
    fn read_clipboard(&self) -> Result<String, OsError> {
        // See the comment on `chord`: recover a poisoned lock rather than
        // propagate it, and log it, matching `chord`. Losing one in-flight
        // clipboard operation to a panic is not an invariant violation worth
        // permanently disabling every future cycle over.
        let mut cb = self.clipboard.lock().unwrap_or_else(|e| {
            eprintln!("clipboard mutex was poisoned by an earlier panic; recovering");
            e.into_inner()
        });
        // Do not flatten a read failure into `Ok(String::new())`: a non-text
        // clipboard (an image, a file list) and a locked/unreadable one both
        // fail here, and the caller must be able to tell that apart from a
        // clipboard that genuinely holds an empty string. Callers that treat
        // "unreadable" the same as "empty" risk overwriting content they
        // never actually read (see cycle::ClipboardGuard).
        cb.get_text().map_err(|e| OsError::Clipboard(e.to_string()))
    }

    fn write_clipboard(&self, text: &str) -> Result<(), OsError> {
        // See `read_clipboard`: recover a poisoned lock and log it.
        let mut cb = self.clipboard.lock().unwrap_or_else(|e| {
            eprintln!("clipboard mutex was poisoned by an earlier panic; recovering");
            e.into_inner()
        });
        cb.set_text(text.to_string()).map_err(|e| OsError::Clipboard(e.to_string()))
    }

    fn clear_clipboard(&self) -> Result<(), OsError> {
        self.write_clipboard("")
    }

    fn modifiers_down(&self) -> bool {
        unsafe {
            [VK_SHIFT, VK_CONTROL, VK_MENU]
                .iter()
                .any(|vk| (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0)
        }
    }

    fn foreground_window(&self) -> isize {
        unsafe { GetForegroundWindow().0 as isize }
    }

    fn send_copy(&self) -> Result<(), OsError> {
        self.chord('c')
    }

    fn send_paste(&self) -> Result<(), OsError> {
        self.chord('v')
    }

    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

/// A safe no-op `OsOps` used only when `WindowsOps::new()` itself fails to
/// initialize (no clipboard or input subsystem available, e.g. a
/// non-interactive session). Every clipboard/input operation reports
/// failure and `foreground_window` reports 0, so a cycle run against it
/// hits the I2 zero-focus guard and aborts immediately, before any
/// clipboard mutation or synthetic input -- harmless rather than a panic.
/// This exists so a broken OS layer still lets the app reach a tray icon
/// (see I5) instead of the whole process dying with no message at all.
pub struct NullOps {
    start: Instant,
}

impl NullOps {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }
}

impl OsOps for NullOps {
    fn read_clipboard(&self) -> Result<String, OsError> {
        Err(OsError::Clipboard("unavailable: OS layer failed to initialize at startup".into()))
    }
    fn write_clipboard(&self, _text: &str) -> Result<(), OsError> {
        Err(OsError::Clipboard("unavailable: OS layer failed to initialize at startup".into()))
    }
    fn clear_clipboard(&self) -> Result<(), OsError> {
        Err(OsError::Clipboard("unavailable: OS layer failed to initialize at startup".into()))
    }
    fn modifiers_down(&self) -> bool {
        false
    }
    fn foreground_window(&self) -> isize {
        0
    }
    fn send_copy(&self) -> Result<(), OsError> {
        Err(OsError::Input("unavailable: OS layer failed to initialize at startup".into()))
    }
    fn send_paste(&self) -> Result<(), OsError> {
        Err(OsError::Input("unavailable: OS layer failed to initialize at startup".into()))
    }
    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    /// A panic while holding the clipboard or enigo mutex must not
    /// permanently disable every future cycle: there is no invariant of
    /// ours protected by the poison flag (`Clipboard`/`Enigo` hold no
    /// partially-mutated state of ours across calls), so recovering the
    /// guard must let normal operation continue instead of every
    /// subsequent `read_clipboard`/`write_clipboard` failing forever.
    #[test]
    fn a_poisoned_clipboard_mutex_recovers_instead_of_disabling_every_future_call() {
        let ops = match WindowsOps::new() {
            Ok(ops) => ops,
            Err(e) => {
                // No clipboard/input subsystem available in this
                // environment (e.g. a non-interactive session) -- nothing
                // to exercise this fix against.
                eprintln!("skipping: WindowsOps::new() failed here: {e}");
                return;
            }
        };
        let original = ops.read_clipboard().unwrap_or_default();

        let ops = Arc::new(ops);
        let poisoner = ops.clone();
        let panicked = std::thread::spawn(move || {
            let _guard = poisoner.clipboard.lock().unwrap();
            panic!("simulated panic while holding the clipboard lock");
        })
        .join();
        assert!(panicked.is_err(), "the spawned thread should have panicked");
        assert!(ops.clipboard.is_poisoned(), "the mutex should now be poisoned");

        // Before the fix, `.lock().map_err(...)?` propagated the poison and
        // every call below would return `Err("clipboard error: poisoned")`
        // forever.
        ops.write_clipboard("poison-recovery-probe")
            .expect("write should recover from a poisoned lock, not fail forever");
        assert_eq!(ops.read_clipboard().unwrap(), "poison-recovery-probe");

        let _ = ops.write_clipboard(&original);
    }

    struct PanicIfCalledBackend;
    impl crate::backend::Backend for PanicIfCalledBackend {
        fn name(&self) -> &str {
            "panic-if-called"
        }
        fn transform(
            &self,
            _system_prompt: &str,
            _text: &str,
        ) -> Result<String, crate::backend::BackendError> {
            panic!("the backend must never be reached when NullOps reports no focus");
        }
    }

    #[test]
    fn a_cycle_run_against_null_ops_aborts_immediately_without_touching_anything() {
        // I5: this is the fallback used when WindowsOps::new() fails.
        // foreground_window() == 0 must trip the I2 guard before the cycle
        // ever calls the backend or attempts a clipboard read/write.
        let ops = NullOps::new();
        let outcome = crate::cycle::run(&PanicIfCalledBackend, "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "focus-changed");
    }
}
