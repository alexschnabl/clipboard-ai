use std::sync::Arc;

use crate::backend::{Backend, BackendError};
use crate::config::{normalize_exe, BackendConfig, CommandConfig, Config, Formatting, Vocabulary};
use crate::cycle::Job;

pub struct RulePlan {
    pub exe: String,
    pub name: String,
    pub prompt_addition: String,
    pub backend: Option<Arc<dyn Backend>>,
    pub formatting: Option<Formatting>,
}

pub struct CommandPlan {
    pub system_prompt: String,
    pub formatting: Formatting,
    pub backend: Arc<dyn Backend>,
    pub rules: Vec<RulePlan>,
    pub vocabulary: Arc<Vocabulary>,
}

pub type BackendBuilder<'a> = &'a dyn Fn(&BackendConfig) -> Result<Arc<dyn Backend>, BackendError>;

pub fn build(
    command: &CommandConfig,
    config: &Config,
    vocabulary: Arc<Vocabulary>,
    builder: BackendBuilder,
) -> Result<(CommandPlan, Vec<String>), String> {
    let backend_cfg = config
        .backend(&command.backend_id)
        .ok_or_else(|| "unknown backend".to_string())?;
    let backend = builder(backend_cfg).map_err(|e| e.to_string())?;

    let mut problems = Vec::new();
    let mut rules = Vec::new();
    for rule in &command.app_rules {
        let exe = normalize_exe(&rule.exe);
        if exe.is_empty() {
            continue;
        }
        let label = if rule.name.trim().is_empty() { rule.exe.trim() } else { rule.name.trim() };
        let rule_backend = match &rule.backend_id {
            None => None,
            Some(id) => match config.backend(id).map(builder) {
                Some(Ok(b)) => Some(b),
                Some(Err(e)) => {
                    problems.push(format!("rule for {label}: {e}"));
                    continue;
                }
                None => {
                    problems.push(format!("rule for {label}: unknown backend"));
                    continue;
                }
            },
        };
        rules.push(RulePlan {
            exe,
            name: label.to_string(),
            prompt_addition: rule.prompt_addition.trim().to_string(),
            backend: rule_backend,
            formatting: rule.formatting,
        });
    }

    Ok((
        CommandPlan {
            system_prompt: command.system_prompt.clone(),
            formatting: command.formatting,
            backend,
            rules,
            vocabulary,
        },
        problems,
    ))
}

impl CommandPlan {
    pub fn rule_for(&self, exe: Option<&str>) -> Option<&RulePlan> {
        let wanted = normalize_exe(exe?);
        if wanted.is_empty() {
            return None;
        }
        self.rules.iter().find(|r| r.exe == wanted)
    }

    pub fn job_for(&self, exe: Option<&str>) -> Job<'_> {
        let rule = self.rule_for(exe);
        let backend = rule.and_then(|r| r.backend.as_deref()).unwrap_or(self.backend.as_ref());
        Job {
            backend,
            system_prompt: &self.system_prompt,
            prompt_addition: rule.map(|r| r.prompt_addition.as_str()).filter(|a| !a.is_empty()),
            formatting: rule.and_then(|r| r.formatting).unwrap_or(self.formatting),
            vocabulary: &self.vocabulary,
            rule_name: rule.map(|r| r.name.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{default_config, AppRule, BackendConfig, Effort, KIND_CLAUDE_CLI};

    struct Named(String);
    impl Backend for Named {
        fn name(&self) -> &str {
            &self.0
        }
        fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
            Ok(String::new())
        }
    }

    fn builder(cfg: &BackendConfig) -> Result<Arc<dyn Backend>, BackendError> {
        Ok(Arc::new(Named(cfg.id.clone())))
    }

    fn config_with_rules(rules: Vec<AppRule>) -> Config {
        let mut config = default_config();
        config.backends.push(BackendConfig {
            id: "other".into(),
            name: "Other".into(),
            kind: KIND_CLAUDE_CLI.into(),
            base_url: None,
            model: None,
            api_key: None,
            timeout_ms: 120_000,
            effort: Effort::Low,
            expects_response: true,
        });
        config.commands[0].app_rules = rules;
        config
    }

    fn rule(exe: &str) -> AppRule {
        AppRule {
            exe: exe.into(),
            name: String::new(),
            prompt_addition: String::new(),
            backend_id: None,
            formatting: None,
        }
    }

    fn plan(config: &Config) -> CommandPlan {
        build(&config.commands[0], config, Arc::new(Vocabulary::default()), &builder).unwrap().0
    }

    #[test]
    fn without_a_matching_rule_the_command_runs_as_configured() {
        let config = config_with_rules(vec![AppRule { prompt_addition: "Formal.".into(), ..rule("OUTLOOK.EXE") }]);
        let plan = plan(&config);
        for exe in [None, Some("notepad.exe"), Some("")] {
            let job = plan.job_for(exe);
            assert_eq!(job.backend.name(), "ollama-local");
            assert_eq!(job.prompt_addition, None);
            assert_eq!(job.formatting, Formatting::Auto);
            assert_eq!(job.rule_name, None);
        }
    }

    #[test]
    fn a_matching_rule_overrides_only_what_it_sets_and_matches_case_insensitively() {
        let config = config_with_rules(vec![AppRule {
            name: "Outlook".into(),
            prompt_addition: "  Keep it formal.  ".into(),
            backend_id: Some("other".into()),
            ..rule("OUTLOOK.EXE")
        }]);
        let plan = plan(&config);
        let job = plan.job_for(Some("outlook.exe"));
        assert_eq!(job.backend.name(), "other");
        assert_eq!(job.prompt_addition, Some("Keep it formal."));
        assert_eq!(job.formatting, Formatting::Auto);
        assert_eq!(job.rule_name, Some("Outlook"));
        assert_eq!(job.system_prompt, "Fix grammar and spelling.");
    }

    #[test]
    fn a_rule_can_change_just_the_formatting() {
        let config =
            config_with_rules(vec![AppRule { formatting: Some(Formatting::Plain), ..rule("Code.exe") }]);
        let plan = plan(&config);
        let job = plan.job_for(Some("code.EXE"));
        assert_eq!(job.formatting, Formatting::Plain);
        assert_eq!(job.backend.name(), "ollama-local");
        assert_eq!(job.prompt_addition, None);
        assert_eq!(job.rule_name, Some("Code.exe"));
    }

    #[test]
    fn a_rule_written_as_a_full_path_still_matches_the_exe_name() {
        let config = config_with_rules(vec![rule(r"C:\Program Files\Microsoft Office\OUTLOOK.EXE")]);
        assert!(plan(&config).rule_for(Some("Outlook.exe")).is_some());
    }

    #[test]
    fn the_first_rule_for_an_app_wins() {
        let config = config_with_rules(vec![
            AppRule { name: "First".into(), ..rule("app.exe") },
            AppRule { name: "Second".into(), ..rule("APP.EXE") },
        ]);
        assert_eq!(plan(&config).job_for(Some("app.exe")).rule_name, Some("First"));
    }

    #[test]
    fn a_rule_whose_backend_cannot_be_built_is_dropped_with_a_problem() {
        let config = config_with_rules(vec![
            AppRule { name: "Broken".into(), backend_id: Some("ghost".into()), ..rule("a.exe") },
            AppRule { prompt_addition: "x".into(), ..rule("  ") },
            rule("b.exe"),
        ]);
        let (plan, problems) =
            build(&config.commands[0], &config, Arc::new(Vocabulary::default()), &builder).unwrap();
        assert_eq!(problems, vec!["rule for Broken: unknown backend".to_string()]);
        assert!(plan.rule_for(Some("a.exe")).is_none());
        assert!(plan.rule_for(Some("b.exe")).is_some());
        assert_eq!(plan.rules.len(), 1);
    }
}
