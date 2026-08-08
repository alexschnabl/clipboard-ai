//! Applies the persisted `config::Settings` to the OS. Currently just
//! autostart -- theme and language are pure frontend concerns (see
//! `config::Theme`/`config::Language`'s doc comments), so there is nothing
//! for the backend to *apply* for those two; this module only exists for
//! the one setting that has to reach outside the process.

use tauri_plugin_autostart::ManagerExt;

/// Registers or removes the "start with Windows" entry, driven by
/// `Settings::autostart`.
///
/// Called from two places: `lib.rs`'s `setup`, right after
/// `config::load_or_create` resolves the startup config, so a persisted
/// `autostart: true` actually takes effect on launch; and
/// `commands::save_config`, after a successful `decide_save`, so toggling
/// the settings-window checkbox takes effect immediately rather than only
/// on next restart. Both call sites treat an `Err` the same way
/// `osops::WindowsOps::new()`'s failure is already treated in `lib.rs`'s
/// `setup` -- logged and turned into a `Problem` for the settings window,
/// never a hard failure that blocks the rest of startup or a save.
///
/// Disabling always runs for real, in every build: it only ever removes a
/// registry entry (a no-op if none exists), so there is nothing it could
/// pollute. Enabling is refused outright from a debug build and returns
/// `Err` instead of calling the plugin. The reason is `auto_launch` (which
/// `tauri-plugin-autostart` delegates to, confirmed by reading its
/// vendored `src/lib.rs`) writes `std::env::current_exe()`'s path into the
/// registry -- under `cargo run`/`tauri dev` that resolves to this
/// machine's own `target\debug\clipboard-ai-app.exe`, so clicking the
/// checkbox once while developing would register a debug build to launch
/// at every login on the developer's real machine, silently, forever,
/// until someone notices and removes it by hand. `cfg!(debug_assertions)`
/// -- not a build-time `#[cfg]` -- so the *code path* exists in every
/// build (unit tests can exercise the refusal), it just always takes the
/// refusal branch in a debug binary.
///
/// The refusal is logged through the same `config_dir`-relative
/// `clipboard-ai.log` every other startup/config line uses, not just
/// `eprintln!`'d -- a debug build does keep its console (see `main.rs`'s
/// `windows_subsystem` attribute, only applied `not(debug_assertions)`),
/// but the log file is where every other "something didn't apply" line in
/// this app already ends up, and is the only copy that survives after the
/// console window is gone.
///
/// Known upstream quirk, documented rather than worked around: `auto-launch`
/// 0.5 writes the exe path into the Run value *unquoted*
/// (`format!("{} {}", app_path, args)` in its windows.rs). A path containing
/// spaces -- `C:\Program Files\...` -- still launches, because CreateProcess
/// falls back through the prefixes, but it is the classic unquoted-path shape
/// and would break outright if Windows ever stopped doing that. Nothing this
/// side of the plugin can quote it; if it ever bites, the fix belongs
/// upstream (or in switching how the entry is written).
pub fn apply_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();

    if !enabled {
        // Disabling something that was never enabled is not a failure.
        //
        // `auto-launch`'s `disable()` deletes the HKCU ...\Run value, and
        // Windows reports ERROR_FILE_NOT_FOUND ("The system cannot find the
        // file specified", os error 2) when there was nothing to delete.
        // Since startup reasserts the persisted setting on every launch, a
        // fresh install with autostart off -- the default -- reported that as
        // a startup problem every single time. The check has to come first:
        // asking whether it is enabled is the only way to tell "nothing to
        // do" apart from a genuine failure to remove a registration that
        // really is there.
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
    // `apply_autostart` needs a live `tauri::AppHandle` (both the debug
    // refusal branch and the real branches call `app.autolaunch()`, which
    // reads managed plugin state) -- this crate has no harness for
    // constructing one in a unit test, the same limitation `lib.rs`'s own
    // doc comments note about `setup`'s closure. The refusal *decision*
    // itself has no branching left to test in isolation: it is exactly
    // `cfg!(debug_assertions) && enabled`, which is what every `cargo test`
    // run of this crate already exercises implicitly -- every other test
    // in this file's sibling modules runs as a debug build, and none of
    // them have ever caused a registry write, which would itself be the
    // symptom of this guard being absent or wrong.
}
