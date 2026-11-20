use std::sync::Arc;
use std::time::Instant;

use serde::Serialize;
use tauri::State;

use crate::backend::{self, Backend, BackendError};
use crate::config::{CommandConfig, Config, Vocabulary};
use crate::cycle::{self, Failure, Transformed};
use crate::osops::Clip;
use crate::{apps, plan, sentinel, AppState};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TryResult {
    pub text: String,
    pub html: Option<String>,
    pub backend_ms: u64,
    pub backend_name: String,
    pub rule: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorError {
    pub reason: String,
    pub detail: Option<String>,
}

impl EditorError {
    fn new(reason: &str, detail: Option<String>) -> Self {
        Self { reason: reason.to_string(), detail }
    }
}

fn backend_error(e: &BackendError) -> EditorError {
    match e {
        BackendError::Timeout => EditorError::new("timeout", None),
        BackendError::Transport(d) => EditorError::new("unreachable", Some(d.clone())),
        BackendError::Misconfigured(d) => EditorError::new("misconfigured", Some(d.clone())),
        BackendError::BadResponse(d) => EditorError::new("bad-response", Some(d.clone())),
    }
}

fn failure(f: Failure) -> EditorError {
    match &f.backend_error {
        Some(e) => backend_error(e),
        None => EditorError::new(f.reason, f.detail),
    }
}

fn current_config(state: &AppState) -> Config {
    crate::recover_lock(state.config.lock(), "config", Some(&state.config_dir)).clone()
}

pub fn try_with(
    command: &CommandConfig,
    config: &Config,
    vocabulary: Arc<Vocabulary>,
    text: &str,
    app: Option<&str>,
    builder: plan::BackendBuilder,
) -> Result<TryResult, EditorError> {
    if text.trim().is_empty() {
        return Err(EditorError::new("empty-input", None));
    }
    let (plan, warnings) = plan::build(command, config, vocabulary, builder)
        .map_err(|e| EditorError::new("unknown-backend", Some(e)))?;
    let job = plan.job_for(app);
    if !job.backend.pastes_result() {
        return Err(EditorError::new("nothing-to-preview", None));
    }
    let started = Instant::now();
    let clock = || started.elapsed().as_millis() as u64;
    let input = Clip { text: text.to_string(), html: None };
    let transform = cycle::transform(&job, &input, &clock);
    match transform.result {
        Ok(Transformed::Result(clip)) => Ok(TryResult {
            text: clip.text,
            html: clip.html,
            backend_ms: transform.backend_ms,
            backend_name: job.backend.name().to_string(),
            rule: job.rule_name.map(str::to_string),
            warnings,
        }),
        Ok(Transformed::Sent) => Err(EditorError::new("nothing-to-preview", None)),
        Err(f) => Err(failure(f)),
    }
}

#[tauri::command]
pub async fn try_command(
    state: State<'_, AppState>,
    command: CommandConfig,
    text: String,
    app: Option<String>,
) -> Result<TryResult, EditorError> {
    let config = current_config(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let vocabulary = Arc::new(config.vocabulary.clone());
        try_with(&command, &config, vocabulary, &text, app.as_deref(), &backend::build)
    })
    .await
    .map_err(|e| EditorError::new("internal", Some(e.to_string())))?
}

pub const WRITER_PROMPT: &str = "You write the prompt for one command in a text tool. The user \
selects text in any app and presses the command's hotkey. Your prompt is then sent to a model \
as its instructions, with the selected text as the message, and the model's answer replaces the \
selection.

Write the prompt that does what the user describes. Match the style of these examples:
Fix grammar and spelling. Keep the meaning, the tone and the formatting.
Cut this to about half the length. Keep every fact and every name.
Translate to German. Use du unless the text is clearly formal.

Rules:
- Speak to the model in the imperative and talk about the text itself.
- Use one to three short sentences. Add a sentence only for something the user asked for, such \
as a tone, a length, an audience or what must stay unchanged.
- Plain sentences only: no headings, no lists, no quotes around the prompt, no role play such as \
\"You are an expert\", no greetings and no placeholders.
- Never ask for the answer to contain only the result, never forbid explanations or preambles, \
never mention tags, HTML, Markdown or keeping the formatting, and never mention hotkeys, the \
clipboard or the app. The tool adds all of that by itself.
- Write the prompt in the language the user wrote the description in.
- If the description is vague, write the most sensible prompt for it instead of asking a question.
- If there is a current prompt, revise it to match the description and keep what still fits.";

pub fn writer_input(description: &str, current_prompt: &str) -> String {
    let description = description.trim();
    let current = current_prompt.trim();
    if current.is_empty() {
        format!("What the command should do:\n{description}")
    } else {
        format!("Current prompt:\n{current}\n\nWhat the command should do:\n{description}")
    }
}

const QUOTE_PAIRS: [(char, char); 4] =
    [('"', '"'), ('\u{201c}', '\u{201d}'), ('\u{201e}', '\u{201c}'), ('\u{ab}', '\u{bb}')];

pub fn clean_written_prompt(raw: &str) -> String {
    let trimmed = raw.trim();
    let unquoted = QUOTE_PAIRS
        .iter()
        .find_map(|(open, close)| trimmed.strip_prefix(*open)?.strip_suffix(*close))
        .unwrap_or(trimmed);
    unquoted.trim().to_string()
}

pub fn write_with(
    backend: &dyn Backend,
    description: &str,
    current_prompt: &str,
) -> Result<String, EditorError> {
    if description.trim().is_empty() {
        return Err(EditorError::new("empty-input", None));
    }
    if !backend.pastes_result() || !backend.uses_sentinel() {
        return Err(EditorError::new("not-an-ai-backend", None));
    }
    let system = sentinel::wrap_system_prompt(WRITER_PROMPT);
    let raw = backend
        .transform(&system, &writer_input(description, current_prompt))
        .map_err(|e| backend_error(&e))?;
    let extracted =
        sentinel::extract(&raw).ok_or_else(|| EditorError::new("no-sentinel", None))?;
    let prompt = clean_written_prompt(&extracted);
    if prompt.is_empty() {
        return Err(EditorError::new("empty-result", None));
    }
    Ok(prompt)
}

#[tauri::command]
pub async fn write_prompt(
    state: State<'_, AppState>,
    backend_id: String,
    description: String,
    current_prompt: String,
) -> Result<String, EditorError> {
    let config = current_config(&state);
    let backend_cfg = config
        .backend(&backend_id)
        .cloned()
        .ok_or_else(|| EditorError::new("unknown-backend", None))?;
    tauri::async_runtime::spawn_blocking(move || {
        let backend = backend::build(&backend_cfg).map_err(|e| backend_error(&e))?;
        write_with(backend.as_ref(), &description, &current_prompt)
    })
    .await
    .map_err(|e| EditorError::new("internal", Some(e.to_string())))?
}

#[tauri::command]
pub async fn list_running_apps() -> Result<Vec<apps::RunningApp>, String> {
    tauri::async_runtime::spawn_blocking(apps::running_apps)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::config::{
        default_config, AppRule, BackendConfig, Formatting, Replacement, KIND_HTTP,
    };

    struct Recording {
        name: String,
        answer: Result<String, BackendError>,
        seen: Mutex<Vec<(String, String)>>,
        sentinel: bool,
        pastes: bool,
    }

    impl Recording {
        fn answering(answer: &str) -> Self {
            Self {
                name: "Recorder".into(),
                answer: Ok(answer.into()),
                seen: Mutex::new(Vec::new()),
                sentinel: true,
                pastes: true,
            }
        }
    }

    impl Backend for Recording {
        fn name(&self) -> &str {
            &self.name
        }
        fn transform(&self, system: &str, text: &str) -> Result<String, BackendError> {
            self.seen.lock().unwrap().push((system.to_string(), text.to_string()));
            self.answer.clone()
        }
        fn uses_sentinel(&self) -> bool {
            self.sentinel
        }
        fn pastes_result(&self) -> bool {
            self.pastes
        }
        fn supports_rich_text(&self) -> bool {
            self.sentinel
        }
    }

    fn shared(backend: &Arc<Recording>) -> impl Fn(&BackendConfig) -> Result<Arc<dyn Backend>, BackendError> {
        let backend = backend.clone();
        move |_| Ok(backend.clone() as Arc<dyn Backend>)
    }

    fn vocab(keep: &[&str], reps: &[(&str, &str)]) -> Arc<Vocabulary> {
        Arc::new(Vocabulary {
            keep: keep.iter().map(|s| s.to_string()).collect(),
            replacements: reps
                .iter()
                .map(|(f, t)| Replacement { from: f.to_string(), to: t.to_string() })
                .collect(),
        })
    }

    #[test]
    fn try_sends_exactly_what_a_hotkey_run_would_send() {
        let config = default_config();
        let mut command = config.commands[0].clone();
        command.system_prompt = "Unsaved prompt.".into();
        let backend = Arc::new(Recording::answering("<output>the cat</output>"));
        let vocabulary = vocab(&["Tauri"], &[("teh", "the")]);

        let result = try_with(&command, &config, vocabulary.clone(), "teh cat", None, &shared(&backend))
            .expect("try should succeed");
        assert_eq!(result.text, "the cat");
        assert_eq!(result.html, None);
        assert_eq!(result.backend_name, "Recorder");

        let plan = plan::build(&command, &config, vocabulary, &shared(&backend)).unwrap().0;
        let job = plan.job_for(None);
        let expected_prompt = cycle::assemble_system_prompt(&job, None);
        let seen = backend.seen.lock().unwrap().clone();
        assert_eq!(seen, vec![(expected_prompt.clone(), "the cat".to_string())]);
        assert!(expected_prompt.starts_with("Unsaved prompt.\n\nWrite these words"));
        assert!(expected_prompt.contains("- Tauri"));
        assert!(expected_prompt.ends_with(sentinel::SENTINEL_INSTRUCTION));
    }

    #[test]
    fn try_with_always_formatted_returns_html() {
        let config = default_config();
        let mut command = config.commands[0].clone();
        command.formatting = Formatting::Rich;
        let backend = Arc::new(Recording::answering("<output><ul><li>One</li></ul></output>"));
        let result =
            try_with(&command, &config, vocab(&[], &[]), "one", None, &shared(&backend)).unwrap();
        assert_eq!(result.html.as_deref(), Some("<ul><li>One</li></ul>"));
        assert_eq!(result.text, "- One");
        let (system, _) = backend.seen.lock().unwrap()[0].clone();
        assert!(system.contains(crate::richtext::RICH_INSTRUCTION));
    }

    #[test]
    fn try_applies_the_rule_for_the_chosen_app() {
        let mut config = default_config();
        config.commands[0].app_rules = vec![AppRule {
            exe: "OUTLOOK.EXE".into(),
            name: "Outlook".into(),
            prompt_addition: "Keep it formal.".into(),
            backend_id: None,
            formatting: None,
        }];
        let command = config.commands[0].clone();
        let backend = Arc::new(Recording::answering("<output>Dear Sir</output>"));
        let result = try_with(
            &command,
            &config,
            vocab(&[], &[]),
            "hi",
            Some("outlook.exe"),
            &shared(&backend),
        )
        .unwrap();
        assert_eq!(result.rule.as_deref(), Some("Outlook"));
        let (system, _) = backend.seen.lock().unwrap()[0].clone();
        assert!(system.starts_with("Fix grammar and spelling.\n\nKeep it formal.\n\n"), "{system}");
    }

    #[test]
    fn try_refuses_a_send_only_backend_without_sending_anything() {
        let mut config = default_config();
        config.backends[0].kind = KIND_HTTP.into();
        let command = config.commands[0].clone();
        let backend = Arc::new(Recording { pastes: false, sentinel: false, ..Recording::answering("") });
        let err =
            try_with(&command, &config, vocab(&[], &[]), "x", None, &shared(&backend)).unwrap_err();
        assert_eq!(err.reason, "nothing-to-preview");
        assert!(backend.seen.lock().unwrap().is_empty());
    }

    #[test]
    fn try_shows_the_raw_response_of_an_http_backend() {
        let config = default_config();
        let command = config.commands[0].clone();
        let backend =
            Arc::new(Recording { sentinel: false, ..Recording::answering("  raw body\n") });
        let result =
            try_with(&command, &config, vocab(&["Kept"], &[]), "x", None, &shared(&backend)).unwrap();
        assert_eq!(result.text, "raw body");
    }

    #[test]
    fn try_turns_backend_failures_into_plain_reasons() {
        let config = default_config();
        let command = config.commands[0].clone();
        let cases = [
            (Err(BackendError::Timeout), "timeout"),
            (Err(BackendError::Transport("refused".into())), "unreachable"),
            (Err(BackendError::Misconfigured("apiKey is required".into())), "misconfigured"),
            (Err(BackendError::BadResponse("HTTP 401".into())), "bad-response"),
            (Ok("no tags here".to_string()), "no-sentinel"),
        ];
        for (answer, reason) in cases {
            let backend = Arc::new(Recording { answer, ..Recording::answering("") });
            let err = try_with(&command, &config, vocab(&[], &[]), "x", None, &shared(&backend))
                .unwrap_err();
            assert_eq!(err.reason, reason);
        }
    }

    #[test]
    fn try_with_blank_input_never_calls_the_backend() {
        let config = default_config();
        let backend = Arc::new(Recording::answering("<output>x</output>"));
        let err = try_with(&config.commands[0], &config, vocab(&[], &[]), "  \n", None, &shared(&backend))
            .unwrap_err();
        assert_eq!(err.reason, "empty-input");
        assert!(backend.seen.lock().unwrap().is_empty());
    }

    #[test]
    fn try_with_an_unknown_backend_fails_cleanly() {
        let config = default_config();
        let mut command = config.commands[0].clone();
        command.backend_id = "ghost".into();
        let backend = Arc::new(Recording::answering("<output>x</output>"));
        let err =
            try_with(&command, &config, vocab(&[], &[]), "x", None, &shared(&backend)).unwrap_err();
        assert_eq!(err.reason, "unknown-backend");
    }

    #[test]
    fn writing_a_prompt_uses_the_meta_prompt_and_returns_the_clean_prompt() {
        let backend = Recording::answering("Sure!\n<output>\"Make it friendlier. Keep it short.\"</output>");
        let prompt = write_with(&backend, "friendlier emails, short", "").unwrap();
        assert_eq!(prompt, "Make it friendlier. Keep it short.");
        let (system, text) = backend.seen.lock().unwrap()[0].clone();
        assert!(system.starts_with(WRITER_PROMPT));
        assert!(system.ends_with(sentinel::SENTINEL_INSTRUCTION));
        assert_eq!(text, "What the command should do:\nfriendlier emails, short");
    }

    #[test]
    fn surrounding_quotes_of_any_common_kind_are_removed() {
        assert_eq!(clean_written_prompt(" \u{201c}Be brief.\u{201d} "), "Be brief.");
        assert_eq!(clean_written_prompt("\u{201e}Sei kurz.\u{201c}"), "Sei kurz.");
        assert_eq!(clean_written_prompt("\u{ab} Sois bref. \u{bb}"), "Sois bref.");
        assert_eq!(clean_written_prompt("Say \"hi\" nicely."), "Say \"hi\" nicely.");
    }

    #[test]
    fn try_reports_a_rule_it_had_to_drop() {
        let mut config = default_config();
        config.commands[0].app_rules = vec![AppRule {
            exe: "OUTLOOK.EXE".into(),
            name: "Outlook".into(),
            prompt_addition: String::new(),
            backend_id: Some("ghost".into()),
            formatting: None,
        }];
        let backend = Arc::new(Recording::answering("<output>x</output>"));
        let command = config.commands[0].clone();
        let result =
            try_with(&command, &config, vocab(&[], &[]), "x", Some("outlook.exe"), &shared(&backend))
                .unwrap();
        assert_eq!(result.rule, None);
        assert_eq!(result.warnings, vec!["rule for Outlook: unknown backend".to_string()]);
    }

    #[test]
    fn writing_a_prompt_passes_the_current_prompt_for_revision() {
        assert_eq!(
            writer_input(" shorter ", " Fix grammar. "),
            "Current prompt:\nFix grammar.\n\nWhat the command should do:\nshorter"
        );
    }

    #[test]
    fn writing_a_prompt_refuses_blank_descriptions_and_non_ai_backends() {
        let backend = Recording::answering("<output>x</output>");
        assert_eq!(write_with(&backend, " ", "").unwrap_err().reason, "empty-input");
        let http = Recording { sentinel: false, ..Recording::answering("x") };
        assert_eq!(write_with(&http, "x", "").unwrap_err().reason, "not-an-ai-backend");
        assert!(http.seen.lock().unwrap().is_empty());
    }

    #[test]
    fn the_writer_prompt_never_asks_for_what_the_app_already_adds() {
        let lower = WRITER_PROMPT.to_lowercase();
        assert!(!lower.contains("<output>"));
        assert!(!WRITER_PROMPT.contains('\u{2014}') && !WRITER_PROMPT.contains('\u{2013}'));
    }
}
