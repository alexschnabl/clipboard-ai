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
    #[error(
        "backend {backend} has timeoutMs {timeout_ms}, below the {minimum} ms minimum; \
         the slowest measured warm backend takes about 4,100 ms and a cold local model \
         about 16,000 ms, so a lower value only causes false timeouts"
    )]
    TimeoutTooLow { backend: String, timeout_ms: u64, minimum: u64 },
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
    #[serde(default)]
    pub effort: Effort,
}

pub const MIN_TIMEOUT_MS: u64 = 5_000;

fn default_timeout() -> u64 {
    120_000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    System,
    En,
    De,
    Fr,
    Es,
    It,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Default,
    #[default]
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub autostart: bool,
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
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub backends: Vec<BackendConfig>,
    pub commands: Vec<CommandConfig>,
    #[serde(default)]
    pub settings: Settings,
}

impl Config {
    pub fn backend(&self, id: &str) -> Option<&BackendConfig> {
        self.backends.iter().find(|b| b.id == id)
    }
}

pub fn parse(json: &str) -> Result<Config, ConfigError> {
    let mut value: serde_json::Value = serde_json::from_str(json)?;
    let version = match value.get("version") {
        None => 1u32,
        Some(v) => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(n) => n,
            None => return Err(ConfigError::MalformedVersion { found: v.to_string() }),
        },
    };

    match version {
        1 => {
            if let Some(obj) = value.as_object_mut() {
                match obj.remove("prompts") {
                    Some(prompts) => {
                        obj.insert("commands".to_string(), prompts);
                    }
                    None => {
                        if !obj.contains_key("commands") {
                            obj.insert("commands".to_string(), serde_json::json!([]));
                        }
                    }
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
        if backend.timeout_ms < MIN_TIMEOUT_MS {
            return Err(ConfigError::TimeoutTooLow {
                backend: backend.id.clone(),
                timeout_ms: backend.timeout_ms,
                minimum: MIN_TIMEOUT_MS,
            });
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
                effort: Effort::Low,
            },
            BackendConfig {
                id: "claude-sub".into(),
                name: "Claude (subscription)".into(),
                kind: KIND_CLAUDE_CLI.into(),
                base_url: None,
                model: None,
                api_key: None,
                timeout_ms: 120_000,
                effort: Effort::Low,
            },
        ],
        commands: vec![CommandConfig {
            id: "fix-grammar".into(),
            name: "Fix grammar".into(),
            backend_id: "ollama-local".into(),
            hotkey: "Ctrl+Alt+J".into(),
            system_prompt: "Fix grammar and spelling.".into(),
        }],
        settings: Settings::default(),
    }
}

pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic(path, &default_config())?;
    }
    let raw = fs::read_to_string(path)?;
    let config = parse(&raw)?;
    if !raw.contains("\"commands\"") {
        let line = format!(
            "STARTUP config-migrated-v1-to-v2 backends={} commands={}",
            config.backends.len(),
            config.commands.len()
        );
        eprintln!("{line}");
        if let Some(dir) = path.parent() {
            crate::status::append_log(dir, &line);
        }
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

pub(crate) fn write_atomic(path: &Path, config: &Config) -> Result<(), ConfigError> {
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

pub(crate) fn preserve_unreadable(path: &Path) -> std::io::Result<PathBuf> {
    let base = match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => "config.json".to_string(),
    };
    let mut candidate = path.with_file_name(format!("{base}.corrupt"));
    let mut suffix = 2;
    while candidate.exists() {
        if suffix > 1000 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "no free <name>.corrupt[.N] slot found",
            ));
        }
        candidate = path.with_file_name(format!("{base}.corrupt.{suffix}"));
        suffix += 1;
    }
    fs::rename(path, &candidate)?;
    Ok(candidate)
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

    fn json_with_settings(settings: serde_json::Value) -> String {
        let mut value: serde_json::Value = serde_json::from_str(valid_json()).unwrap();
        value.as_object_mut().unwrap().insert("settings".to_string(), settings);
        serde_json::to_string(&value).unwrap()
    }

    #[test]
    fn a_config_with_no_settings_key_loads_with_defaults() {
        let cfg = parse(valid_json()).expect("a config with no settings key should still parse");
        assert_eq!(cfg.settings.theme, Theme::System);
        assert_eq!(cfg.settings.language, Language::System);
        assert!(!cfg.settings.autostart);
    }

    #[test]
    fn partial_settings_fill_in_defaults_for_the_missing_fields() {
        let json = json_with_settings(serde_json::json!({"theme": "dark"}));
        let cfg = parse(&json).expect("partial settings should still parse");
        assert_eq!(cfg.settings.theme, Theme::Dark);
        assert_eq!(cfg.settings.language, Language::System);
        assert!(!cfg.settings.autostart);
    }

    #[test]
    fn rejects_an_unknown_theme_value() {
        let json = json_with_settings(serde_json::json!({"theme": "rainbow"}));
        assert!(matches!(parse(&json), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn rejects_an_unknown_language_value() {
        let json = json_with_settings(serde_json::json!({"language": "not-a-language"}));
        assert!(matches!(parse(&json), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn accepts_every_shipped_language() {
        for code in ["system", "en", "de", "fr", "es", "it"] {
            let json = json_with_settings(serde_json::json!({ "language": code }));
            assert!(parse(&json).is_ok(), "{code} must be a legal language value");
        }
    }

    #[test]
    fn a_backend_with_no_effort_key_keeps_todays_behaviour() {
        let cfg = parse(valid_json()).expect("a backend missing this key should still parse");
        assert_eq!(cfg.backends[0].effort, Effort::Low);
    }

    #[test]
    fn rejects_an_unknown_effort_value() {
        let mut value: serde_json::Value = serde_json::from_str(valid_json()).unwrap();
        value["backends"][0]["effort"] = serde_json::json!("turbo");
        let json = serde_json::to_string(&value).unwrap();
        assert!(matches!(parse(&json), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn a_config_written_by_this_version_round_trips_its_settings_intact() {
        let mut cfg = default_config();
        cfg.settings.theme = Theme::Dark;
        cfg.settings.language = Language::De;
        cfg.settings.autostart = true;

        let json = serde_json::to_string(&cfg).unwrap();
        let parsed = parse(&json).expect("a config written by this version must parse");

        assert_eq!(parsed.version, CURRENT_VERSION);
        assert_eq!(parsed.settings.theme, Theme::Dark);
        assert_eq!(parsed.settings.language, Language::De);
        assert!(parsed.settings.autostart);
    }

    #[test]
    fn theme_and_language_serialize_to_the_expected_lowercase_wire_values() {
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Theme::Light).unwrap(), "\"light\"");
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
        assert_eq!(serde_json::to_string(&Language::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Language::En).unwrap(), "\"en\"");
        assert_eq!(serde_json::to_string(&Language::De).unwrap(), "\"de\"");
    }

    #[test]
    fn rejects_every_malformed_version_value_instead_of_silently_migrating() {
        for bad_version in ["4294967297", "-1", "1.5", "\"2\"", "null"] {
            let json =
                valid_json().replace("\"version\": 2", &format!("\"version\": {bad_version}"));
            assert!(
                matches!(parse(&json), Err(ConfigError::MalformedVersion { .. })),
                "expected {bad_version} to be rejected as malformed"
            );
        }
    }

    #[test]
    fn migrates_a_v1_file_that_has_no_prompts_key_at_all() {
        let json = r#"{"version": 1,
            "backends": [{"id":"b","name":"B","kind":"claude-cli",
            "baseUrl":null,"model":null,"apiKey":null,"timeoutMs":120000}]}"#;
        let cfg = parse(json).expect("a promptless v1 file must migrate, not be rejected");
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert!(cfg.commands.is_empty());
        assert_eq!(cfg.backends.len(), 1);
    }

    #[test]
    fn rejects_a_timeout_below_the_floor_naming_the_backend_and_the_minimum() {
        let json = valid_json().replace(r#""timeoutMs":120000"#, r#""timeoutMs":1"#);
        let err = parse(&json).expect_err("a 1 ms timeout must be rejected");
        assert!(
            matches!(err, ConfigError::TimeoutTooLow { timeout_ms: 1, .. }),
            "got: {err:?}"
        );
        let message = err.to_string();
        assert!(message.contains("ollama"), "must name the backend, got: {message}");
        assert!(message.contains("5000"), "must state the minimum, got: {message}");
    }

    #[test]
    fn a_backend_omitting_timeout_ms_entirely_gets_the_default_and_passes_the_floor() {
        let json = valid_json().replace(r#","timeoutMs":120000"#, "");
        let cfg = parse(&json).expect("an omitted timeoutMs must fall back to the default");
        assert_eq!(cfg.backends[0].timeout_ms, default_timeout());
        assert!(cfg.backends[0].timeout_ms >= MIN_TIMEOUT_MS);
    }

    #[test]
    fn accepts_a_timeout_exactly_at_the_floor() {
        let json = valid_json()
            .replace(r#""timeoutMs":120000"#, &format!(r#""timeoutMs":{MIN_TIMEOUT_MS}"#));
        let cfg = parse(&json).expect("the floor itself must be a legal value");
        assert_eq!(cfg.backends[0].timeout_ms, MIN_TIMEOUT_MS);
    }

    #[test]
    fn the_default_timeout_comfortably_exceeds_a_cold_local_model_start() {
        assert!(
            default_timeout() >= 16_036 * 2,
            "default {} ms leaves too little headroom over a 16,036 ms cold start",
            default_timeout()
        );
    }

    #[test]
    fn rejects_a_command_naming_an_unknown_backend() {
        let json = valid_json().replace(r#""backendId":"ollama""#, r#""backendId":"nope""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownBackend { .. })));
    }

    #[test]
    fn rejects_duplicate_backend_ids() {
        let json = valid_json().replace(
            r#""apiKey":null,"timeoutMs":120000}"#,
            r#""apiKey":null,"timeoutMs":120000},
               {"id":"ollama","name":"Dup","kind":"openai-compatible",
                "baseUrl":"http://x/v1","model":"m","apiKey":null,"timeoutMs":120000}"#,
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
    fn rejects_duplicate_hotkeys_that_differ_only_in_case() {
        let json = valid_json().replace(
            r#""systemPrompt":"Fix grammar."}"#,
            r#""systemPrompt":"Fix grammar."},
               {"id":"other","name":"Other","backendId":"ollama",
                "hotkey":"ctrl+alt+j","systemPrompt":"Other."}"#,
        );
        assert!(matches!(parse(&json), Err(ConfigError::DuplicateHotkey { .. })));
    }

    #[test]
    fn rejects_duplicate_command_ids() {
        let json = valid_json().replace(
            r#""systemPrompt":"Fix grammar."}"#,
            r#""systemPrompt":"Fix grammar."},
               {"id":"fix","name":"Other","backendId":"ollama",
                "hotkey":"Ctrl+Alt+K","systemPrompt":"Other."}"#,
        );
        assert!(matches!(parse(&json), Err(ConfigError::DuplicateId { .. })));
    }

    #[test]
    fn rejects_an_unknown_backend_kind() {
        let json = valid_json().replace(r#""kind":"openai-compatible""#, r#""kind":"telepathy""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownKind { .. })));
    }

    #[test]
    fn accepts_anthropic_api_as_a_backend_kind() {
        let json =
            valid_json().replace(r#""kind":"openai-compatible""#, r#""kind":"anthropic-api""#);
        assert!(parse(&json).is_ok(), "anthropic-api must be a legal backend kind");
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
        let json = valid_json().replace(r#""version": 2"#, r#""version": 99"#);
        assert!(matches!(parse(&json), Err(ConfigError::UnsupportedVersion { version: 99 })));
    }

    #[test]
    fn a_v2_file_round_trips_unchanged() {
        let cfg = parse(&serde_json::to_string(&default_config()).unwrap()).unwrap();
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert_eq!(cfg.commands.len(), 1);
    }

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

    #[test]
    fn a_successful_v1_to_v2_migration_is_logged() {
        let dir = std::env::temp_dir().join(format!(
            "cai-test-migration-logged-{}-{}",
            std::process::id(),
            "a"
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        fs::write(&path, v1_json()).unwrap();

        let cfg = load_or_create(&path).expect("v1 file should migrate");
        assert_eq!(cfg.version, CURRENT_VERSION);

        let log = fs::read_to_string(dir.join("clipboard-ai.log"))
            .expect("a migration should have logged something");
        assert!(
            log.contains("config-migrated-v1-to-v2"),
            "expected a migration log line, got: {log}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_native_v2_file_does_not_log_a_migration() {
        let dir = std::env::temp_dir().join(format!(
            "cai-test-no-migration-logged-{}-{}",
            std::process::id(),
            "a"
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        fs::write(&path, valid_json()).unwrap();

        load_or_create(&path).expect("v2 file should load");

        let log_path = dir.join("clipboard-ai.log");
        let log = fs::read_to_string(&log_path).unwrap_or_default();
        assert!(
            !log.contains("config-migrated-v1-to-v2"),
            "a native v2 file must not be reported as migrated, got: {log}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

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

    #[test]
    fn load_or_create_writes_a_valid_default_config_on_first_run() {
        let dir = std::env::temp_dir().join(format!(
            "cai-test-first-run-{}-{}",
            std::process::id(),
            "a"
        ));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("config.json");

        let cfg = load_or_create(&path).expect("first run should succeed");

        assert_eq!(cfg.commands.len(), 1);
        assert!(path.exists(), "a default config.json should now exist on disk");
        assert!(!dir.join("config.json.tmp").exists(), "no leftover temp file");
        let raw = fs::read_to_string(&path).unwrap();
        parse(&raw).expect("the written file must itself be valid, parseable config");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn preserve_unreadable_moves_the_file_aside_intact() {
        let dir = std::env::temp_dir().join(format!(
            "cai-test-preserve-{}-{}",
            std::process::id(),
            "a"
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let original = "{ this is not valid json, trailing comma etc, }";
        fs::write(&path, original).unwrap();

        let preserved = preserve_unreadable(&path).expect("preserve should succeed");

        assert!(!path.exists(), "the original path must no longer hold the unreadable file");
        assert_eq!(preserved, dir.join("config.json.corrupt"));
        assert_eq!(
            fs::read_to_string(&preserved).unwrap(),
            original,
            "the preserved copy must be byte-for-byte the original content"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn preserve_unreadable_does_not_clobber_an_earlier_preserved_copy() {
        let dir = std::env::temp_dir().join(format!(
            "cai-test-preserve-{}-{}",
            std::process::id(),
            "b"
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        fs::write(&path, "first broken file").unwrap();
        let first_preserved = preserve_unreadable(&path).expect("first preserve should succeed");

        fs::write(&path, "second broken file").unwrap();
        let second_preserved =
            preserve_unreadable(&path).expect("second preserve should succeed");

        assert_ne!(
            first_preserved, second_preserved,
            "the second preserve must not reuse the first's destination"
        );
        assert_eq!(
            fs::read_to_string(&first_preserved).unwrap(),
            "first broken file",
            "the first preserved copy must survive the second preserve untouched"
        );
        assert_eq!(fs::read_to_string(&second_preserved).unwrap(), "second broken file");

        let _ = fs::remove_dir_all(&dir);
    }

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

}
