use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const KIND_OPENAI: &str = "openai-compatible";
pub const KIND_ANTHROPIC: &str = "anthropic-api";
pub const KIND_CLAUDE_CLI: &str = "claude-cli";

pub const CURRENT_VERSION: u32 = 2;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config is not valid JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("command {command} names unknown backend {backend}")]
    UnknownBackend { command: String, backend: String },
    #[error("duplicate id: {id}")]
    DuplicateId { id: String },
    #[error("duplicate hotkey: {hotkey}")]
    DuplicateHotkey { hotkey: String },
    #[error("unknown backend kind: {kind}")]
    UnknownKind { kind: String },
    #[error("config version {version} is newer than this app understands")]
    UnsupportedVersion { version: u32 },
    #[error("config version field is malformed: {found}")]
    MalformedVersion { found: String },
    #[error("could not read or write config: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendConfig {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    120_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandConfig {
    pub id: String,
    pub name: String,
    pub backend_id: String,
    pub hotkey: String,
    pub system_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub backends: Vec<BackendConfig>,
    pub commands: Vec<CommandConfig>,
}

impl Config {
    pub fn backend(&self, id: &str) -> Option<&BackendConfig> {
        self.backends.iter().find(|b| b.id == id)
    }
}

pub fn parse(json: &str) -> Result<Config, ConfigError> {
    let mut value: serde_json::Value = serde_json::from_str(json)?;
    // A missing "version" key is a defensible default of 1 (a v1 file might
    // predate the field), but a key that is *present* and not a
    // non-negative integer -- a string, null, a negative number, a float --
    // must not silently take the v1 migration path. That would bypass the
    // "reject unknown versions rather than guess" guarantee for what might
    // be a corrupted or hand-mangled future-version file.
    let version = match value.get("version") {
        None => 1u32,
        Some(v) => match v.as_u64() {
            Some(n) => n as u32,
            None => return Err(ConfigError::MalformedVersion { found: v.to_string() }),
        },
    };

    match version {
        1 => {
            // v1 called them prompts. Move the key, then type it as v2.
            if let Some(obj) = value.as_object_mut() {
                if let Some(prompts) = obj.remove("prompts") {
                    obj.insert("commands".to_string(), prompts);
                }
                obj.insert("version".to_string(), serde_json::json!(CURRENT_VERSION));
            }
        }
        v if v == CURRENT_VERSION => {}
        other => return Err(ConfigError::UnsupportedVersion { version: other }),
    }

    let config: Config = serde_json::from_value(value)?;
    validate(&config)?;
    Ok(config)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    let mut ids = HashSet::new();
    for backend in &config.backends {
        if !ids.insert(&backend.id) {
            return Err(ConfigError::DuplicateId { id: backend.id.clone() });
        }
        match backend.kind.as_str() {
            KIND_OPENAI | KIND_ANTHROPIC | KIND_CLAUDE_CLI => {}
            other => return Err(ConfigError::UnknownKind { kind: other.to_string() }),
        }
    }

    let mut command_ids = HashSet::new();
    let mut hotkeys = HashSet::new();
    for command in &config.commands {
        if !command_ids.insert(&command.id) {
            return Err(ConfigError::DuplicateId { id: command.id.clone() });
        }
        if !hotkeys.insert(command.hotkey.to_lowercase()) {
            return Err(ConfigError::DuplicateHotkey { hotkey: command.hotkey.clone() });
        }
        if config.backend(&command.backend_id).is_none() {
            return Err(ConfigError::UnknownBackend {
                command: command.id.clone(),
                backend: command.backend_id.clone(),
            });
        }
    }
    Ok(())
}

pub fn default_config() -> Config {
    Config {
        version: CURRENT_VERSION,
        backends: vec![
            BackendConfig {
                id: "ollama-local".into(),
                name: "Local Ollama".into(),
                kind: KIND_OPENAI.into(),
                base_url: Some("http://localhost:11434/v1".into()),
                model: Some("qwen2.5:32b-instruct".into()),
                api_key: None,
                timeout_ms: 120_000,
            },
            BackendConfig {
                id: "claude-sub".into(),
                name: "Claude (subscription)".into(),
                kind: KIND_CLAUDE_CLI.into(),
                base_url: None,
                model: None,
                api_key: None,
                timeout_ms: 120_000,
            },
        ],
        commands: vec![CommandConfig {
            id: "fix-grammar".into(),
            name: "Fix grammar".into(),
            backend_id: "ollama-local".into(),
            hotkey: "Ctrl+Alt+J".into(),
            system_prompt: "Fix grammar and spelling.".into(),
        }],
    }
}

pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(&default_config())?)?;
    }
    let raw = fs::read_to_string(path)?;
    let config = parse(&raw)?;
    if !raw.contains("\"commands\"") {
        // Migrated from v1 — persist it so this happens once. A failure
        // here (locked file, disk full, permission denied, an antivirus or
        // cloud-sync tool holding a handle -- all routine on Windows) must
        // never lose a config that already parsed successfully one line
        // above: the rewrite is only a once-off optimisation so the
        // migration doesn't re-run on every launch, not a precondition for
        // returning the config at all. Log and carry on with `config`
        // either way; the migration will simply be retried next launch.
        if let Err(e) = write_atomic(path, &config) {
            let line = format!("CONFIG migration-rewrite-failed reason={e}");
            eprintln!("{line}");
            if let Some(dir) = path.parent() {
                crate::status::append_log(dir, &line);
            }
        }
    }
    Ok(config)
}

/// Write `config` to `path` atomically: serialize to a sibling `.tmp` file,
/// then `rename` it over `path`. A same-directory rename is atomic on
/// Windows (and POSIX) -- unlike `fs::write`, which opens the destination
/// with truncation and can leave a half-written or empty file behind if the
/// write fails partway through, a failed rename leaves the original file
/// completely untouched. The temp file is best-effort cleaned up on any
/// failure; its own removal failing is not itself an error worth reporting.
fn write_atomic(path: &Path, config: &Config) -> Result<(), ConfigError> {
    let tmp_name = match path.file_name() {
        Some(name) => format!("{}.tmp", name.to_string_lossy()),
        None => "config.json.tmp".to_string(),
    };
    let tmp_path = path.with_file_name(tmp_name);

    let json = serde_json::to_string_pretty(config)?;
    if let Err(e) = fs::write(&tmp_path, json) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}

pub fn config_path(app: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    app.path()
        .app_config_dir()
        .expect("app config dir")
        .join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_json() -> &'static str {
        r#"{
          "version": 2,
          "backends": [
            {"id":"ollama","name":"Local","kind":"openai-compatible",
             "baseUrl":"http://localhost:11434/v1","model":"qwen2.5:32b-instruct",
             "apiKey":null,"timeoutMs":120000}
          ],
          "commands": [
            {"id":"fix","name":"Fix grammar","backendId":"ollama",
             "hotkey":"Ctrl+Alt+J","systemPrompt":"Fix grammar."}
          ]
        }"#
    }

    fn v1_json() -> &'static str {
        r#"{
          "version": 1,
          "backends": [
            {"id":"ollama","name":"Local","kind":"openai-compatible",
             "baseUrl":"http://localhost:11434/v1","model":"m","apiKey":null,"timeoutMs":120000}
          ],
          "prompts": [
            {"id":"fix","name":"Fix grammar","backendId":"ollama",
             "hotkey":"Ctrl+Alt+J","systemPrompt":"Fix grammar."}
          ]
        }"#
    }

    #[test]
    fn parses_a_valid_config() {
        let cfg = parse(valid_json()).expect("should parse");
        assert_eq!(cfg.backends.len(), 1);
        assert_eq!(cfg.commands[0].backend_id, "ollama");
        assert_eq!(cfg.backends[0].timeout_ms, 120_000);
    }

    #[test]
    fn rejects_a_command_naming_an_unknown_backend() {
        let json = valid_json().replace(r#""backendId":"ollama""#, r#""backendId":"nope""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownBackend { .. })));
    }

    #[test]
    fn rejects_duplicate_backend_ids() {
        let json = valid_json().replace(
            r#""commands""#,
            r#""_unused":0,"commands""#,
        );
        // duplicate the single backend entry
        let json = json.replace(
            r#""apiKey":null,"timeoutMs":120000}"#,
            r#""apiKey":null,"timeoutMs":120000},
               {"id":"ollama","name":"Dup","kind":"openai-compatible",
                "baseUrl":"http://x/v1","model":"m","apiKey":null,"timeoutMs":1000}"#,
        );
        assert!(matches!(parse(&json), Err(ConfigError::DuplicateId { .. })));
    }

    #[test]
    fn rejects_duplicate_hotkeys() {
        let json = valid_json().replace(
            r#""systemPrompt":"Fix grammar."}"#,
            r#""systemPrompt":"Fix grammar."},
               {"id":"other","name":"Other","backendId":"ollama",
                "hotkey":"Ctrl+Alt+J","systemPrompt":"Other."}"#,
        );
        assert!(matches!(parse(&json), Err(ConfigError::DuplicateHotkey { .. })));
    }

    #[test]
    fn rejects_an_unknown_backend_kind() {
        let json = valid_json().replace(r#""kind":"openai-compatible""#, r#""kind":"telepathy""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownKind { .. })));
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(parse("{nonsense"), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn the_default_config_is_valid() {
        let json = serde_json::to_string(&default_config()).unwrap();
        parse(&json).expect("default config must round-trip through validation");
    }

    #[test]
    fn migrates_a_v1_file_by_reading_prompts_as_commands() {
        let cfg = parse(v1_json()).expect("v1 should migrate");
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert_eq!(cfg.commands.len(), 1);
        assert_eq!(cfg.commands[0].id, "fix");
        assert_eq!(cfg.commands[0].hotkey, "Ctrl+Alt+J");
    }

    #[test]
    fn a_migrated_v1_config_is_still_validated() {
        let json = v1_json().replace(r#""backendId":"ollama""#, r#""backendId":"nope""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownBackend { .. })));
    }

    #[test]
    fn rejects_a_version_from_the_future() {
        // valid_json() is a v2 fixture ("version": 2); replacing a
        // "version": 1 literal here would silently no-op and this test
        // would fail for the wrong reason (an Ok instead of an Err).
        let json = valid_json().replace(r#""version": 2"#, r#""version": 99"#);
        assert!(matches!(parse(&json), Err(ConfigError::UnsupportedVersion { version: 99 })));
    }

    #[test]
    fn a_v2_file_round_trips_unchanged() {
        let cfg = parse(&serde_json::to_string(&default_config()).unwrap()).unwrap();
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert_eq!(cfg.commands.len(), 1);
    }

    // I1: a failed migration rewrite must never discard a config that
    // already parsed successfully. Mark the on-disk file read-only so the
    // rewrite attempt inside `load_or_create` fails, then assert the
    // command survives anyway.
    #[test]
    fn a_migration_rewrite_failure_still_returns_the_parsed_config() {
        let dir = std::env::temp_dir().join(format!(
            "cai-test-rewrite-fail-{}-{}",
            std::process::id(),
            "a"
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        fs::write(&path, v1_json()).unwrap();

        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).unwrap();

        let result = load_or_create(&path);

        // Restore write access before cleanup regardless of the outcome,
        // so a failing assertion below doesn't leak a read-only temp dir.
        let mut perms2 = fs::metadata(&path).unwrap().permissions();
        perms2.set_readonly(false);
        let _ = fs::set_permissions(&path, perms2);
        let _ = fs::remove_dir_all(&dir);

        let cfg = result.expect(
            "a failed migration rewrite must not lose an already-parsed config",
        );
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert_eq!(cfg.commands.len(), 1);
        assert_eq!(cfg.commands[0].id, "fix");
    }

    // I1 part 2: the rewrite must be atomic -- a failure must never leave
    // the destination truncated or partially written. Force the rename
    // step specifically to fail (destination read-only) and confirm the
    // pre-existing destination content is completely untouched.
    #[test]
    fn write_atomic_leaves_the_destination_untouched_on_failure() {
        let dir = std::env::temp_dir().join(format!(
            "cai-test-atomic-{}-{}",
            std::process::id(),
            "b"
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let original = "not valid config json, just a marker of pre-existing content";
        fs::write(&path, original).unwrap();

        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).unwrap();

        let result = write_atomic(&path, &default_config());

        let mut perms2 = fs::metadata(&path).unwrap().permissions();
        perms2.set_readonly(false);
        let _ = fs::set_permissions(&path, perms2);
        let on_disk = fs::read_to_string(&path).unwrap();
        let tmp_leaked = dir.join("config.json.tmp").exists();
        let _ = fs::remove_dir_all(&dir);

        assert!(result.is_err(), "expected the rename-over-read-only-destination to fail");
        assert_eq!(
            on_disk, original,
            "a failed atomic write must never modify the pre-existing destination"
        );
        assert!(!tmp_leaked, "the temp file should be cleaned up after a failed rename");
    }

    // I2: a "version" key that is present but not a non-negative integer
    // must not silently be treated as v1 -- see missing vs malformed below.
    fn missing_version_json() -> &'static str {
        r#"{
          "backends": [
            {"id":"ollama","name":"Local","kind":"openai-compatible",
             "baseUrl":"http://localhost:11434/v1","model":"m","apiKey":null,"timeoutMs":120000}
          ],
          "prompts": [
            {"id":"fix","name":"Fix grammar","backendId":"ollama",
             "hotkey":"Ctrl+Alt+J","systemPrompt":"Fix grammar."}
          ]
        }"#
    }

    #[test]
    fn a_missing_version_key_migrates_as_v1() {
        let cfg = parse(missing_version_json())
            .expect("a missing version key should default to v1 and migrate");
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert_eq!(cfg.commands.len(), 1);
        assert_eq!(cfg.commands[0].id, "fix");
    }

    #[test]
    fn rejects_a_string_version_instead_of_silently_migrating() {
        let json = valid_json().replace(r#""version": 2"#, r#""version": "2""#);
        assert!(matches!(parse(&json), Err(ConfigError::MalformedVersion { .. })));
    }

    #[test]
    fn rejects_a_null_version_instead_of_silently_migrating() {
        let json = valid_json().replace(r#""version": 2"#, r#""version": null"#);
        assert!(matches!(parse(&json), Err(ConfigError::MalformedVersion { .. })));
    }
}
