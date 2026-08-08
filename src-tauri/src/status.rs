use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::cycle::CycleOutcome;

const MAX_LOG_BYTES: u64 = 1_048_576;

pub fn log_line(outcome: &CycleOutcome, prompt_name: &str, backend_name: &str) -> String {
    let verdict = if outcome.pasted { "PASTED" } else { "ABORTED" };
    // Unix seconds, not a formatted date: no new dependency needed, and a
    // log line with no timestamp at all is useless for correlating a
    // failure with what the user was doing at the time.
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut line = format!(
        "ts={ts} {verdict} prompt={prompt_name:?} backend={backend_name:?} reason={} capture_ms={} backend_ms={}",
        outcome.reason, outcome.captured_ms, outcome.backend_ms
    );
    // R5: this is the one place `CycleOutcome::detail` -- the real
    // `BackendError`, not just the generic `reason` -- actually reaches
    // anywhere a human can read it. `{detail:?}` (Debug, not Display) keeps
    // a multi-line stderr blob or an embedded quote from breaking the log's
    // one-line-per-entry shape.
    if let Some(detail) = &outcome.detail {
        line.push_str(&format!(" detail={detail:?}"));
    }
    line
}

pub fn summary(outcome: &CycleOutcome, prompt_name: &str) -> String {
    if outcome.pasted {
        format!("{prompt_name}: ok ({} ms)", outcome.backend_ms)
    } else {
        format!("{prompt_name}: {}", outcome.reason)
    }
}

/// Append one line to `clipboard-ai.log`, truncating first if it has grown
/// past MAX_LOG_BYTES. A logging failure is swallowed deliberately: losing a
/// log line must never take down a cycle that otherwise succeeded.
pub fn append_log(dir: &Path, line: &str) {
    let path = dir.join("clipboard-ai.log");
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = fs::remove_file(&path);
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cycle::CycleOutcome;

    fn outcome(pasted: bool, reason: &'static str) -> CycleOutcome {
        CycleOutcome {
            pasted,
            reason,
            detail: None,
            captured_ms: 42,
            backend_ms: 1234,
            // Irrelevant here: `log_line` deliberately never records the
            // user's text, only metadata and timings.
            captured_text: None,
            result_text: None,
        }
    }

    #[test]
    fn a_log_line_carries_prompt_backend_outcome_and_timings() {
        let line = log_line(&outcome(true, "ok"), "Fix grammar", "Local Ollama");
        assert!(line.contains("Fix grammar"));
        assert!(line.contains("Local Ollama"));
        assert!(line.contains("ok"));
        assert!(line.contains("42"));
        assert!(line.contains("1234"));
    }

    #[test]
    fn a_failed_cycle_is_visibly_marked_in_the_log() {
        let line = log_line(&outcome(false, "backend-error"), "Fix grammar", "Local Ollama");
        assert!(line.contains("backend-error"));
        assert!(line.contains("ABORTED"));
    }

    // R5: the log line is the one place `CycleOutcome::detail` -- the real
    // `BackendError` behind a generic "backend-error" -- must actually
    // surface, or the field is pointless plumbing.
    #[test]
    fn a_backend_error_detail_is_included_in_the_log_line() {
        let mut o = outcome(false, "backend-error");
        o.detail = Some("backend is misconfigured: apiKey is required".to_string());
        let line = log_line(&o, "Fix grammar", "Claude API");
        assert!(
            line.contains("apiKey is required"),
            "the real error must reach the log line, got: {line}"
        );
    }

    #[test]
    fn no_detail_field_appears_when_there_is_nothing_extra_to_say() {
        let line = log_line(&outcome(true, "ok"), "Fix grammar", "Local Ollama");
        assert!(
            !line.contains("detail="),
            "a successful outcome has no detail to append, got: {line}"
        );
    }

    #[test]
    fn the_tray_summary_names_the_reason_on_failure() {
        let text = summary(&outcome(false, "capture-timeout"), "Fix grammar");
        assert!(text.contains("capture-timeout"));
    }

    #[test]
    fn a_log_line_carries_a_unix_timestamp() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let line = log_line(&outcome(true, "ok"), "Fix grammar", "Local Ollama");
        assert!(line.contains("ts="), "expected a ts= field, got: {line}");
        let ts_field = line.split("ts=").nth(1).unwrap().split_whitespace().next().unwrap();
        let ts: u64 = ts_field.parse().expect("timestamp should be a plain number");
        assert!(ts >= before, "timestamp should be current, got {ts} vs baseline {before}");
    }

    #[test]
    fn the_tray_summary_is_short_enough_for_a_menu_item() {
        let text = summary(&outcome(true, "ok"), "Fix grammar");
        assert!(text.len() <= 60, "menu labels must stay short, got {}", text.len());
    }

    #[test]
    fn appending_creates_the_file_and_keeps_earlier_lines() {
        let dir = std::env::temp_dir().join(format!("clipboard-ai-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clipboard-ai.log");
        let _ = std::fs::remove_file(&path);

        append_log(&dir, "first");
        append_log(&dir, "second");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("first"));
        assert!(contents.contains("second"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
