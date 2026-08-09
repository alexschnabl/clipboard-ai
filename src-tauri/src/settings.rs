
use tauri_plugin_autostart::ManagerExt;

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

    manager.enable().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
}
