use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
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

        // Writing all of stdin before reading any of stdout/stderr can deadlock:
        // if the child starts producing output before it has consumed all of
        // stdin, and either OS pipe buffer fills before its far end is drained,
        // both sides block forever (this is called out in the std::process
        // docs). Doing the write and both reads on their own threads means the
        // parent (this thread) never blocks any one of them on another.
        let stdin_payload = stdin.to_string();
        let stdin_thread = thread::spawn(move || {
            let result = stdin_handle.write_all(stdin_payload.as_bytes());
            drop(stdin_handle); // close stdin so the child sees EOF
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

        // Poll for exit against a deadline so a hang is bounded rather than
        // permanent. `Child` has no wait-with-timeout in std, so we poll.
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
                // Kill the immediate child so it can't survive as an orphan.
                // This does not kill any of ITS descendants -- Windows does
                // not tear down a process tree on TerminateProcess without a
                // job object -- so a grandchild (e.g. `cmd /C ping ...`'s
                // `ping.exe`) can keep the stdout/stderr pipes open long after
                // the child we spawned is gone. The reader/writer threads are
                // therefore deliberately abandoned here rather than joined:
                // joining would block for as long as that grandchild lives,
                // turning a bounded timeout back into an unbounded hang.
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        };

        let stdin_write = stdin_thread
            .join()
            .map_err(|_| BackendError::Transport("stdin writer thread panicked".into()))?;
        if let Err(e) = stdin_write {
            let _ = child.kill();
            return Err(BackendError::Transport(format!("failed writing stdin: {e}")));
        }

        let stdout_bytes = stdout_thread
            .join()
            .map_err(|_| BackendError::Transport("stdout reader thread panicked".into()))?
            .map_err(|e| {
                let _ = child.kill();
                BackendError::Transport(e.to_string())
            })?;
        let stderr_bytes = stderr_thread
            .join()
            .map_err(|_| BackendError::Transport("stderr reader thread panicked".into()))?
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

/// These tests exercise the real `ProcessRunner`, spawning actual child processes
/// via `cmd.exe`. They are Windows-specific by construction (the commands used are
/// `cmd.exe` builtins), which matches the platform this app ships on.
#[cfg(test)]
mod process_runner_tests {
    use std::time::Instant;

    use super::*;

    /// A payload comfortably larger than any OS pipe buffer, built as many CRLF-
    /// terminated lines (not one giant line) so `findstr /r ".*"` -- which matches
    /// and re-emits input line by line -- interleaves reading stdin with writing
    /// stdout, which is what makes the write-then-read deadlock reachable.
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
        // `timeout.exe` refuses to run at all when stdin is redirected ("Input
        // redirection is not supported"), which `ProcessRunner` always does. `ping`
        // has no such restriction and reliably takes ~30s for 31 pings.
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
