use std::{ffi::c_void, path::Path};

use tauri_plugin_autostart::ManagerExt;
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

pub fn apply_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();

    if !enabled {
        if matches!(manager.is_enabled(), Ok(false)) {
            return Ok(());
        }
        return manager.disable().map_err(|e| e.to_string());
    }

    if cfg!(debug_assertions) {
        let message = "refusing to enable autostart from a debug build: auto-launch would \
                        register this machine's own target\\debug\\clipboard-ai-app.exe to run \
                        at every login; test this with an installed (`npm run tauri build`) \
                        build instead"
            .to_string();
        let line = format!("AUTOSTART {message}");
        eprintln!("{line}");
        if let Some(dir) = crate::config::config_path(app).parent() {
            crate::status::append_log(dir, &line);
        }
        return Err(message);
    }

    manager.enable().map_err(|e| e.to_string())?;
    if let Err(e) = quote_run_entry(&app.package_info().name) {
        let line = format!("AUTOSTART could not quote the Run entry: {e}");
        eprintln!("{line}");
        if let Some(dir) = crate::config::config_path(app).parent() {
            crate::status::append_log(dir, &line);
        }
    }
    Ok(())
}

pub(crate) fn run_entry_value(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn quote_run_entry(name: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let key = wide(RUN_KEY);
    let value_name = wide(name);
    let data = wide(&run_entry_value(&exe));
    unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(value_name.as_ptr()),
            REG_SZ.0,
            Some(data.as_ptr().cast::<c_void>()),
            (data.len() * 2) as u32,
        )
    }
    .ok()
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::run_entry_value;
    use std::path::Path;

    #[test]
    fn the_autostart_entry_quotes_a_path_with_spaces() {
        let exe = Path::new(r"C:\Users\Work\AppData\Local\Clipboard AI\clipboard-ai-app.exe");
        assert_eq!(
            run_entry_value(exe),
            r#""C:\Users\Work\AppData\Local\Clipboard AI\clipboard-ai-app.exe""#
        );
    }
}
