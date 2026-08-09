use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::backend::{Backend, BackendError};
use crate::config::{BackendConfig, Effort};

pub trait CommandRunner: Send + Sync {
    fn run(
        &self,
        program: &str,
        args: &[String],
        stdin: &str,
        timeout_ms: u64,
    ) -> Result<String, BackendError>;
}

fn join_with_deadline<T: Send + 'static>(
    handle: thread::JoinHandle<T>,
    deadline: Instant,
    label: &str,
) -> Result<T, BackendError> {
    loop {
        if handle.is_finished() {
            return handle
                .join()
                .map_err(|_| BackendError::Transport(format!("{label} thread panicked")));
        }
        if Instant::now() >= deadline {
            return Err(BackendError::Timeout);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

pub struct ProcessRunner;

impl CommandRunner for ProcessRunner {
    fn run(
        &self,
        program: &str,
        args: &[String],
        stdin: &str,
        timeout_ms: u64,
    ) -> Result<String, BackendError> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| BackendError::Transport(format!("could not start {program}: {e}")))?;

        let (Some(mut stdin_handle), Some(mut stdout_handle), Some(mut stderr_handle)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            return Err(BackendError::Transport("missing a stdio handle".into()));
        };

        let stdin_payload = stdin.to_string();
        let stdin_thread = thread::spawn(move || {
            let result = stdin_handle.write_all(stdin_payload.as_bytes());
            drop(stdin_handle);
            result
        });
        let stdout_thread = thread::spawn(move || {
            let mut buf = Vec::new();
            stdout_handle.read_to_end(&mut buf).map(|_| buf)
        });
        let stderr_thread = thread::spawn(move || {
            let mut buf = Vec::new();
            stderr_handle.read_to_end(&mut buf).map(|_| buf)
        });

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let wait_result = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break Err(BackendError::Timeout);
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(e) => break Err(BackendError::Transport(e.to_string())),
            }
        };

        let status = match wait_result {
            Ok(status) => status,
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        };

        let stdin_write = join_with_deadline(stdin_thread, deadline, "stdin writer")?;
        if let Err(e) = stdin_write {
            let _ = child.kill();
            return Err(BackendError::Transport(format!("failed writing stdin: {e}")));
        }

        let stdout_bytes = join_with_deadline(stdout_thread, deadline, "stdout reader")?
            .map_err(|e| {
                let _ = child.kill();
                BackendError::Transport(e.to_string())
            })?;
        let stderr_bytes = join_with_deadline(stderr_thread, deadline, "stderr reader")?
            .map_err(|e| {
                let _ = child.kill();
                BackendError::Transport(e.to_string())
            })?;

        if !status.success() {
            return Err(BackendError::BadResponse(format!(
                "{program} exited with {}: {}",
                status,
                String::from_utf8_lossy(&stderr_bytes)
            )));
        }
        Ok(String::from_utf8_lossy(&stdout_bytes).to_string())
    }
}

pub struct ClaudeCli {
    name: String,
    model: Option<String>,
    effort: Effort,
    session_persistence: bool,
    timeout_ms: u64,
    runner: Arc<dyn CommandRunner>,
}

impl ClaudeCli {
    pub fn new(config: &BackendConfig, runner: Arc<dyn CommandRunner>) -> Self {
        Self {
            name: config.name.clone(),
            model: config.model.clone().filter(|m| !m.trim().is_empty()),
            effort: config.effort,
            session_persistence: config.session_persistence,
            timeout_ms: config.timeout_ms,
            runner,
        }
    }
}

fn effort_flag_value(effort: Effort) -> Option<&'static str> {
    match effort {
        Effort::Default => None,
        Effort::Low => Some("low"),
        Effort::Medium => Some("medium"),
        Effort::High => Some("high"),
        Effort::Xhigh => Some("xhigh"),
        Effort::Max => Some("max"),
    }
}

impl Backend for ClaudeCli {
    fn name(&self) -> &str {
        &self.name
    }

    fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError> {
        let mut args = vec![
            "-p".to_string(),
            "--system-prompt".to_string(),
            system_prompt.to_string(),
        ];
        if let Some(level) = effort_flag_value(self.effort) {
            args.push("--effort".to_string());
            args.push(level.to_string());
        }
        if !self.session_persistence {
            args.push("--no-session-persistence".to_string());
        }
        if let Some(model) = &self.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }
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
    use crate::config::{BackendConfig, Effort, KIND_CLAUDE_CLI};

    struct FakeRunner {
        result: Result<String, BackendError>,
        seen: Mutex<Option<(String, Vec<String>, String, u64)>>,
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
            timeout_ms: u64,
        ) -> Result<String, BackendError> {
            *self.seen.lock().unwrap() =
                Some((program.into(), args.to_vec(), stdin.into(), timeout_ms));
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
            effort: Effort::Low,
            session_persistence: false,
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
        let (program, args, stdin, _timeout_ms) = runner.seen.lock().unwrap().clone().unwrap();
        assert_eq!(program, "claude");
        assert!(args.contains(&"-p".to_string()));
        let idx = args.iter().position(|a| a == "--system-prompt").unwrap();
        assert_eq!(args[idx + 1], "Fix it.");
        assert_eq!(stdin, "input text");
    }

    #[test]
    fn a_configured_model_is_passed_as_a_model_flag() {
        let mut config = cfg();
        config.model = Some("haiku".into());
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&config, runner.clone());
        backend.transform("s", "t").unwrap();
        let (_p, args, _stdin, _t) = runner.seen.lock().unwrap().clone().unwrap();
        let idx = args.iter().position(|a| a == "--model").expect("--model must be present");
        assert_eq!(args[idx + 1], "haiku");
    }

    #[test]
    fn no_model_flag_is_passed_when_none_is_configured() {
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&cfg(), runner.clone());
        backend.transform("s", "t").unwrap();
        let (_p, args, _stdin, _t) = runner.seen.lock().unwrap().clone().unwrap();
        assert!(!args.contains(&"--model".to_string()), "no --model without a configured one");
    }

    #[test]
    fn a_blank_model_is_treated_as_absent_not_passed_as_an_empty_flag() {
        let mut config = cfg();
        config.model = Some("   ".into());
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&config, runner.clone());
        backend.transform("s", "t").unwrap();
        let (_p, args, _stdin, _t) = runner.seen.lock().unwrap().clone().unwrap();
        assert!(!args.contains(&"--model".to_string()), "a blank model must not send --model");
    }

    #[test]
    fn effort_low_produces_the_effort_low_flag() {
        let mut config = cfg();
        config.effort = Effort::Low;
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&config, runner.clone());
        backend.transform("s", "t").unwrap();
        let (_p, args, _stdin, _t) = runner.seen.lock().unwrap().clone().unwrap();
        let idx = args.iter().position(|a| a == "--effort").expect("--effort must be present");
        assert_eq!(args[idx + 1], "low");
    }

    #[test]
    fn effort_default_produces_no_effort_flag() {
        let mut config = cfg();
        config.effort = Effort::Default;
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&config, runner.clone());
        backend.transform("s", "t").unwrap();
        let (_p, args, _stdin, _t) = runner.seen.lock().unwrap().clone().unwrap();
        assert!(!args.contains(&"--effort".to_string()), "Default must not send --effort");
    }

    #[test]
    fn effort_max_produces_the_effort_max_flag() {
        let mut config = cfg();
        config.effort = Effort::Max;
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&config, runner.clone());
        backend.transform("s", "t").unwrap();
        let (_p, args, _stdin, _t) = runner.seen.lock().unwrap().clone().unwrap();
        let idx = args.iter().position(|a| a == "--effort").expect("--effort must be present");
        assert_eq!(args[idx + 1], "max");
    }

    #[test]
    fn session_persistence_false_produces_the_no_session_persistence_flag() {
        let mut config = cfg();
        config.session_persistence = false;
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&config, runner.clone());
        backend.transform("s", "t").unwrap();
        let (_p, args, _stdin, _t) = runner.seen.lock().unwrap().clone().unwrap();
        assert!(args.contains(&"--no-session-persistence".to_string()));
    }

    #[test]
    fn session_persistence_true_produces_neither_session_flag() {
        let mut config = cfg();
        config.session_persistence = true;
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&config, runner.clone());
        backend.transform("s", "t").unwrap();
        let (_p, args, _stdin, _t) = runner.seen.lock().unwrap().clone().unwrap();
        assert!(
            !args.contains(&"--no-session-persistence".to_string()),
            "persistence on must not pass --no-session-persistence"
        );
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

    #[test]
    fn the_configured_timeout_reaches_the_runner() {
        let mut config = cfg();
        config.timeout_ms = 42_000;
        let runner = FakeRunner::ok("ok");
        let backend = ClaudeCli::new(&config, runner.clone());
        backend.transform("s", "t").unwrap();
        let (.., timeout_ms) = runner.seen.lock().unwrap().clone().unwrap();
        assert_eq!(timeout_ms, 42_000);
    }
}

#[cfg(test)]
mod join_with_deadline_tests {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use super::*;

    #[test]
    fn abandons_a_hung_thread_and_returns_timeout_promptly() {
        let deadline = Instant::now() + Duration::from_millis(150);
        let handle = thread::spawn(|| {
            thread::sleep(Duration::from_secs(30));
            42
        });
        let start = Instant::now();
        let result = join_with_deadline(handle, deadline, "test");
        let elapsed = start.elapsed();

        assert!(matches!(result, Err(BackendError::Timeout)), "expected Timeout, got {result:?}");
        assert!(
            elapsed.as_secs() < 5,
            "the join must be bounded by the deadline, not the hung thread's lifetime; took {elapsed:?}"
        );
    }

    #[test]
    fn returns_the_value_when_the_thread_finishes_before_the_deadline() {
        let deadline = Instant::now() + Duration::from_secs(5);
        let handle = thread::spawn(|| 42);
        let result = join_with_deadline(handle, deadline, "test");
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn a_panicking_thread_is_reported_as_a_transport_error() {
        let deadline = Instant::now() + Duration::from_secs(5);
        let handle = thread::spawn(|| -> i32 { panic!("boom") });
        let result = join_with_deadline(handle, deadline, "test");
        assert!(matches!(result, Err(BackendError::Transport(_))), "got {result:?}");
    }
}

#[cfg(test)]
mod process_runner_tests {
    use std::time::Instant;

    use super::*;

    fn big_payload() -> String {
        let line = "x".repeat(200);
        let mut payload = String::with_capacity(600_000);
        for i in 0..3000 {
            payload.push_str(&line);
            payload.push('-');
            payload.push_str(&i.to_string());
            payload.push_str("\r\n");
        }
        payload
    }

    #[test]
    fn large_payload_does_not_deadlock() {
        let payload = big_payload();
        assert!(payload.len() > 256 * 1024, "fixture must exceed 256 KB, got {}", payload.len());

        let args = vec!["/C".to_string(), "findstr".to_string(), "/r".to_string(), ".*".to_string()];
        let out = ProcessRunner
            .run("cmd", &args, &payload, 20_000)
            .expect("large payload round-trip should succeed, not deadlock");
        assert_eq!(out, payload, "output must come back byte-for-byte intact");
    }

    #[test]
    fn a_hanging_process_is_killed_and_returns_timeout() {
        let args = vec![
            "/C".to_string(),
            "ping".to_string(),
            "-n".to_string(),
            "31".to_string(),
            "127.0.0.1".to_string(),
        ];
        let start = Instant::now();
        let result = ProcessRunner.run("cmd", &args, "", 500);
        let elapsed = start.elapsed();

        assert!(matches!(result, Err(BackendError::Timeout)), "expected Timeout, got {result:?}");
        assert!(
            elapsed.as_secs() < 10,
            "expected the timeout to be enforced promptly, took {elapsed:?}"
        );
    }

    #[test]
    fn a_nonzero_exit_is_a_bad_response_carrying_stderr() {
        let args = vec![
            "/C".to_string(),
            "echo boom-detail 1>&2 & exit 7".to_string(),
        ];
        let result = ProcessRunner.run("cmd", &args, "", 5_000);
        match result {
            Err(BackendError::BadResponse(message)) => {
                assert!(
                    message.contains("boom-detail"),
                    "error should carry the child's stderr, got: {message}"
                );
            }
            other => panic!("expected BadResponse, got {other:?}"),
        }
    }
}
