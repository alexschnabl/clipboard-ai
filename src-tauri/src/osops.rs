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
        let mut enigo = self.enigo.lock().map_err(|_| OsError::Input("poisoned".into()))?;
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| OsError::Input(e.to_string()))?;
        let click = enigo.key(Key::Unicode(letter), Direction::Click);
        // Release Control even if the letter failed, so a failure never leaves
        // a modifier logically stuck down for the user.
        let release = enigo.key(Key::Control, Direction::Release);
        click.map_err(|e| OsError::Input(e.to_string()))?;
        release.map_err(|e| OsError::Input(e.to_string()))?;
        Ok(())
    }
}

impl OsOps for WindowsOps {
    fn read_clipboard(&self) -> Result<String, OsError> {
        let mut cb = self.clipboard.lock().map_err(|_| OsError::Clipboard("poisoned".into()))?;
        // Do not flatten a read failure into `Ok(String::new())`: a non-text
        // clipboard (an image, a file list) and a locked/unreadable one both
        // fail here, and the caller must be able to tell that apart from a
        // clipboard that genuinely holds an empty string. Callers that treat
        // "unreadable" the same as "empty" risk overwriting content they
        // never actually read (see cycle::ClipboardGuard).
        cb.get_text().map_err(|e| OsError::Clipboard(e.to_string()))
    }

    fn write_clipboard(&self, text: &str) -> Result<(), OsError> {
        let mut cb = self.clipboard.lock().map_err(|_| OsError::Clipboard("poisoned".into()))?;
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
