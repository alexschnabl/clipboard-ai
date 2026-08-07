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

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config is not valid JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("prompt {prompt} names unknown backend {backend}")]
    UnknownBackend { prompt: String, backend: String },
    #[error("duplicate id: {id}")]
    DuplicateId { id: String },
    #[error("duplicate hotkey: {hotkey}")]
    DuplicateHotkey { hotkey: String },
    #[error("unknown backend kind: {kind}")]
    UnknownKind { kind: String },
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
pub struct PromptConfig {
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
    pub prompts: Vec<PromptConfig>,
}

impl Config {
    pub fn backend(&self, id: &str) -> Option<&BackendConfig> {
        self.backends.iter().find(|b| b.id == id)
    }
}

pub fn parse(json: &str) -> Result<Config, ConfigError> {
    let config: Config = serde_json::from_str(json)?;
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

    let mut prompt_ids = HashSet::new();
    let mut hotkeys = HashSet::new();
    for prompt in &config.prompts {
        if !prompt_ids.insert(&prompt.id) {
            return Err(ConfigError::DuplicateId { id: prompt.id.clone() });
        }
        if !hotkeys.insert(prompt.hotkey.to_lowercase()) {
            return Err(ConfigError::DuplicateHotkey { hotkey: prompt.hotkey.clone() });
        }
        if config.backend(&prompt.backend_id).is_none() {
            return Err(ConfigError::UnknownBackend {
                prompt: prompt.id.clone(),
                backend: prompt.backend_id.clone(),
            });
        }
    }
    Ok(())
}

pub fn default_config() -> Config {
    Config {
        version: 1,
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
        prompts: vec![PromptConfig {
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
        let json = serde_json::to_string_pretty(&default_config())?;
        fs::write(path, json)?;
    }
    parse(&fs::read_to_string(path)?)
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
          "version": 1,
          "backends": [
            {"id":"ollama","name":"Local","kind":"openai-compatible",
             "baseUrl":"http://localhost:11434/v1","model":"qwen2.5:32b-instruct",
             "apiKey":null,"timeoutMs":120000}
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
        assert_eq!(cfg.prompts[0].backend_id, "ollama");
        assert_eq!(cfg.backends[0].timeout_ms, 120_000);
    }

    #[test]
    fn rejects_a_prompt_naming_an_unknown_backend() {
        let json = valid_json().replace(r#""backendId":"ollama""#, r#""backendId":"nope""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownBackend { .. })));
    }

    #[test]
    fn rejects_duplicate_backend_ids() {
        let json = valid_json().replace(
            r#""prompts""#,
            r#""_unused":0,"prompts""#,
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
}
