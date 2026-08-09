fn main() {
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
