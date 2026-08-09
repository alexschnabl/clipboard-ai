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
        let mut enigo = self.enigo.lock().unwrap_or_else(|e| {
            eprintln!("enigo mutex was poisoned by an earlier panic; recovering");
            e.into_inner()
        });
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| OsError::Input(e.to_string()))?;
        let click = enigo.key(Key::Unicode(letter), Direction::Click);
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
        let mut cb = self.clipboard.lock().unwrap_or_else(|e| {
            eprintln!("clipboard mutex was poisoned by an earlier panic; recovering");
            e.into_inner()
        });
        cb.get_text().map_err(|e| OsError::Clipboard(e.to_string()))
    }

    fn write_clipboard(&self, text: &str) -> Result<(), OsError> {
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

    #[test]
    fn a_poisoned_clipboard_mutex_recovers_instead_of_disabling_every_future_call() {
        let ops = match WindowsOps::new() {
            Ok(ops) => ops,
            Err(e) => {
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
        let ops = NullOps::new();
        let outcome = crate::cycle::run(&PanicIfCalledBackend, "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "focus-changed");
    }
}
