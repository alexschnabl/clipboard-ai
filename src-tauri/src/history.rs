
use std::{
    fs,
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FILE_NAME: &str = "history.json";

static HISTORY_LOCK: Mutex<()> = Mutex::new(());

pub const MAX_ENTRIES: usize = 100;

pub const MAX_TEXT_CHARS: usize = 4_000;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("could not write history: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not serialize history: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Outcome {
    Succeeded,
    Aborted { reason: String },
    Failed { error: String },
}

impl Outcome {
    pub fn succeeded() -> Self {
        Outcome::Succeeded
    }
    pub fn aborted(reason: impl Into<String>) -> Self {
        Outcome::Aborted { reason: reason.into() }
    }
    pub fn failed(error: impl Into<String>) -> Self {
        Outcome::Failed { error: error.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub timestamp_ms: u64,
    pub command_id: String,
    pub command_name: String,
    pub backend_name: String,
    pub outcome: Outcome,
    pub backend_ms: u64,
    #[serde(default)]
    pub total_cycle_ms: Option<u64>,
    #[serde(default)]
    pub input_text: Option<String>,
    #[serde(default)]
    pub result_text: Option<String>,
}

fn wall_clock_now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn truncate_text(text: &str) -> String {
    let total_chars = text.chars().count();
    if total_chars <= MAX_TEXT_CHARS {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(MAX_TEXT_CHARS).collect();
    truncated.push_str(&format!(
        "\n\n… [truncated: showing {MAX_TEXT_CHARS} of {total_chars} characters]"
    ));
    truncated
}

pub fn load(path: &Path) -> Vec<HistoryEntry> {
    if !path.exists() {
        return Vec::new();
    }
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            log_corrupt(path, &e.to_string());
            return Vec::new();
        }
    };
    match serde_json::from_str::<Vec<HistoryEntry>>(&raw) {
        Ok(entries) => entries,
        Err(e) => {
            log_corrupt(path, &e.to_string());
            Vec::new()
        }
    }
}

fn log_corrupt(path: &Path, reason: &str) {
    let line = format!("HISTORY corrupt-file-starting-fresh reason={reason}");
    eprintln!("{line}");
    if let Some(dir) = path.parent() {
        crate::status::append_log(dir, &line);
    }
}

pub fn append(path: &Path, mut entry: HistoryEntry) -> Result<(), HistoryError> {
    entry.timestamp_ms = wall_clock_now_ms();
    entry.input_text = entry.input_text.as_deref().map(truncate_text);
    entry.result_text = entry.result_text.as_deref().map(truncate_text);

    let _guard = crate::recover_lock(HISTORY_LOCK.lock(), "history", path.parent());
    let mut entries = load(path);
    entries.insert(0, entry);
    entries.truncate(MAX_ENTRIES);

    write_atomic(path, &entries)
}

pub fn clear(path: &Path) -> Result<(), HistoryError> {
    let _guard = crate::recover_lock(HISTORY_LOCK.lock(), "history", path.parent());
    write_atomic(path, &[])
}

fn write_atomic(path: &Path, entries: &[HistoryEntry]) -> Result<(), HistoryError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_name = match path.file_name() {
        Some(name) => format!("{}.tmp", name.to_string_lossy()),
        None => format!("{FILE_NAME}.tmp"),
    };
    let tmp_path = path.with_file_name(tmp_name);

    let json = serde_json::to_string_pretty(entries)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cai-test-history-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(command_id: &str) -> HistoryEntry {
        HistoryEntry {
            timestamp_ms: 0,
            command_id: command_id.to_string(),
            command_name: "Fix grammar".to_string(),
            backend_name: "Local Ollama".to_string(),
            outcome: Outcome::succeeded(),
            backend_ms: 1234,
            total_cycle_ms: Some(1500),
            input_text: Some("teh quick fox".to_string()),
            result_text: Some("The quick fox.".to_string()),
        }
    }

    #[test]
    fn newest_first_ordering() {
        let dir = temp_dir("ordering");
        let path = dir.join(FILE_NAME);

        append(&path, entry("first")).unwrap();
        append(&path, entry("second")).unwrap();

        let entries = load(&path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command_id, "second", "the newest entry must come first");
        assert_eq!(entries[1].command_id, "first");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn eviction_at_the_100_cap() {
        let dir = temp_dir("eviction");
        let path = dir.join(FILE_NAME);

        for i in 0..(MAX_ENTRIES + 5) {
            append(&path, entry(&format!("cmd-{i}"))).unwrap();
        }

        let entries = load(&path);
        assert_eq!(entries.len(), MAX_ENTRIES, "history must never grow past the cap");
        assert_eq!(
            entries[0].command_id,
            format!("cmd-{}", MAX_ENTRIES + 4),
            "the most recently appended entry must survive eviction"
        );
        assert_eq!(
            entries[MAX_ENTRIES - 1].command_id,
            "cmd-5",
            "the 5 oldest entries (cmd-0..cmd-4) must have been evicted, oldest first"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_file_starts_fresh_rather_than_erroring() {
        let dir = temp_dir("corrupt");
        let path = dir.join(FILE_NAME);
        fs::write(&path, "{ not valid json, trailing comma, }").unwrap();

        let entries = load(&path);

        assert!(entries.is_empty(), "a corrupt history.json must never break the caller");
        let log = fs::read_to_string(dir.join("clipboard-ai.log")).unwrap_or_default();
        assert!(
            log.contains("corrupt-file-starting-fresh"),
            "the corruption must be logged, not silently swallowed; got: {log}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_starts_fresh_without_logging_anything() {
        let dir = temp_dir("missing");
        let path = dir.join(FILE_NAME);

        let entries = load(&path);

        assert!(entries.is_empty());
        let log = fs::read_to_string(dir.join("clipboard-ai.log")).unwrap_or_default();
        assert!(
            !log.contains("corrupt"),
            "a simply-missing file is not corruption and must not log as such, got: {log}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_of_an_entry_with_all_fields() {
        let dir = temp_dir("round-trip");
        let path = dir.join(FILE_NAME);

        let original = HistoryEntry {
            timestamp_ms: 0,
            command_id: "fix-grammar".to_string(),
            command_name: "Fix grammar".to_string(),
            backend_name: "Claude (subscription)".to_string(),
            outcome: Outcome::failed("Ollama connection refused"),
            backend_ms: 4200,
            total_cycle_ms: Some(4550),
            input_text: Some("teh quick fox".to_string()),
            result_text: None,
        };

        let before = wall_clock_now_ms();
        append(&path, original.clone()).unwrap();
        let after = wall_clock_now_ms();

        let entries = load(&path);
        assert_eq!(entries.len(), 1);
        let stored = &entries[0];

        assert_eq!(stored.command_id, original.command_id);
        assert_eq!(stored.command_name, original.command_name);
        assert_eq!(stored.backend_name, original.backend_name);
        assert_eq!(stored.outcome, original.outcome);
        assert_eq!(stored.backend_ms, original.backend_ms);
        assert_eq!(stored.total_cycle_ms, original.total_cycle_ms);
        assert_eq!(stored.input_text, original.input_text);
        assert_eq!(stored.result_text, original.result_text);
        assert!(
            stored.timestamp_ms >= before && stored.timestamp_ms <= after,
            "append must stamp a current wall-clock timestamp, got {} outside [{before}, {after}]",
            stored.timestamp_ms
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_stamps_the_current_time_ignoring_whatever_the_caller_set() {
        let dir = temp_dir("stamp-override");
        let path = dir.join(FILE_NAME);

        let mut e = entry("stale-timestamp");
        e.timestamp_ms = 1;
        let before = wall_clock_now_ms();
        append(&path, e).unwrap();

        let stored = &load(&path)[0];
        assert!(
            stored.timestamp_ms >= before,
            "append must overwrite a caller-supplied timestamp with the real current time"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn long_input_text_is_truncated_with_a_visible_marker() {
        let dir = temp_dir("truncate");
        let path = dir.join(FILE_NAME);

        let long_text: String = "x".repeat(MAX_TEXT_CHARS + 500);
        let mut e = entry("big-doc");
        e.input_text = Some(long_text.clone());
        e.result_text = Some(long_text.clone());
        append(&path, e).unwrap();

        let stored = &load(&path)[0];
        let stored_input = stored.input_text.as_ref().unwrap();
        let stored_result = stored.result_text.as_ref().unwrap();

        assert!(
            stored_input.starts_with(&"x".repeat(MAX_TEXT_CHARS)),
            "the retained prefix must be exactly the first MAX_TEXT_CHARS characters"
        );
        assert!(stored_input.contains("truncated"), "truncation must be visible, not silent");
        assert!(
            stored_input.chars().count() > MAX_TEXT_CHARS,
            "the marker itself pushes stored length past the cap -- only the *content* is capped"
        );
        assert!(stored_result.contains("truncated"), "both text fields must be bounded, not just input");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncation_is_safe_on_a_multi_byte_character_boundary() {
        let dir = temp_dir("multibyte");
        let path = dir.join(FILE_NAME);

        let long_text: String = "é".repeat(MAX_TEXT_CHARS + 500);
        let mut e = entry("multibyte-doc");
        e.input_text = Some(long_text);
        append(&path, e).unwrap();

        let stored = &load(&path)[0];
        let stored_input = stored.input_text.as_ref().unwrap();
        assert!(
            stored_input.starts_with(&"é".repeat(MAX_TEXT_CHARS)),
            "the retained prefix must be exactly the first MAX_TEXT_CHARS characters"
        );
        assert!(stored_input.contains("truncated"), "truncation must be visible, not silent");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_real_append_after_a_corrupt_file_overwrites_it_with_just_the_new_entry() {
        let dir = temp_dir("corrupt-then-append");
        let path = dir.join(FILE_NAME);
        fs::write(&path, "{ not valid json, trailing comma, }").unwrap();

        append(&path, entry("first-good-entry")).unwrap();

        let entries = load(&path);
        assert_eq!(
            entries.len(),
            1,
            "the corrupt bytes must be overwritten, not merged with the new entry"
        );
        assert_eq!(entries[0].command_id, "first-good-entry");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_and_clear_survive_a_few_hundred_concurrent_iterations() {
        let dir = temp_dir("stress");
        let path = dir.join(FILE_NAME);

        let path_for_append = path.clone();
        let appender = std::thread::spawn(move || {
            for i in 0..300 {
                append(&path_for_append, entry(&format!("stress-{i}"))).unwrap();
            }
        });
        let path_for_clear = path.clone();
        let clearer = std::thread::spawn(move || {
            for _ in 0..300 {
                clear(&path_for_clear).unwrap();
            }
        });

        appender.join().unwrap();
        clearer.join().unwrap();

        let log = fs::read_to_string(dir.join("clipboard-ai.log")).unwrap_or_default();
        assert!(
            !log.contains("corrupt-file-starting-fresh"),
            "every write must land as a complete, parseable file even under concurrent access, got: {log}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn text_exactly_at_the_cap_is_not_truncated() {
        let dir = temp_dir("at-cap");
        let path = dir.join(FILE_NAME);

        let exact_text = "y".repeat(MAX_TEXT_CHARS);
        let mut e = entry("exact-fit");
        e.input_text = Some(exact_text.clone());
        append(&path, e).unwrap();

        let stored = load(&path);
        assert_eq!(
            stored[0].input_text.as_ref().unwrap(),
            &exact_text,
            "text exactly at the cap must round-trip byte-for-byte, with no marker appended"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn absent_text_and_absent_total_cycle_ms_round_trip_as_none() {
        let dir = temp_dir("absent");
        let path = dir.join(FILE_NAME);

        let mut e = entry("stuck-modifiers");
        e.outcome = Outcome::aborted("modifiers-stuck");
        e.total_cycle_ms = None;
        e.input_text = None;
        e.result_text = None;
        append(&path, e).unwrap();

        let stored = &load(&path)[0];
        assert!(stored.total_cycle_ms.is_none());
        assert!(stored.input_text.is_none());
        assert!(stored.result_text.is_none());
        assert_eq!(stored.outcome, Outcome::aborted("modifiers-stuck"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_empties_the_history() {
        let dir = temp_dir("clear");
        let path = dir.join(FILE_NAME);

        append(&path, entry("one")).unwrap();
        append(&path, entry("two")).unwrap();
        assert_eq!(load(&path).len(), 2);

        clear(&path).unwrap();

        assert!(load(&path).is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_concurrent_append_and_clear_cannot_interleave_into_a_lost_update() {
        let dir = temp_dir("race");
        let path = dir.join(FILE_NAME);

        append(&path, entry("pre-existing")).unwrap();

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));

        let path_for_append = path.clone();
        let barrier_for_append = barrier.clone();
        let append_thread = std::thread::spawn(move || {
            let _guard = HISTORY_LOCK.lock().unwrap();
            barrier_for_append.wait();
            let mut entries = load(&path_for_append);
            std::thread::sleep(std::time::Duration::from_millis(100));
            entries.insert(0, entry("racing-append"));
            write_atomic(&path_for_append, &entries).unwrap();
        });

        barrier.wait();
        std::thread::sleep(std::time::Duration::from_millis(20));
        clear(&path).unwrap();

        append_thread.join().unwrap();

        assert!(
            load(&path).is_empty(),
            "clear must not be undone by an append that had already started reading the file"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_outcome_enum_serializes_to_the_documented_wire_shape() {
        assert_eq!(
            serde_json::to_value(Outcome::succeeded()).unwrap(),
            serde_json::json!({"status": "succeeded"})
        );
        assert_eq!(
            serde_json::to_value(Outcome::aborted("capture-timeout")).unwrap(),
            serde_json::json!({"status": "aborted", "reason": "capture-timeout"})
        );
        assert_eq!(
            serde_json::to_value(Outcome::failed("Ollama connection refused")).unwrap(),
            serde_json::json!({"status": "failed", "error": "Ollama connection refused"})
        );
    }

    #[test]
    fn write_atomic_leaves_the_destination_untouched_on_failure() {
        let dir = temp_dir("atomic-failure");
        let path = dir.join(FILE_NAME);
        let original = "not valid history json, just a marker of pre-existing content";
        fs::write(&path, original).unwrap();

        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).unwrap();

        let result = append(&path, entry("should-not-land"));

        let mut perms2 = fs::metadata(&path).unwrap().permissions();
        perms2.set_readonly(false);
        let _ = fs::set_permissions(&path, perms2);
        let on_disk = fs::read_to_string(&path).unwrap();
        let tmp_leaked = dir.join(format!("{FILE_NAME}.tmp")).exists();
        let _ = fs::remove_dir_all(&dir);

        assert!(result.is_err(), "expected the rename-over-read-only-destination to fail");
        assert_eq!(
            on_disk, original,
            "a failed atomic write must never modify the pre-existing destination"
        );
        assert!(!tmp_leaked, "the temp file should be cleaned up after a failed rename");
    }
}
