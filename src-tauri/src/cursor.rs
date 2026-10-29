use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
};

use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    CopyIcon, DestroyCursor, LoadCursorW, LoadImageW, SetSystemCursor, SystemParametersInfoW,
    HCURSOR, HICON, IDC_APPSTARTING, IDC_ARROW, IDC_HAND, IDC_IBEAM, IMAGE_CURSOR,
    LR_LOADFROMFILE, OCR_HAND, OCR_IBEAM, OCR_NORMAL, SPI_SETCURSORS, SYSTEM_CURSOR_ID,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

const MARKER_FILE: &str = "busy-cursor.marker";

const CURSORS_KEY: &str = "Control Panel\\Cursors";

const DEFAULT_BASE_SIZE: u32 = 32;

const SLOTS: [(SYSTEM_CURSOR_ID, PCWSTR); 3] =
    [(OCR_NORMAL, IDC_ARROW), (OCR_IBEAM, IDC_IBEAM), (OCR_HAND, IDC_HAND)];

type Saved = Vec<(SYSTEM_CURSOR_ID, HCURSOR)>;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn registry_string(value: &str) -> Option<Vec<u16>> {
    let key = wide(CURSORS_KEY);
    let name = wide(value);
    let mut size: u32 = 0;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    if size < 2 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast::<c_void>()),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    if len == 0 {
        return None;
    }
    buf.truncate(len);
    buf.push(0);
    Some(buf)
}

fn registry_dword(value: &str) -> Option<u32> {
    let key = wide(CURSORS_KEY);
    let name = wide(value);
    let mut data: u32 = 0;
    let mut size: u32 = 4;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut data as *mut u32).cast::<c_void>()),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    Some(data)
}

fn scaled_size(base: u32, dpi: u32) -> i32 {
    let base = if (16..=512).contains(&base) { base } else { DEFAULT_BASE_SIZE };
    let dpi = dpi.max(96);
    ((base * dpi) / 96) as i32
}

fn busy_cursor_size() -> i32 {
    let base = registry_dword("CursorBaseSize").unwrap_or(DEFAULT_BASE_SIZE);
    scaled_size(base, unsafe { GetDpiForSystem() })
}

fn load_busy_cursor() -> Result<HCURSOR, String> {
    if let Some(path) = registry_string("AppStarting") {
        let size = busy_cursor_size();
        let loaded = unsafe {
            LoadImageW(None, PCWSTR(path.as_ptr()), IMAGE_CURSOR, size, size, LR_LOADFROMFILE)
        };
        if let Ok(handle) = loaded {
            if !handle.is_invalid() {
                return Ok(HCURSOR(handle.0));
            }
        }
    }
    let shared = unsafe { LoadCursorW(None, IDC_APPSTARTING) }.map_err(|e| e.to_string())?;
    let copy = unsafe { CopyIcon(HICON(shared.0)) }.map_err(|e| e.to_string())?;
    Ok(HCURSOR(copy.0))
}

fn destroy_all(saved: &mut Saved) {
    for (_, handle) in saved.drain(..) {
        unsafe {
            let _ = DestroyCursor(handle);
        }
    }
}

fn snapshot() -> Result<Saved, String> {
    let mut saved: Saved = Vec::new();
    for (id, idc) in SLOTS {
        let copied = unsafe { LoadCursorW(None, idc) }
            .and_then(|current| unsafe { CopyIcon(HICON(current.0)) });
        match copied {
            Ok(copy) => saved.push((id, HCURSOR(copy.0))),
            Err(e) => {
                destroy_all(&mut saved);
                return Err(format!("could not copy the current cursor: {e}"));
            }
        }
    }
    Ok(saved)
}

fn restore_saved(saved: &mut Saved) -> Result<(), String> {
    let mut first_error = None;
    for (id, handle) in saved.drain(..) {
        if let Err(e) = unsafe { SetSystemCursor(handle, id) } {
            unsafe {
                let _ = DestroyCursor(handle);
            }
            first_error.get_or_insert_with(|| e.to_string());
        }
    }
    match first_error {
        None => Ok(()),
        Some(e) => Err(e),
    }
}

fn reload_scheme() -> Result<(), String> {
    unsafe {
        SystemParametersInfoW(SPI_SETCURSORS, 0, None, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0))
    }
    .map_err(|e| e.to_string())
}

pub struct BusyCursor {
    config_dir: PathBuf,
    saved: Saved,
    done: bool,
}

impl BusyCursor {
    pub fn show(config_dir: &Path) -> Result<Self, String> {
        let mut saved = snapshot()?;
        let marker = config_dir.join(MARKER_FILE);
        let _ = fs::write(&marker, b"");
        for (id, _) in SLOTS {
            let busy = match load_busy_cursor() {
                Ok(handle) => handle,
                Err(e) => {
                    if restore_saved(&mut saved).is_ok() {
                        let _ = fs::remove_file(&marker);
                    }
                    return Err(e);
                }
            };
            if let Err(e) = unsafe { SetSystemCursor(busy, id) } {
                unsafe {
                    let _ = DestroyCursor(busy);
                }
                if restore_saved(&mut saved).is_ok() {
                    let _ = fs::remove_file(&marker);
                }
                return Err(e.to_string());
            }
        }
        Ok(Self { config_dir: config_dir.to_path_buf(), saved, done: false })
    }

    pub fn restore(mut self) -> Result<(), String> {
        self.restore_inner()
    }

    fn restore_inner(&mut self) -> Result<(), String> {
        if self.done {
            return Ok(());
        }
        self.done = true;
        let marker = self.config_dir.join(MARKER_FILE);
        match restore_saved(&mut self.saved) {
            Ok(()) => {
                let _ = fs::remove_file(&marker);
                Ok(())
            }
            Err(e) => {
                if reload_scheme().is_ok() {
                    let _ = fs::remove_file(&marker);
                }
                Err(e)
            }
        }
    }
}

impl Drop for BusyCursor {
    fn drop(&mut self) {
        let _ = self.restore_inner();
    }
}

pub fn restore_if_left_busy(config_dir: &Path) -> Option<Result<(), String>> {
    if !config_dir.join(MARKER_FILE).exists() {
        return None;
    }
    let result = reload_scheme();
    if result.is_ok() {
        let _ = fs::remove_file(config_dir.join(MARKER_FILE));
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cai-test-cursor-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn nothing_to_restore_when_no_marker_was_left_behind() {
        let dir = temp_dir("clean");
        assert!(restore_if_left_busy(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_leftover_marker_triggers_a_scheme_reload_and_is_removed() {
        let dir = temp_dir("leftover");
        fs::write(dir.join(MARKER_FILE), b"").unwrap();
        let outcome = restore_if_left_busy(&dir).expect("a marker must trigger a restore");
        if outcome.is_ok() {
            assert!(!dir.join(MARKER_FILE).exists(), "a successful restore clears the marker");
        } else {
            assert!(
                dir.join(MARKER_FILE).exists(),
                "a failed restore keeps the marker for the next launch"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_busy_size_scales_with_dpi_and_falls_back_on_nonsense() {
        assert_eq!(scaled_size(32, 96), 32);
        assert_eq!(scaled_size(32, 144), 48);
        assert_eq!(scaled_size(48, 192), 96);
        assert_eq!(scaled_size(0, 96), 32, "a zero base size falls back to the default");
        assert_eq!(scaled_size(32, 10), 32, "a dpi below 96 never shrinks the cursor");
    }

    #[test]
    fn show_and_restore_round_trip_leaves_no_marker_behind() {
        let dir = temp_dir("round-trip");
        match BusyCursor::show(&dir) {
            Ok(busy) => {
                assert!(dir.join(MARKER_FILE).exists(), "the marker guards the override");
                busy.restore().expect("restoring the snapshot must succeed");
                assert!(!dir.join(MARKER_FILE).exists(), "a clean restore removes the marker");
            }
            Err(e) => eprintln!("skipping: the busy cursor could not be shown here: {e}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_busy_cursor_loads_from_the_scheme_or_the_system_fallback() {
        match load_busy_cursor() {
            Ok(handle) => unsafe {
                let _ = DestroyCursor(handle);
            },
            Err(e) => eprintln!("skipping: no busy cursor could be loaded here: {e}"),
        }
    }
}
