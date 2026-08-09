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
    /// `claude-cli`-only: the CLI's own `--effort` flag. Meaningless for the
    /// two HTTP kinds, same as `model` is meaningless for neither -- see
    /// `Effort`'s own doc comment for why `Low` (not `Effort::Default`) is
    /// this field's serde default.
    #[serde(default)]
    pub effort: Effort,
    /// `claude-cli`-only: whether the CLI should keep this run in its own
    /// session history. Defaults to `false` (matches the CLI adapter's
    /// historical hardcoded `--no-session-persistence`) so an existing
    /// claude-cli backend's config, which predates this field, keeps
    /// behaving exactly as it does today.
    #[serde(default)]
    pub session_persistence: bool,
}

/// The lowest `timeoutMs` a backend may be configured with.
///
/// A timeout is a failure bound, not a latency target: lowering it does not
/// make a response arrive sooner, it only converts slow successes into
/// failures. R16: this floor guarantees exactly one thing, from the
/// project's own latency measurements -- below it, even a *warm* backend
/// cannot succeed. The fastest measured warm response was
/// `claude -p` at 3,290-4,126 ms, so anything under 5 s is a value that
/// could only ever produce a false timeout, on any backend, under any
/// conditions. It does *not* guarantee protection against a cold local
/// model: that took 16,036 ms in the same measurements, well above this
/// floor, so a user who configures exactly `MIN_TIMEOUT_MS` on a
/// cold-starting Ollama backend will still hit the false timeout this floor
/// does not catch -- only the much higher default (`default_timeout`,
/// 120,000 ms) is sized for that case.
///
/// Enforced here rather than only in the settings UI because hand-editing
/// `config.json` remains a supported path, and validation is meant to have
/// exactly one home.
pub const MIN_TIMEOUT_MS: u64 = 5_000;

fn default_timeout() -> u64 {
    120_000
}

/// Appearance the frontend should render. `System` means "follow the OS
/// theme" -- detecting *what* the OS theme is stays a frontend concern (this
/// struct only persists the choice), the same division of labour as
/// `Language::System` below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// UI language. `System` means "detect the OS locale, fall back to
/// English" -- like `Theme::System`, the detection itself is the frontend's
/// job; this only persists which language the user chose.
///
/// Stays a plain enum -- one variant per language -- rather than switching
/// to `BackendConfig.kind`'s raw-`String`-plus-`validate()` style as the
/// frontend's `src/lib/i18n/registry.ts` grows past two entries. That
/// switch was considered and rejected: `kind`'s style exists because
/// backend kinds need a `ConfigError::UnknownKind` variant *and* a
/// `validate()` branch, written by hand, to reject a bad value at all --
/// serde alone doesn't know the legal set for a raw `String` field. A
/// language, like `Theme`, is a fixed, developer-curated set the same way
/// `kind` is not (nothing here ever reads a language code from outside the
/// binary), so serde already rejects a value absent from this enum as
/// `ConfigError::Parse`, for free, via `#[serde(rename_all = "lowercase")]`
/// -- for *any* number of variants, not just two or three. Switching to a
/// string would trade that compiler-checked, free-rejecting enum for a
/// runtime lookup this file would have to keep in sync with the registry by
/// hand, which is strictly more code for an identical guarantee. Adding a
/// language is therefore one variant line here (matched by one entry in
/// `registry.ts`) either way -- and, because this only ever *adds* a legal
/// value to an already-`#[serde(default)]` field, never changes the
/// `Config`/`Settings` shape itself, it needs no `CURRENT_VERSION` bump
/// (see the comment on `Config::settings` for why bumping that constant
/// without a matching migration arm would be actively harmful).
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

/// The `--effort` level `claude_cli::ClaudeCli` passes to `claude -p`.
/// `Default` means "pass no `--effort` flag at all, let the CLI pick its own"
/// -- a real, user-chosen option, and *not* the same thing as this enum's
/// own `#[default]`.
///
/// `Low` carries `#[default]` instead, precisely because it is not the
/// intuitive choice: the CLI adapter has always hardcoded `--effort low`, so
/// an existing claude-cli backend's config -- written before this field
/// existed, and therefore missing the `effort` key entirely -- must keep
/// invoking the CLI exactly as it does today after this field is
/// introduced. `#[serde(default)]` on `BackendConfig::effort` reads that
/// missing key as `Effort::default()`, so that default has to be `Low`, not
/// the `Default` variant, or every pre-existing config would silently
/// switch behaviour the moment this field shipped.
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

/// Plain enums, not `BackendConfig.kind`'s raw-`String`-plus-`validate()`
/// style: for a fixed, developer-curated value set, an unknown value
/// already becomes a `ConfigError::Parse` for free via serde, with no extra
/// `ConfigError` variant or `validate()` branch needed -- less code, not
/// more, for this shape of field. See `Language`'s own doc comment for why
/// this holds regardless of how many languages that enum ends up with.
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

// Every field here is currently a single lowercase word, so this attribute
// is a no-op today -- it's added defensively, to match every sibling wire
// struct (`BackendConfig`, `CommandConfig`, ...), so a future multi-word
// field can't silently serialize as snake_case and break the frontend
// contract with no compiler warning.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub backends: Vec<BackendConfig>,
    pub commands: Vec<CommandConfig>,
    /// Additive since it was introduced: stays `#[serde(default)]` rather
    /// than forcing a `CURRENT_VERSION` bump. See `parse`'s version dispatch
    /// -- it accepts exactly `1` or `CURRENT_VERSION` and hard-rejects every
    /// other value as `UnsupportedVersion`, so bumping the constant without
    /// a matching new migration arm would reject every existing user's
    /// on-disk config on their very next launch. A purely-additive field
    /// with sensible defaults is exactly what `#[serde(default)]` (here) and
    /// serde_json's default ignore-unknown-fields behaviour (on the read
    /// side, for an older binary reading a newer file) already handle
    /// without any of that.
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
    // A missing "version" key is a defensible default of 1 (a v1 file might
    // predate the field), but a key that is *present* and not a
    // non-negative integer -- a string, null, a negative number, a float --
    // must not silently take the v1 migration path. That would bypass the
    // "reject unknown versions rather than guess" guarantee for what might
    // be a corrupted or hand-mangled future-version file.
    let version = match value.get("version") {
        None => 1u32,
        Some(v) => match v.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(n) => n,
            // `try_from`, not `as`: the old truncating cast meant a version
            // above u32::MAX wrapped -- 4294967297 became 1 and silently
            // took the v1 migration path, bypassing exactly the
            // "reject unknown versions rather than guess" guarantee the
            // comment above commits to. Out of range is malformed, same as
            // a string or a float.
            None => return Err(ConfigError::MalformedVersion { found: v.to_string() }),
        },
    };

    match version {
        1 => {
            // v1 called them prompts. Move the key, then type it as v2.
            if let Some(obj) = value.as_object_mut() {
                match obj.remove("prompts") {
                    Some(prompts) => {
                        obj.insert("commands".to_string(), prompts);
                    }
                    // A v1 file with no "prompts" key at all -- hand-editing
                    // is a supported path, and "I deleted the whole array"
                    // is a legitimate state of it. `commands` has no serde
                    // default (a *v2* file missing it should still be
                    // rejected rather than guessed at), so the migration
                    // must supply the empty list itself or this file would
                    // be wrongly refused.
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
                session_persistence: false,
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
                session_persistence: false,
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
        // Same reasoning as the migration rewrite below: `fs::write` opens
        // the destination with truncation, so a failure partway through
        // (disk full, an AV/cloud-sync tool grabbing the handle) could
        // otherwise leave a half-written config.json behind. That's worse
        // than leaving nothing: the *next* launch would see `path.exists()`
        // as true, skip this first-run branch entirely, and hand the
        // truncated JSON to `parse` below as if it were a real, if corrupt,
        // user file. `write_atomic` either leaves a fully-written file or
        // none at all, so a failed first run just retries cleanly next time.
        write_atomic(path, &default_config())?;
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
        //
        // The spec says migration "happens once, silently, and is logged" —
        // silently means no UI prompt, not no log line. Log the successful
        // case too (same `STARTUP ...` style used elsewhere), not just the
        // rewrite-failure case below: without this line there is no way to
        // tell, after the fact, whether a given install's config.json was
        // ever on v1.
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

/// Write `config` to `path` atomically: serialize to a sibling `.tmp` file,
/// then `rename` it over `path`. A same-directory rename is atomic on
/// Windows (and POSIX) -- unlike `fs::write`, which opens the destination
/// with truncation and can leave a half-written or empty file behind if the
/// write fails partway through, a failed rename leaves the original file
/// completely untouched. The temp file is best-effort cleaned up on any
/// failure; its own removal failing is not itself an error worth reporting.
///
/// `pub(crate)`, not private: `commands::save_config` -- the settings
/// window's Save button, the single most-executed write path in the app --
/// needs this exact guarantee too, not just the once-off v1->v2 migration
/// rewrite below.
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

/// Moves an unreadable or unparseable config file aside instead of leaving
/// it where a later save (`commands::save_config`) would silently overwrite
/// it once the app falls back to an empty in-memory config -- see the C2
/// finding: hand-editing is v1's only configuration mechanism, so a
/// malformed-but-recoverable file (a stray trailing comma) is a likely
/// state, not an exotic one, and it must survive the fallback.
///
/// Picks the first free `<name>.corrupt[.N]` sibling rather than always the
/// bare `.corrupt` name, so a second failure -- this one recurring on a
/// later launch, or the user managing to break the *preserved* copy too --
/// can never clobber an earlier preserved copy. Every corrupt file the user
/// ever produces stays recoverable, not just the most recent one.
///
/// Caller's responsibility to decide what "unreadable" means and whether
/// `path` still exists at all; this only performs the move.
pub(crate) fn preserve_unreadable(path: &Path) -> std::io::Result<PathBuf> {
    let base = match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => "config.json".to_string(),
    };
    let mut candidate = path.with_file_name(format!("{base}.corrupt"));
    let mut suffix = 2;
    while candidate.exists() {
        if suffix > 1000 {
            // Not a realistic count to actually hit -- it would mean a
            // thousand prior unreadable configs never cleaned up -- but the
            // alternative to giving up here is picking a name that might
            // collide anyway, which is exactly the clobber this function
            // exists to prevent.
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

    // Builds on `valid_json()` via `serde_json::Value` rather than string
    // splicing -- `valid_json()`'s exact whitespace/bracket layout is
    // incidental, and a `Value`-level insert can't be broken by reformatting
    // that fixture later the way a brittle string `.replace` could.
    fn json_with_settings(settings: serde_json::Value) -> String {
        let mut value: serde_json::Value = serde_json::from_str(valid_json()).unwrap();
        value.as_object_mut().unwrap().insert("settings".to_string(), settings);
        serde_json::to_string(&value).unwrap()
    }

    // An existing config.json predating this feature has no "settings" key
    // at all -- `valid_json()` already has none -- and must load with every
    // default rather than fail to parse.
    #[test]
    fn a_config_with_no_settings_key_loads_with_defaults() {
        let cfg = parse(valid_json()).expect("a config with no settings key should still parse");
        assert_eq!(cfg.settings.theme, Theme::System);
        assert_eq!(cfg.settings.language, Language::System);
        assert!(!cfg.settings.autostart);
    }

    // A hand-edited or partially-migrated config.json might set only one of
    // the three fields -- the other two must still fall back to their
    // individual defaults, not fail to parse or zero out the whole object.
    #[test]
    fn partial_settings_fill_in_defaults_for_the_missing_fields() {
        let json = json_with_settings(serde_json::json!({"theme": "dark"}));
        let cfg = parse(&json).expect("partial settings should still parse");
        assert_eq!(cfg.settings.theme, Theme::Dark);
        assert_eq!(cfg.settings.language, Language::System);
        assert!(!cfg.settings.autostart);
    }

    // Prefer a real enum over a stringly-typed field precisely so this
    // case is free: an unknown value is rejected at parse time like any
    // other malformed config, with no dedicated `ConfigError` variant or
    // `validate()` branch needed.
    #[test]
    fn rejects_an_unknown_theme_value() {
        let json = json_with_settings(serde_json::json!({"theme": "rainbow"}));
        assert!(matches!(parse(&json), Err(ConfigError::Parse(_))));
    }

    // Deliberately not a real ISO code: this test previously used "fr",
    // which stopped being unknown the moment French shipped -- so it started
    // failing for the right reason, but it was pinning the wrong thing. A
    // string that can never become a language keeps testing "serde rejects
    // values outside the enum" rather than "we happen not to ship French".
    #[test]
    fn rejects_an_unknown_language_value() {
        let json = json_with_settings(serde_json::json!({"language": "not-a-language"}));
        assert!(matches!(parse(&json), Err(ConfigError::Parse(_))));
    }

    // The counterpart to the test above: every language the app actually
    // ships must round-trip. Adding a variant to `Language` without adding
    // its dictionary and registry entry on the frontend would otherwise be
    // caught nowhere on this side of the bridge.
    #[test]
    fn accepts_every_shipped_language() {
        for code in ["system", "en", "de", "fr", "es", "it"] {
            let json = json_with_settings(serde_json::json!({ "language": code }));
            assert!(parse(&json).is_ok(), "{code} must be a legal language value");
        }
    }

    // Pins today's behaviour: an existing claude-cli backend's config
    // predates `effort`/`sessionPersistence` entirely, and `valid_json()`'s
    // single backend already has neither key. It must load as `Effort::Low`
    // + `session_persistence == false` -- exactly what the CLI adapter used
    // to hardcode -- not silently switch to `Effort::Default` or `true`.
    #[test]
    fn a_backend_with_no_effort_or_session_persistence_keys_keeps_todays_behaviour() {
        let cfg = parse(valid_json()).expect("a backend missing these keys should still parse");
        assert_eq!(cfg.backends[0].effort, Effort::Low);
        assert!(!cfg.backends[0].session_persistence);
    }

    // Same free rejection `rejects_an_unknown_theme_value` and
    // `rejects_an_unknown_language_value` rely on: a real enum, not a
    // stringly-typed field, so an unknown `effort` value needs no dedicated
    // `ConfigError` variant or `validate()` branch to be refused.
    #[test]
    fn rejects_an_unknown_effort_value() {
        let mut value: serde_json::Value = serde_json::from_str(valid_json()).unwrap();
        value["backends"][0]["effort"] = serde_json::json!("turbo");
        let json = serde_json::to_string(&value).unwrap();
        assert!(matches!(parse(&json), Err(ConfigError::Parse(_))));
    }

    // A config written by *this* version (settings present, fully
    // populated) must parse cleanly and keep every settings value intact --
    // the ordinary round trip every save/reload cycle depends on.
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

    // `#[serde(rename_all = "lowercase")]` produces exactly these three
    // wire values -- pin them down explicitly so a future refactor of the
    // enum (renaming a variant, reordering) can't silently change the wire
    // contract `settings.svelte.ts` will be written against without a test
    // failing here first.
    #[test]
    fn theme_and_language_serialize_to_the_expected_lowercase_wire_values() {
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Theme::Light).unwrap(), "\"light\"");
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
        assert_eq!(serde_json::to_string(&Language::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Language::En).unwrap(), "\"en\"");
        assert_eq!(serde_json::to_string(&Language::De).unwrap(), "\"de\"");
    }

    // The settings UI enforces this floor too, but hand-editing config.json
    // is a supported path and would otherwise bypass it entirely -- the UI
    // check alone was cosmetic.
    // The old `as u32` cast wrapped this to exactly 1 and silently migrated
    // a file claiming a version in the billions -- bypassing the "reject
    // unknown versions rather than guess" guarantee for precisely the kind
    // of corrupted value it exists for.
    // Merged (was 5 tests: overflow, negative, float, string, null): each
    // is a "version" value that is present but not a non-negative u32, and
    // every one must be rejected rather than silently treated as v1 --
    // table-driven like `accepts_every_shipped_language` above.
    // The old `as u32` cast wrapped the overflow case to exactly 1 and
    // silently migrated a file claiming a version in the billions --
    // bypassing the "reject unknown versions rather than guess" guarantee
    // for precisely the kind of corrupted value it exists for.
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

    // Hand-editing is a supported path, and "I deleted the whole prompts
    // array" is a legitimate state of a v1 file. The migration must supply
    // the empty command list itself; `commands` deliberately has no serde
    // default, so without this arm the file would be wrongly rejected.
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

    // Merged: both parse the identical mutated JSON, one asserting the
    // error variant and the other its message text -- combined into one
    // test with both assertions.
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

    // A backend entry that omits `timeoutMs` entirely (not just sets it to
    // null) relies on `#[serde(default = "default_timeout")]` -- this must
    // still parse and land safely above MIN_TIMEOUT_MS, not just when the
    // key is explicitly present.
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

    // The measured cold start for a local model is 16,036 ms; the default
    // must leave generous headroom above that or every first request of the
    // day fails.
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
        // duplicate the single backend entry
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

    // The existing test above reuses the identical byte-for-byte hotkey
    // string; `validate` actually dedups via `.to_lowercase()`, which that
    // case can never exercise. A regression dropping the normalization
    // would pass every existing test without this one.
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
        // Only backend ids were covered before this -- command ids go
        // through the identical `HashSet::insert` check in `validate` and
        // deserve the same coverage.
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

    // KIND_ANTHROPIC never appears in any fixture in this file; without
    // this, a typo in the constant or a dropped match arm in `validate`
    // would go undetected.
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

    // Spec: migration "happens once, silently, and is logged." Silent means
    // no UI prompt -- it does not mean no log line. Confirm the *successful*
    // migration path writes a log line a maintainer could later grep for,
    // not just the already-covered rewrite-failure path above.
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

    // A file that is already v2 must not log a migration line -- there was
    // no migration to report.
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

    // I1 part 2 / C1: `write_atomic` is the one function both the v1->v2
    // migration rewrite *and* `commands::save_config` (the settings
    // window's Save button, now that C1 is fixed) rely on for this
    // guarantee -- there is no separate write path to test on the
    // `save_config` side, since it calls this exact function. A failure
    // must never leave the destination truncated or partially written.
    // Force the rename step specifically to fail (destination read-only)
    // and confirm the pre-existing destination content is completely
    // untouched.
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

    // C1 extension: `load_or_create`'s first-run write goes through
    // `write_atomic` now too, rather than a separate `fs::write` call
    // nothing here previously exercised at all. Confirm the ordinary,
    // successful path still ends up with a valid, fully-written
    // config.json and no leftover `.tmp` file.
    #[test]
    fn load_or_create_writes_a_valid_default_config_on_first_run() {
        let dir = std::env::temp_dir().join(format!(
            "cai-test-first-run-{}-{}",
            std::process::id(),
            "a"
        ));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("config.json"); // parent doesn't exist yet either

        let cfg = load_or_create(&path).expect("first run should succeed");

        assert_eq!(cfg.commands.len(), 1);
        assert!(path.exists(), "a default config.json should now exist on disk");
        assert!(!dir.join("config.json.tmp").exists(), "no leftover temp file");
        let raw = fs::read_to_string(&path).unwrap();
        parse(&raw).expect("the written file must itself be valid, parseable config");

        let _ = fs::remove_dir_all(&dir);
    }

    // C2: an unreadable/unparseable config file must be preserved, not
    // lost, once the caller (lib.rs's `setup`) decides the load failed.
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

    // C2: a second unreadable file (a later launch hitting the same
    // failure again, or the user re-breaking the file after fixing it once)
    // must not clobber the first preserved copy -- every corrupt file the
    // user ever produces should stay recoverable.
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

        // A fresh config.json reappears at the original path -- either a
        // fresh default write, or (as here) the user hand-editing something
        // back into place -- and breaks again.
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

}
