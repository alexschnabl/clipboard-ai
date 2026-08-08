//! Command history: the last [`MAX_ENTRIES`] cycle runs, persisted to a
//! `history.json` file living beside `config.json` in the app config
//! directory.
//!
//! ## This writes clipboard content to disk -- on purpose
//!
//! `clipboard-ai.log` (`status.rs`) has always deliberately excluded
//! clipboard content -- `status::log_line` logs timings, the outcome
//! reason, and a backend error's `Display` text, never the text that was
//! captured or pasted. This module is a deliberate departure from that: a
//! [`HistoryEntry`] stores both the input text captured from the selection
//! and the result text that was pasted, in plaintext, in `history.json`.
//! That is not an oversight carried over from `status.rs`'s posture -- it
//! is the entire point of a history a user can open and read ("what did I
//! actually run, and what did it produce?") -- but it does mean this file
//! holds potentially sensitive text (credentials, PII, anything a user
//! selects and transforms) on disk for as long as it survives the
//! [`MAX_ENTRIES`]-entry eviction window, not just for the few seconds a
//! clipboard swap takes. Anyone auditing what this app writes to disk needs
//! to know that up front.
//!
//! ## Why a separate file, not a section of `config.json`
//!
//! `config.json` (`config.rs`) has three load-bearing properties this data
//! does not share: it is hand-editable by design (a human reading it is a
//! supported use case -- see `config::preserve_unreadable`'s doc comment),
//! it is rewritten wholesale on every settings save, and corruption is
//! preserved rather than discarded because it can hold irreplaceable
//! hand-authored data. History is none of those things -- it is
//! machine-generated, high-churn (written on every cycle, not just on a
//! settings save), and fully disposable. Mixing it into `config.json` would
//! make every unrelated settings edit rewrite the history array, make a
//! hand-edit of a hotkey a place history could be affected, and clutter the
//! one file this project's README treats as a supported hand-editing
//! surface. A sibling file with its own, much lighter durability contract
//! is the better fit -- see `.superpowers/sdd/v3-research.md` for the full
//! comparison this module follows.
//!
//! ## Durability: same atomic-write shape as `config.rs`, lighter contract
//!
//! Writes go through [`write_atomic`] -- serialize to a sibling `.tmp` file
//! in the same directory, then `rename` over the destination -- the same
//! temp-file-plus-rename primitive `config::write_atomic` uses, for the
//! same reason: a same-directory rename is atomic on Windows and POSIX, so
//! a crash or a failed write partway through can never leave a truncated or
//! half-written `history.json` behind, and existing history is never lost
//! to a failed write. It is a separate ~15-line copy rather than a shared
//! generic function, because `config.rs` is off limits for this change (see
//! the task this module was built under) -- if that constraint is ever
//! lifted, factoring the two into one generic-over-`Serialize` function
//! would be a reasonable follow-up, not a correction of anything wrong here.
//!
//! Reading is where the contract is deliberately *lighter* than
//! `config.rs`'s: [`load`] never returns an error. A missing file (first
//! run) is silently an empty history; an unreadable or unparseable one is
//! logged and *also* treated as an empty history, not preserved-and-reported
//! the way `config::preserve_unreadable` handles a broken `config.json`.
//! That machinery exists specifically for irreplaceable hand-authored data
//! -- history is regenerable by definition (the next cycle just appends to
//! whatever is there, empty or not), so the extra ceremony would protect
//! nothing.
//!
//! ## Keeping the write off the hot path
//!
//! The hotkey listener (`lib.rs`) already runs each cycle on a spawned
//! worker thread, separate from the global-shortcut callback that has to
//! stay responsive for the *next* hotkey press -- appending a history entry
//! is meant to happen on that same worker thread, **after** `cycle::run`
//! has already returned and the paste has already happened, so this
//! module's disk I/O can never add latency to the thing the user is
//! actually waiting on (the paste). Beyond that, this module does nothing
//! further to get off the hot path (no background writer thread, no
//! batching/debouncing of its own) -- the project's stated bias is against
//! that kind of complexity, and a bounded, capped-at-[`MAX_ENTRIES`]
//! rewrite is cheap enough per cycle not to need it. See the report this
//! module shipped with for the concrete call-site recommendation.
//!
//! ## Bounding stored text
//!
//! `input_text`/`result_text` are truncated to [`MAX_TEXT_CHARS`] characters
//! (see [`truncate_text`]) before being written. Without a bound, someone
//! running a command against a large document -- the spec's own example is
//! a 500 KB document -- would write that entire document into
//! `history.json` on every single cycle, and with up to [`MAX_ENTRIES`]
//! entries retained, each holding *two* such fields (input and result), an
//! unbounded file could reach many megabytes and make every subsequent
//! cycle's read-modify-write pay for reserializing all of it, on top of the
//! antivirus/cloud-sync contention `config.rs`'s own comments already flag
//! as a real concern on Windows. [`MAX_TEXT_CHARS`] is chosen to comfortably
//! hold a realistic clipboard selection this app is meant for -- a
//! paragraph, an email, a commit message -- in full, while keeping the
//! worst case (someone running a command against a huge document
//! repeatedly) bounded to a predictable, small-multiple-of-entries file
//! size instead of unbounded growth. Truncation is visible in the stored
//! text (a trailing marker naming how much was cut), not silent.
//!
//! ## Timestamps: wall clock, not `osops::now_ms()`
//!
//! `osops::now_ms()` measures elapsed milliseconds since the process
//! started (an `Instant`, with an arbitrary per-process epoch) -- exactly
//! right for measuring a cycle's *duration* (see `cycle::CycleOutcome`),
//! and exactly wrong for answering "what time did this run", which is what
//! a history list needs in order to show something like "14:32 today".
//! [`HistoryEntry::timestamp_ms`] is milliseconds since the Unix epoch
//! instead -- the same choice `status::log_line` already makes for its own
//! `ts=` field (in seconds; this uses milliseconds so two entries in the
//! same second still sort and display distinctly), so a frontend can format
//! it directly with `new Date(timestampMs)`.

use std::{
    fs,
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The file name this module's data lives under, always a sibling of
/// `config.json` in the app config directory (see `config::config_path`).
/// A `const` rather than callers each spelling `"history.json"` themselves.
pub const FILE_NAME: &str = "history.json";

/// Serializes every read-then-write access to `history.json` -- [`append`]'s
/// load-mutate-write and [`clear`]'s write -- against each other.
///
/// `clear_history` is an ordinary Tauri command with no relationship to the
/// `busy` re-entrancy flag in `lib.rs` at all (that flag only serializes
/// cycle-triggered `append` calls against *each other*). Without this lock,
/// a `clear_history` write landing between an in-flight cycle's `append`
/// reading the file and that same `append` writing it back would be
/// silently undone: `append` would overwrite the just-cleared file with the
/// pre-clear entries plus its own new one. Those entries are the text the
/// user selected and the model's output (see the module doc comment's data-
/// sensitivity note), so "I deleted that and it came back" is the specific
/// outcome this lock exists to make impossible, not just unlikely.
static HISTORY_LOCK: Mutex<()> = Mutex::new(());

/// Hard cap on stored entries. Newest first; the oldest entry is evicted
/// once a write would exceed this.
pub const MAX_ENTRIES: usize = 100;

/// Longest `input_text`/`result_text` stored per entry, in `char`s (not
/// bytes -- see [`truncate_text`]). See the module doc comment's "Bounding
/// stored text" section for the reasoning behind both the existence of a
/// cap and this specific value.
pub const MAX_TEXT_CHARS: usize = 4_000;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("could not write history: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not serialize history: {0}")]
    Json(#[from] serde_json::Error),
}

/// How one recorded run ended. Internally tagged (`"status"`) so the wire
/// shape is exactly the three cases the spec calls for -- a plain
/// `{"status":"succeeded"}`, an aborted run naming its reason, or a failed
/// run carrying the real error -- rather than a generic `pasted: bool` plus
/// loosely-related optional fields a reader would have to cross-reference
/// to make sense of.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Outcome {
    Succeeded,
    /// The cycle stopped itself deliberately before anything user-visible
    /// happened -- e.g. `cycle::CycleOutcome`'s `reason` for every abort
    /// that isn't a backend failure (`"capture-timeout"`,
    /// `"focus-changed"`, ...).
    Aborted { reason: String },
    /// The backend call itself failed. `error` is the real error text
    /// (`cycle::CycleOutcome::detail`), not just the generic
    /// `"backend-error"` reason -- see `status.rs`'s own reasoning for why
    /// that distinction matters (a bad API key, Ollama not running, and
    /// `claude` missing from PATH must not all collapse into the same
    /// unhelpful string).
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

/// One recorded run of a command.
///
/// Every text field here is a snapshot at the time the command ran, not a
/// live reference: `command_name`/`backend_name` are copied out rather than
/// looked up by id, so a later rename in `config.json` does not retroactively
/// change what an old history entry says it ran as.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Milliseconds since the Unix epoch. Always overwritten by [`append`]
    /// at write time -- see that function's doc comment -- so whatever a
    /// caller puts here before calling `append` is only meaningful for
    /// tests that exercise [`load`]/[`write_atomic`] directly.
    pub timestamp_ms: u64,
    pub command_id: String,
    /// The command's display name at the time it ran (see the struct doc
    /// comment: a snapshot, not a live lookup).
    pub command_name: String,
    /// The backend's display name at the time it ran (same reasoning).
    pub backend_name: String,
    pub outcome: Outcome,
    /// Milliseconds spent inside the backend call --
    /// `cycle::CycleOutcome::backend_ms`, always available even on an abort
    /// that never reached the backend (it is simply `0` then).
    pub backend_ms: u64,
    /// Milliseconds for the whole cycle, end to end, when the caller has
    /// it. `cycle::CycleOutcome` does not currently track this itself (only
    /// `captured_ms` and `backend_ms` separately), so this is `None` unless
    /// whoever calls [`append`] timed the whole `cycle::run` call
    /// externally -- see this module's shipping report for exactly where
    /// that would need to happen.
    #[serde(default)]
    pub total_cycle_ms: Option<u64>,
    /// What was captured from the selection. `None` when nothing was ever
    /// captured (an abort before or during capture, e.g.
    /// `"modifiers-stuck"`, `"capture-timeout"`) -- not an empty string,
    /// which would claim a capture happened and produced nothing.
    #[serde(default)]
    pub input_text: Option<String>,
    /// What was actually pasted. `None` whenever the paste did not happen
    /// -- including when a backend produced *candidate* text but the paste
    /// step itself failed or was aborted (`"paste-failed"`,
    /// `"clipboard-write-failed"`, a focus change before paste): this field
    /// means "what was pasted", not "what the backend returned".
    #[serde(default)]
    pub result_text: Option<String>,
}

/// Wall-clock milliseconds since the Unix epoch. See the module doc
/// comment's "Timestamps" section for why this, and not `osops::now_ms()`,
/// is what [`HistoryEntry::timestamp_ms`] needs.
fn wall_clock_now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Truncates `text` to [`MAX_TEXT_CHARS`] `char`s (not bytes: slicing a
/// `String` by byte count risks landing mid-UTF-8-codepoint and panicking;
/// `.chars().take(n)` cannot land anywhere but a char boundary), appending a
/// visible marker naming how much was cut. A no-op, returning `text`
/// unchanged, when it is already at or under the cap -- including no marker
/// appended, so a short entry round-trips byte-for-byte.
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

/// Loads every stored entry, newest first. Never fails: a missing file
/// (the common case -- first run, or history has been cleared) is silently
/// an empty history, and an unreadable or unparseable one is logged (see
/// `log_corrupt`) and *also* treated as empty. See the module doc comment's
/// "Durability" section for why this is deliberately lighter than
/// `config::load_or_create`'s contract.
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

/// Logs an unreadable/unparseable `history.json` the same way
/// `config.rs`'s startup-failure paths log through `status::append_log` --
/// but, unlike those, this is never more than a log line: there is no
/// preserve-the-original step (see the module doc comment) because there is
/// nothing irreplaceable to preserve.
fn log_corrupt(path: &Path, reason: &str) {
    let line = format!("HISTORY corrupt-file-starting-fresh reason={reason}");
    eprintln!("{line}");
    if let Some(dir) = path.parent() {
        crate::status::append_log(dir, &line);
    }
}

/// Appends one entry: stamps it with the current wall-clock time (see
/// [`HistoryEntry::timestamp_ms`]'s doc comment -- any timestamp the caller
/// set is discarded), bounds its text fields (see [`truncate_text`]), then
/// loads the existing history, inserts the new entry at the front, evicts
/// down to [`MAX_ENTRIES`] if needed, and writes the result back
/// atomically.
///
/// Intended to be called once per cycle, from the same worker thread that
/// ran `cycle::run` -- see the module doc comment's "Keeping the write off
/// the hot path" section -- after the cycle has already finished and the
/// paste has already happened, so this call's own latency is never on the
/// critical path the user is waiting on.
pub fn append(path: &Path, mut entry: HistoryEntry) -> Result<(), HistoryError> {
    entry.timestamp_ms = wall_clock_now_ms();
    entry.input_text = entry.input_text.as_deref().map(truncate_text);
    entry.result_text = entry.result_text.as_deref().map(truncate_text);

    // Held across the whole load-mutate-write span, not just the write --
    // see HISTORY_LOCK's doc comment. Locking only around `write_atomic`
    // would still let a `clear` land right after this `load()`, which is
    // the exact interleaving that silently undoes a clear.
    let _guard = crate::recover_lock(HISTORY_LOCK.lock(), "history", path.parent());
    let mut entries = load(path);
    entries.insert(0, entry);
    entries.truncate(MAX_ENTRIES);

    write_atomic(path, &entries)
}

/// Discards every stored entry -- writes an empty array, through the same
/// atomic path as [`append`], rather than deleting the file, so a concurrent
/// or immediately-following [`load`] never has to distinguish "empty
/// history" from "no file yet" from "file mid-deletion".
pub fn clear(path: &Path) -> Result<(), HistoryError> {
    let _guard = crate::recover_lock(HISTORY_LOCK.lock(), "history", path.parent());
    write_atomic(path, &[])
}

/// Writes `entries` to `path` atomically: serialize to a sibling `.tmp`
/// file, then `rename` it over `path`. See the module doc comment's
/// "Durability" section -- this mirrors `config::write_atomic`'s exact
/// shape and reasoning (a same-directory rename is atomic on Windows and
/// POSIX; a failed write or rename leaves `path` completely untouched
/// rather than truncated).
fn write_atomic(path: &Path, entries: &[HistoryEntry]) -> Result<(), HistoryError> {
    // `config::write_atomic` does not need this: by the time it can ever
    // run, `config::load_or_create` has already created this directory.
    // Nothing here guarantees the same ordering -- if `append` is ever
    // called before anything else has created the app config directory,
    // this is the only thing that would.
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
            timestamp_ms: 0, // append() overwrites this; see its doc comment
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
        // First run, or a history that has genuinely never been written --
        // distinct from corruption, and must not be reported as if it were.
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
            result_text: None, // a failed run pasted nothing
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
        e.timestamp_ms = 1; // an obviously-stale value a caller must not be able to force through
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
        // `truncate_text`'s doc comment promises char-boundary-safe
        // truncation specifically because slicing a `String` by byte count
        // risks landing mid-codepoint and panicking. "é" is 2 bytes but 1
        // char, so a naive byte-count truncation would be exercised here
        // where a plain ASCII fixture (see the other truncation tests)
        // could not catch it.
        let dir = temp_dir("multibyte");
        let path = dir.join(FILE_NAME);

        let long_text: String = "é".repeat(MAX_TEXT_CHARS + 500);
        let mut e = entry("multibyte-doc");
        e.input_text = Some(long_text);
        append(&path, e).unwrap(); // must not panic

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
        // Pins that a corrupt on-disk file does not somehow survive or get
        // merged with the next real write: `append`'s `load()` treats it as
        // empty (see `a_corrupt_file_starts_fresh_rather_than_erroring`),
        // and its `write_atomic` then replaces the corrupt bytes entirely.
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

    // Unlike `a_concurrent_append_and_clear_cannot_interleave_into_a_lost_update`
    // above (which reaches past `append`/`clear` to grab `HISTORY_LOCK`
    // directly, deliberately widening a too-fast-to-hit window), this calls
    // the real public `append`/`clear` with no sleeps at all, from two
    // threads, a few hundred times each. It is what actually exercises
    // `append`'s own lock acquisition -- deleting that lock line fails
    // nothing in the white-box test above, since that test takes the lock
    // itself either way. The property under test is "never panics and never
    // produces a corrupt/unparseable file", not a specific final entry
    // count: the interleaving is nondeterministic by design.
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
        // An abort before capture (e.g. "modifiers-stuck") has no input, no
        // result, and -- if the caller never measured a total -- no total
        // duration either. None of that should be coerced into an empty
        // string or a zero.
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

    // R1: a `clear_history` command landing between an in-flight `append`'s
    // `load()` and its `write_atomic` must never be silently undone. This
    // reaches inside the module (rather than calling only the public
    // `append`/`clear`) to widen `append`'s own load-then-write gap with a
    // deliberate sleep -- `append`'s real critical section is far too fast
    // to reliably land a second thread inside it otherwise -- while still
    // exercising the exact `HISTORY_LOCK` both public functions take. A
    // `Barrier` plus a short sleep on the clearing thread forces `clear` to
    // attempt its own lock acquisition while the simulated append still
    // holds it, deterministically (not just "usually") proving the two
    // cannot interleave: if `HISTORY_LOCK` were absent or scoped only around
    // `write_atomic`, `clear`'s write would land during the sleep and the
    // pre-existing entry `load()` already read would come back once the
    // simulated append's own write lands afterward.
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
            barrier_for_append.wait(); // signal: lock is held, clear may now try
            let mut entries = load(&path_for_append);
            std::thread::sleep(std::time::Duration::from_millis(100));
            entries.insert(0, entry("racing-append"));
            write_atomic(&path_for_append, &entries).unwrap();
        });

        barrier.wait();
        // Give the append thread a moment to actually be inside its sleep
        // (i.e. past `load()`, still holding the lock) before `clear` tries
        // to acquire it.
        std::thread::sleep(std::time::Duration::from_millis(20));
        clear(&path).unwrap();

        append_thread.join().unwrap();

        // Correct serialization has exactly one outcome here: `clear`'s
        // lock acquisition must block until the simulated append finishes
        // its write, so `clear` always runs strictly after it -- the
        // pre-existing entry that append's `load()` read can never survive.
        assert!(
            load(&path).is_empty(),
            "clear must not be undone by an append that had already started reading the file"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // Wire-contract test: the frontend consumes this JSON directly, so the
    // three-shape tagged encoding is worth pinning explicitly, not just
    // trusting the round-trip tests above to catch a shape regression.
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

    // Mirrors config.rs's `write_atomic_leaves_the_destination_untouched_on_failure`:
    // a failed rename (destination locked/read-only) must never leave
    // `history.json` truncated or partially written.
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
