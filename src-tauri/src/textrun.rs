use std::time::Instant;

use serde::Serialize;
use serde_json::{json, Value};
use thiserror::Error;

use crate::backend::{self, BackendError};
use crate::config::{BackendConfig, Config, Formatting, Vocabulary};
use crate::cycle::{self, Job, Transformed};
use crate::osops::Clip;
use crate::plan::BackendBuilder;

#[derive(Debug, Clone)]
pub struct Plan {
    pub command_id: Option<String>,
    pub command_name: Option<String>,
    pub backend: BackendConfig,
    pub system_prompt: String,
    pub formatting: Formatting,
    pub vocabulary: Vocabulary,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResolveError {
    #[error("there is no command called \"{0}\"")]
    UnknownCommand(String),
    #[error("more than one command is called \"{0}\"; use its id instead")]
    AmbiguousCommand(String),
    #[error("there is no backend called \"{0}\"")]
    UnknownBackend(String),
    #[error("more than one backend is called \"{0}\"; use its id instead")]
    AmbiguousBackend(String),
    #[error("name a command, or give a prompt to run without one")]
    NothingToRun,
    #[error("several backends are configured; choose one for a run without a command")]
    BackendNeeded,
    #[error("no backends are configured")]
    NoBackends,
    #[error("the prompt is empty")]
    EmptyPrompt,
}

impl ResolveError {
    pub fn code(&self) -> &'static str {
        match self {
            ResolveError::UnknownCommand(_) => "unknown-command",
            ResolveError::AmbiguousCommand(_) => "ambiguous-command",
            ResolveError::UnknownBackend(_) => "unknown-backend",
            ResolveError::AmbiguousBackend(_) => "ambiguous-backend",
            ResolveError::NothingToRun => "nothing-to-run",
            ResolveError::BackendNeeded => "backend-needed",
            ResolveError::NoBackends => "no-backends",
            ResolveError::EmptyPrompt => "empty-prompt",
        }
    }
}

#[derive(Debug, Error, Clone)]
pub enum RunError {
    #[error("{0}")]
    Backend(BackendError),
    #[error("the backend answered without the expected <output> block")]
    NoSentinel,
    #[error("the backend returned an empty result")]
    EmptyResult,
    #[error("the run did not finish ({0})")]
    Other(&'static str),
}

impl RunError {
    pub fn code(&self) -> &'static str {
        match self {
            RunError::Backend(BackendError::Timeout) => "backend-timeout",
            RunError::Backend(BackendError::Misconfigured(_)) => "backend-misconfigured",
            RunError::Backend(_) => "backend-error",
            RunError::NoSentinel => "no-sentinel",
            RunError::EmptyResult => "empty-result",
            RunError::Other(reason) => reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextResult {
    pub text: Option<String>,
    pub html: Option<String>,
    pub backend_ms: u64,
}

fn find_one<'a, T>(
    items: &'a [T],
    wanted: &str,
    id: impl Fn(&T) -> &str,
    name: impl Fn(&T) -> &str,
) -> Result<Option<&'a T>, ()> {
    if let Some(hit) = items.iter().find(|item| id(item) == wanted) {
        return Ok(Some(hit));
    }
    let needle = wanted.trim().to_lowercase();
    let mut hits = items.iter().filter(|item| name(item).trim().to_lowercase() == needle);
    match (hits.next(), hits.next()) {
        (Some(hit), None) => Ok(Some(hit)),
        (None, _) => Ok(None),
        (Some(_), Some(_)) => Err(()),
    }
}

fn find_backend<'a>(config: &'a Config, wanted: &str) -> Result<&'a BackendConfig, ResolveError> {
    match find_one(&config.backends, wanted, |b| &b.id, |b| &b.name) {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(ResolveError::UnknownBackend(wanted.to_string())),
        Err(()) => Err(ResolveError::AmbiguousBackend(wanted.to_string())),
    }
}

pub fn resolve(
    config: &Config,
    command: Option<&str>,
    backend: Option<&str>,
    prompt: Option<&str>,
) -> Result<Plan, ResolveError> {
    if prompt.is_some_and(|p| p.trim().is_empty()) {
        return Err(ResolveError::EmptyPrompt);
    }
    let backend_override = backend.map(|b| find_backend(config, b)).transpose()?;

    match command {
        Some(wanted) => {
            let found = match find_one(&config.commands, wanted, |c| &c.id, |c| &c.name) {
                Ok(Some(c)) => c,
                Ok(None) => return Err(ResolveError::UnknownCommand(wanted.to_string())),
                Err(()) => return Err(ResolveError::AmbiguousCommand(wanted.to_string())),
            };
            let backend = match backend_override {
                Some(b) => b,
                None => config
                    .backend(&found.backend_id)
                    .ok_or_else(|| ResolveError::UnknownBackend(found.backend_id.clone()))?,
            };
            Ok(Plan {
                command_id: Some(found.id.clone()),
                command_name: Some(found.name.clone()),
                backend: backend.clone(),
                system_prompt: prompt.unwrap_or(&found.system_prompt).to_string(),
                formatting: found.formatting,
                vocabulary: config.vocabulary.clone(),
            })
        }
        None => {
            let Some(prompt) = prompt else {
                return Err(ResolveError::NothingToRun);
            };
            let backend = match backend_override {
                Some(b) => b,
                None => match config.backends.as_slice() {
                    [] => return Err(ResolveError::NoBackends),
                    [only] => only,
                    _ => return Err(ResolveError::BackendNeeded),
                },
            };
            Ok(Plan {
                command_id: None,
                command_name: None,
                backend: backend.clone(),
                system_prompt: prompt.to_string(),
                formatting: Formatting::Auto,
                vocabulary: config.vocabulary.clone(),
            })
        }
    }
}

pub fn run_plan_with(plan: &Plan, text: &str, builder: BackendBuilder) -> Result<TextResult, RunError> {
    let backend = builder(&plan.backend).map_err(RunError::Backend)?;
    let job = Job {
        backend: &backend,
        system_prompt: &plan.system_prompt,
        prompt_addition: None,
        formatting: plan.formatting,
        vocabulary: &plan.vocabulary,
        rule_name: None,
    };
    let started = Instant::now();
    let clock = || started.elapsed().as_millis() as u64;
    let input = Clip { text: text.to_string(), html: None };
    let transform = cycle::transform(&job, &input, &clock);
    let backend_ms = transform.backend_ms;
    match transform.result {
        Ok(Transformed::Result(clip)) => {
            Ok(TextResult { text: Some(clip.text), html: clip.html, backend_ms })
        }
        Ok(Transformed::Sent) => Ok(TextResult { text: None, html: None, backend_ms }),
        Err(failure) => Err(match (failure.backend_error, failure.reason) {
            (Some(e), _) => RunError::Backend(e),
            (None, "no-sentinel") => RunError::NoSentinel,
            (None, "empty-result") => RunError::EmptyResult,
            (None, other) => RunError::Other(other),
        }),
    }
}

pub fn run_plan(plan: &Plan, text: &str) -> Result<TextResult, RunError> {
    run_plan_with(plan, text, &backend::build)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicCommand<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub hotkey: &'a str,
    pub backend_id: &'a str,
    pub formatting: Formatting,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicBackend<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub kind: &'a str,
    pub model: Option<&'a str>,
}

pub fn public_commands(config: &Config) -> Vec<PublicCommand<'_>> {
    config
        .commands
        .iter()
        .map(|c| PublicCommand {
            id: &c.id,
            name: &c.name,
            hotkey: &c.hotkey,
            backend_id: &c.backend_id,
            formatting: c.formatting,
        })
        .collect()
}

pub fn public_backends(config: &Config) -> Vec<PublicBackend<'_>> {
    config
        .backends
        .iter()
        .map(|b| PublicBackend {
            id: &b.id,
            name: &b.name,
            kind: &b.kind,
            model: b.model.as_deref().filter(|m| !m.trim().is_empty()),
        })
        .collect()
}

pub fn listing_json(config: &Config) -> Value {
    json!({
        "commands": public_commands(config),
        "backends": public_backends(config),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::backend::Backend;
    use crate::config::{self, CommandConfig, Effort, KIND_HTTP, KIND_OPENAI};

    fn backend_cfg(id: &str, name: &str) -> BackendConfig {
        BackendConfig {
            id: id.into(),
            name: name.into(),
            kind: KIND_OPENAI.into(),
            base_url: Some("http://127.0.0.1:9/v1".into()),
            model: Some("m".into()),
            api_key: Some("sk-secret".into()),
            timeout_ms: 120_000,
            effort: Effort::Low,
            expects_response: true,
        }
    }

    fn command_cfg(id: &str, name: &str, backend: &str) -> CommandConfig {
        CommandConfig {
            id: id.into(),
            name: name.into(),
            backend_id: backend.into(),
            hotkey: format!("Ctrl+Alt+{}", id.len()),
            system_prompt: format!("prompt of {id}"),
            formatting: Formatting::Auto,
            app_rules: Vec::new(),
        }
    }

    fn sample() -> Config {
        Config {
            version: config::CURRENT_VERSION,
            backends: vec![backend_cfg("local", "Local Ollama"), backend_cfg("cloud", "Cloud")],
            commands: vec![
                command_cfg("fix", "Fix grammar", "local"),
                command_cfg("shorten", "Shorten", "cloud"),
            ],
            settings: config::Settings::default(),
            vocabulary: Default::default(),
        }
    }

    #[test]
    fn resolves_a_command_by_id_or_by_name_ignoring_case() {
        let cfg = sample();
        for wanted in ["fix", "Fix grammar", "  fix GRAMMAR "] {
            let plan = resolve(&cfg, Some(wanted), None, None).unwrap();
            assert_eq!(plan.command_id.as_deref(), Some("fix"), "{wanted}");
            assert_eq!(plan.backend.id, "local");
            assert_eq!(plan.system_prompt, "prompt of fix");
        }
    }

    #[test]
    fn an_id_match_wins_over_a_name_match() {
        let mut cfg = sample();
        cfg.commands[1].name = "fix".into();
        let plan = resolve(&cfg, Some("fix"), None, None).unwrap();
        assert_eq!(plan.command_id.as_deref(), Some("fix"));
    }

    #[test]
    fn two_commands_with_the_same_name_are_ambiguous() {
        let mut cfg = sample();
        cfg.commands[1].name = "Fix Grammar".into();
        assert_eq!(
            resolve(&cfg, Some("fix grammar"), None, None).unwrap_err(),
            ResolveError::AmbiguousCommand("fix grammar".into())
        );
    }

    #[test]
    fn backend_and_prompt_overrides_apply_to_a_command() {
        let plan = resolve(&sample(), Some("fix"), Some("cloud"), Some("Translate.")).unwrap();
        assert_eq!(plan.backend.id, "cloud");
        assert_eq!(plan.system_prompt, "Translate.");
        assert_eq!(plan.command_id.as_deref(), Some("fix"));
    }

    #[test]
    fn unknown_names_are_reported_with_the_name_given() {
        let cfg = sample();
        assert_eq!(
            resolve(&cfg, Some("nope"), None, None).unwrap_err(),
            ResolveError::UnknownCommand("nope".into())
        );
        assert_eq!(
            resolve(&cfg, Some("fix"), Some("nope"), None).unwrap_err(),
            ResolveError::UnknownBackend("nope".into())
        );
    }

    #[test]
    fn a_prompt_without_a_command_needs_a_backend_when_there_are_several() {
        let cfg = sample();
        assert_eq!(resolve(&cfg, None, None, None).unwrap_err(), ResolveError::NothingToRun);
        assert_eq!(resolve(&cfg, None, None, Some("Do it.")).unwrap_err(), ResolveError::BackendNeeded);
        let plan = resolve(&cfg, None, Some("Cloud"), Some("Do it.")).unwrap();
        assert_eq!(plan.backend.id, "cloud");
        assert_eq!(plan.command_id, None);
        assert_eq!(plan.formatting, Formatting::Auto);

        let mut single = sample();
        single.backends.truncate(1);
        single.commands.truncate(1);
        assert_eq!(resolve(&single, None, None, Some("Do it.")).unwrap().backend.id, "local");

        let mut empty = sample();
        empty.backends.clear();
        empty.commands.clear();
        assert_eq!(resolve(&empty, None, None, Some("Do it.")).unwrap_err(), ResolveError::NoBackends);
    }

    #[test]
    fn a_blank_prompt_is_refused() {
        assert_eq!(resolve(&sample(), Some("fix"), None, Some("  ")).unwrap_err(), ResolveError::EmptyPrompt);
    }

    struct Recording {
        reply: Result<String, BackendError>,
        sentinel: bool,
        pastes: bool,
        seen: Mutex<Option<(String, String)>>,
    }

    impl Recording {
        fn new(reply: &str) -> Arc<Self> {
            Arc::new(Self { reply: Ok(reply.into()), sentinel: true, pastes: true, seen: Mutex::new(None) })
        }
        fn raw(reply: Result<String, BackendError>, sentinel: bool, pastes: bool) -> Arc<Self> {
            Arc::new(Self { reply, sentinel, pastes, seen: Mutex::new(None) })
        }
        fn seen(&self) -> (String, String) {
            self.seen.lock().unwrap().clone().expect("the backend was called")
        }
    }

    impl Backend for Recording {
        fn name(&self) -> &str {
            "rec"
        }
        fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError> {
            *self.seen.lock().unwrap() = Some((system_prompt.into(), text.into()));
            self.reply.clone()
        }
        fn uses_sentinel(&self) -> bool {
            self.sentinel
        }
        fn pastes_result(&self) -> bool {
            self.pastes
        }
    }

    fn run_with(backend: &Arc<Recording>, plan: &Plan, text: &str) -> Result<TextResult, RunError> {
        let shared: Arc<dyn Backend> = backend.clone();
        run_plan_with(plan, text, &move |_| Ok(shared.clone()))
    }

    fn plan_for(cfg: &Config, command: &str) -> Plan {
        resolve(cfg, Some(command), None, None).unwrap()
    }

    #[test]
    fn runs_through_the_shared_transform_like_a_hotkey_run_without_an_app() {
        let backend = Recording::new("thinking...<output>Fixed text.</output>");
        let cfg = sample();
        let plan = plan_for(&cfg, "fix");
        let result = run_with(&backend, &plan, "fixd txt").unwrap();
        assert_eq!(result.text.as_deref(), Some("Fixed text."));
        assert_eq!(result.html, None);

        let shared: Arc<dyn Backend> = backend.clone();
        let job = Job {
            backend: &shared,
            system_prompt: "prompt of fix",
            prompt_addition: None,
            formatting: Formatting::Auto,
            vocabulary: &cfg.vocabulary,
            rule_name: None,
        };
        let (prompt, text) = backend.seen();
        assert_eq!(prompt, cycle::assemble_system_prompt(&job, None));
        assert_eq!(text, "fixd txt");
    }

    #[test]
    fn vocabulary_applies_exactly_as_in_a_hotkey_run() {
        let backend = Recording::new("<output>ok</output>");
        let mut cfg = sample();
        cfg.vocabulary = Vocabulary {
            keep: vec!["Tauri".into()],
            replacements: vec![crate::config::Replacement { from: "teh".into(), to: "the".into() }],
        };
        let plan = plan_for(&cfg, "fix");
        run_with(&backend, &plan, "teh app uses Tauri").unwrap();
        let (prompt, text) = backend.seen();
        assert_eq!(text, "the app uses Tauri");
        assert!(prompt.contains("Tauri"), "{prompt}");
        assert!(prompt.contains(&crate::vocabulary::keep_instruction(&cfg.vocabulary).unwrap()));

        let one_off = resolve(&cfg, None, Some("local"), Some("Shout.")).unwrap();
        run_with(&backend, &one_off, "teh end").unwrap();
        assert_eq!(backend.seen().1, "the end", "a one-off prompt gets the vocabulary too");
    }

    #[test]
    fn rich_formatting_asks_for_html_and_returns_both_forms() {
        let backend = Recording::new("<output><p>Hello <b>world</b></p></output>");
        let mut cfg = sample();
        cfg.commands[0].formatting = Formatting::Rich;
        let result = run_with(&backend, &plan_for(&cfg, "fix"), "hello world").unwrap();
        assert!(backend.seen().0.contains(crate::richtext::RICH_INSTRUCTION));
        assert_eq!(result.text.as_deref(), Some("Hello world"));
        assert!(result.html.unwrap().contains("<b>world</b>"));
    }

    #[test]
    fn a_missing_sentinel_or_empty_answer_is_an_error_not_a_result() {
        let cfg = sample();
        let plan = plan_for(&cfg, "fix");
        let backend = Recording::new("no tags here");
        assert!(matches!(run_with(&backend, &plan, "x"), Err(RunError::NoSentinel)));
        let raw = Recording::raw(Ok("   ".into()), false, true);
        assert!(matches!(run_with(&raw, &plan, "x"), Err(RunError::EmptyResult)));
    }

    #[test]
    fn a_send_only_backend_returns_no_text() {
        let backend = Recording::raw(Ok("ignored".into()), false, false);
        let result = run_with(&backend, &plan_for(&sample(), "fix"), "x").unwrap();
        assert_eq!(result.text, None);
    }

    #[test]
    fn backend_errors_keep_their_message_and_get_a_stable_code() {
        let backend = Recording::raw(Err(BackendError::Timeout), true, true);
        let err = run_with(&backend, &plan_for(&sample(), "fix"), "x").unwrap_err();
        assert_eq!(err.code(), "backend-timeout");
        assert_eq!(err.to_string(), "backend timed out");
    }

    #[test]
    fn a_backend_that_cannot_be_built_is_a_backend_error() {
        let plan = plan_for(&sample(), "fix");
        let err = run_plan_with(&plan, "x", &|_| Err(BackendError::Misconfigured("no key".into()))).unwrap_err();
        assert_eq!(err.code(), "backend-misconfigured");
    }

    #[test]
    fn the_public_listing_never_contains_keys_or_urls() {
        let mut cfg = sample();
        cfg.backends[1].kind = KIND_HTTP.into();
        let listing = listing_json(&cfg).to_string();
        assert!(!listing.contains("sk-secret"), "{listing}");
        assert!(!listing.contains("127.0.0.1"), "{listing}");
        assert!(listing.contains("\"backendId\":\"local\""));
        assert!(listing.contains("\"kind\":\"http\""));
    }
}
