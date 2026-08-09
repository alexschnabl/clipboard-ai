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
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut line = format!(
        "ts={ts} {verdict} prompt={prompt_name:?} backend={backend_name:?} reason={} capture_ms={} backend_ms={}",
        outcome.reason, outcome.captured_ms, outcome.backend_ms
    );
    if let Some(detail) = &outcome.detail {
        line.push_str(&format!(" detail={detail:?}"));
    }
    line
}

const SUMMARY_MAX_BYTES: usize = 60;

pub fn summary(outcome: &CycleOutcome, prompt_name: &str) -> String {
    let text = if outcome.pasted {
        format!("{prompt_name}: ok ({} ms)", outcome.backend_ms)
    } else {
        format!("{prompt_name}: {}", outcome.reason)
    };
    truncate_for_menu(&text)
}

fn truncate_for_menu(text: &str) -> String {
    if text.len() <= SUMMARY_MAX_BYTES {
        return text.to_string();
    }
    let mut cut = SUMMARY_MAX_BYTES.saturating_sub("…".len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", &text[..cut])
}

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
        let long_name = "A".repeat(200);
        let text = summary(&outcome(true, "ok"), &long_name);
        assert!(text.len() <= 60, "menu labels must stay short, got {}", text.len());
        assert!(text.ends_with('…'), "a truncated summary should be marked, got: {text}");
    }

    #[test]
    fn log_line_never_leaks_captured_or_result_text() {
        let mut o = outcome(true, "ok");
        o.captured_text = Some("MARKER-CAPTURED-do-not-leak".to_string());
        o.result_text = Some("MARKER-RESULT-do-not-leak".to_string());
        let line = log_line(&o, "Fix grammar", "Local Ollama");
        assert!(!line.contains("MARKER-CAPTURED"), "captured_text leaked into the log line: {line}");
        assert!(!line.contains("MARKER-RESULT"), "result_text leaked into the log line: {line}");
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
