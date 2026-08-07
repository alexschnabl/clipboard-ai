use std::{
    io::Write,
    process::{Command, Stdio},
    sync::Arc,
};

use crate::backend::{Backend, BackendError};
use crate::config::BackendConfig;

pub trait CommandRunner: Send + Sync {
    fn run(
        &self,
        program: &str,
        args: &[String],
        stdin: &str,
        timeout_ms: u64,
    ) -> Result<String, BackendError>;
}

pub struct ProcessRunner;

impl CommandRunner for ProcessRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        stdin: &str,
        _timeout_ms: u64,
    ) -> Result<String, BackendError> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| BackendError::Transport(format!("could not start {program}: {e}")))?;

        child
            .stdin
            .as_mut()
            .ok_or_else(|| BackendError::Transport("no stdin handle".into()))?
            .write_all(stdin.as_bytes())
            .map_err(|e| BackendError::Transport(e.to_string()))?;
        drop(child.stdin.take());

        let output = child
            .wait_with_output()
            .map_err(|e| BackendError::Transport(e.to_string()))?;

        if !output.status.success() {
            return Err(BackendError::BadResponse(format!(
                "{program} exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

pub struct ClaudeCli {
    name: String,
    timeout_ms: u64,
    runner: Arc<dyn CommandRunner>,
}

impl ClaudeCli {
    pub fn new(config: &BackendConfig, runner: Arc<dyn CommandRunner>) -> Self {
        Self {
            name: config.name.clone(),
            timeout_ms: config.timeout_ms,
            runner,
        }
    }
}

impl Backend for ClaudeCli {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError> {
        let args = vec![
            "-p".to_string(),
            "--system-prompt".to_string(),
            system_prompt.to_string(),
        ];
        let output = self.runner.run("claude", &args, text, self.timeout_ms)?;
        let trimmed = output.trim();
        if trimmed.is_empty() {
            return Err(BackendError::BadResponse("claude returned no output".into()));
        }
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::backend::BackendError;
    use crate::config::{BackendConfig, KIND_CLAUDE_CLI};

    struct FakeRunner {
        result: Result<String, BackendError>,
        seen: Mutex<Option<(String, Vec<String>, String)>>,
    }

    impl FakeRunner {
        fn ok(out: &str) -> Arc<Self> {
            Arc::new(Self { result: Ok(out.into()), seen: Mutex::new(None) })
        }
        fn failing(err: BackendError) -> Arc<Self> {
            Arc::new(Self { result: Err(err), seen: Mutex::new(None) })
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(
            &self,
            program: &str,
            args: &[String],
            stdin: &str,
            _timeout_ms: u64,
        ) -> Result<String, BackendError> {
            *self.seen.lock().unwrap() = Some((program.into(), args.to_vec(), stdin.into()));
            self.result.clone()
        }
    }

    fn cfg() -> BackendConfig {
        BackendConfig {
            id: "claude-sub".into(),
            name: "Claude (subscription)".into(),
            kind: KIND_CLAUDE_CLI.into(),
            base_url: None,
            model: None,
            api_key: None,
            timeout_ms: 120_000,
        }
    }

    #[test]
    fn returns_the_trimmed_stdout() {
        let backend = ClaudeCli::new(&cfg(), FakeRunner::ok("Corrected.\n"));
        assert_eq!(backend.transform("s", "t").unwrap(), "Corrected.");
    }

    #[test]
    fn invokes_claude_p_with_the_system_prompt_and_text_on_stdin() {
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&cfg(), runner.clone());
        backend.transform("Fix it.", "input text").unwrap();
        let (program, args, stdin) = runner.seen.lock().unwrap().clone().unwrap();
        assert_eq!(program, "claude");
        assert!(args.contains(&"-p".to_string()));
        let idx = args.iter().position(|a| a == "--system-prompt").unwrap();
        assert_eq!(args[idx + 1], "Fix it.");
        assert_eq!(stdin, "input text");
    }

    #[test]
    fn empty_output_is_a_backend_error() {
        let backend = ClaudeCli::new(&cfg(), FakeRunner::ok("   \n "));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::BadResponse(_))));
    }

    #[test]
    fn a_runner_timeout_propagates_as_a_backend_error() {
        let backend = ClaudeCli::new(&cfg(), FakeRunner::failing(BackendError::Timeout));
        assert!(matches!(backend.transform("s", "t"), Err(BackendError::Timeout)));
    }
}
