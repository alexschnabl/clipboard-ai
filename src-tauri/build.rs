fn main() {
    // Declaring the app's own commands here generates an `allow-<command>`
    // permission for each and -- more importantly -- flips Tauri's runtime
    // into gating *app* commands through the ACL at all. Without this
    // manifest, only plugin commands are checked: `capabilities/default.json`
    // could say whatever it liked about which window may call `save_config`
    // or `get_history`, and the runtime would never consult it (verified in
    // tauri 2.11.5 webview/mod.rs -- the check is skipped for local origins
    // when no app manifest exists). With it, the capability file becomes the
    // real authority: only the settings window (the sole window that calls
    // them) is granted these six.
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "get_config",
            "save_config",
            "suspend_hotkeys",
            "resume_hotkeys",
            "get_history",
            "clear_history",
        ])),
    )
    .expect("failed to run tauri-build");
}
