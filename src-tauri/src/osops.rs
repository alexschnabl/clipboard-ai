use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use clipboard_win::{raw, Clipboard as WinClipboard};
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Clip {
    pub text: String,
    pub html: Option<String>,
}

const CONTROL_RELEASE_RETRIES: u32 = 3;

const HTML_FORMAT_NAME: &str = "HTML Format";

const CLIPBOARD_OPEN_ATTEMPTS: usize = 10;

const CLIPBOARD_OPEN_RETRY_MS: u64 = 5;

pub trait OsOps: Send + Sync {
    fn read_clipboard(&self) -> Result<Clip, OsError>;
    fn write_clipboard(&self, clip: &Clip) -> Result<(), OsError>;
    fn clear_clipboard(&self) -> Result<(), OsError>;
    fn modifiers_down(&self) -> bool;
    fn foreground_window(&self) -> isize;
    fn send_copy(&self) -> Result<(), OsError>;
    fn send_paste(&self) -> Result<(), OsError>;
    fn sleep_ms(&self, ms: u64);
    fn now_ms(&self) -> u64;
}

fn html_format() -> Option<u32> {
    raw::register_format(HTML_FORMAT_NAME).map(|n| n.get())
}

fn open_clipboard() -> Result<WinClipboard, OsError> {
    let mut last_error = None;
    for attempt in 0..CLIPBOARD_OPEN_ATTEMPTS {
        match WinClipboard::new() {
            Ok(open) => return Ok(open),
            Err(e) => last_error = Some(e),
        }
        if attempt + 1 < CLIPBOARD_OPEN_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(CLIPBOARD_OPEN_RETRY_MS));
        }
    }
    Err(OsError::Clipboard(format!(
        "could not open the clipboard after {CLIPBOARD_OPEN_ATTEMPTS} attempts: {}",
        last_error.map(|e| e.to_string()).unwrap_or_default()
    )))
}

fn read_html_while_open() -> Option<String> {
    let format = html_format()?;
    if !raw::is_format_avail(format) {
        return None;
    }
    let mut bytes = Vec::new();
    raw::get_vec(format, &mut bytes).ok()?;
    crate::cfhtml::fragment(&bytes)
}

pub struct WindowsOps {
    clipboard: Mutex<()>,
    enigo: Mutex<Enigo>,
    start: Instant,
}

impl WindowsOps {
    pub fn new() -> Result<Self, OsError> {
        Ok(Self {
            clipboard: Mutex::new(()),
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

    fn lock_clipboard(&self) -> std::sync::MutexGuard<'_, ()> {
        self.clipboard.lock().unwrap_or_else(|e| {
            eprintln!("clipboard mutex was poisoned by an earlier panic; recovering");
            e.into_inner()
        })
    }
}

impl OsOps for WindowsOps {
    fn read_clipboard(&self) -> Result<Clip, OsError> {
        let _serial = self.lock_clipboard();
        let _open = open_clipboard()?;
        let mut bytes = Vec::new();
        raw::get_string(&mut bytes)
            .map_err(|e| OsError::Clipboard(format!("no text on the clipboard: {e}")))?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let html = read_html_while_open();
        Ok(Clip { text, html })
    }

    fn write_clipboard(&self, clip: &Clip) -> Result<(), OsError> {
        let _serial = self.lock_clipboard();
        let _open = open_clipboard()?;
        raw::set_string(&clip.text).map_err(|e| OsError::Clipboard(e.to_string()))?;
        if let Some(html) = &clip.html {
            let format = html_format().ok_or_else(|| {
                OsError::Clipboard("could not register the HTML clipboard format".into())
            })?;
            raw::set_without_clear(format, &crate::cfhtml::build(html))
                .map_err(|e| OsError::Clipboard(e.to_string()))?;
        }
        Ok(())
    }

    fn clear_clipboard(&self) -> Result<(), OsError> {
        self.write_clipboard(&Clip::default())
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
    fn read_clipboard(&self) -> Result<Clip, OsError> {
        Err(OsError::Clipboard("unavailable: OS layer failed to initialize at startup".into()))
    }
    fn write_clipboard(&self, _clip: &Clip) -> Result<(), OsError> {
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

    static REAL_CLIPBOARD: Mutex<()> = Mutex::new(());

    #[test]
    fn a_poisoned_clipboard_mutex_recovers_instead_of_disabling_every_future_call() {
        let _serial = REAL_CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
        let ops = match WindowsOps::new() {
            Ok(ops) => ops,
            Err(e) => {
                eprintln!("skipping: WindowsOps::new() failed here: {e}");
                return;
            }
        };
        let original = ops.read_clipboard().ok();

        let ops = Arc::new(ops);
        let poisoner = ops.clone();
        let panicked = std::thread::spawn(move || {
            let _guard = poisoner.clipboard.lock().unwrap();
            panic!("simulated panic while holding the clipboard lock");
        })
        .join();
        assert!(panicked.is_err(), "the spawned thread should have panicked");
        assert!(ops.clipboard.is_poisoned(), "the mutex should now be poisoned");

        let probe = Clip { text: "poison-recovery-probe".into(), html: None };
        ops.write_clipboard(&probe)
            .expect("write should recover from a poisoned lock, not fail forever");
        assert_eq!(ops.read_clipboard().unwrap().text, "poison-recovery-probe");

        if let Some(original) = original {
            let _ = ops.write_clipboard(&original);
        }
    }

    #[test]
    fn html_written_with_a_text_fallback_reads_back_as_both_formats() {
        let _serial = REAL_CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
        let ops = match WindowsOps::new() {
            Ok(ops) => ops,
            Err(e) => {
                eprintln!("skipping: WindowsOps::new() failed here: {e}");
                return;
            }
        };
        let original = ops.read_clipboard().ok();

        let rich = Clip {
            text: "fallback text äöü".into(),
            html: Some("<p>fallback <b>text</b> äöü</p>".into()),
        };
        ops.write_clipboard(&rich).expect("rich write");
        let back = ops.read_clipboard().expect("read back");
        assert_eq!(back.text, "fallback text äöü");
        assert_eq!(back.html.as_deref(), Some("<p>fallback <b>text</b> äöü</p>"));

        ops.write_clipboard(&Clip { text: "plain only".into(), html: None }).expect("plain write");
        let back = ops.read_clipboard().expect("read back");
        assert_eq!(back.text, "plain only");
        assert!(back.html.is_none(), "a plain write must replace the HTML format too");

        if let Some(original) = original {
            let _ = ops.write_clipboard(&original);
        }
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
        let outcome = crate::cycle::run(
            &PanicIfCalledBackend,
            "Fix.",
            crate::config::Formatting::Auto,
            &ops,
        );
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "focus-changed");
    }
}
