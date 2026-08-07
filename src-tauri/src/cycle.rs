use crate::backend::Backend;
use crate::osops::OsOps;
use crate::sentinel;

const CAPTURE_TIMEOUT_MS: u64 = 1000;
const CAPTURE_POLL_MS: u64 = 20;
const MODIFIER_TIMEOUT_MS: u64 = 2000;
const MODIFIER_POLL_MS: u64 = 10;
const RESTORE_DELAY_MS: u64 = 300;

#[derive(Debug, Clone)]
pub struct CycleOutcome {
    pub pasted: bool,
    pub reason: &'static str,
    /// R5: `reason` is `&'static str` on purpose -- every caller (tests,
    /// `status.rs`) matches or compares it by value, never by parsing free
    /// text. That is exactly what makes it structurally unable to carry a
    /// real error message: a wrong API key, Ollama not running, `claude`
    /// missing from PATH, and a genuine timeout all used to collapse into
    /// the identical `reason: "backend-error"`, with the actual cause
    /// visible nowhere -- not the log, not the tray -- forcing a user to
    /// guess. `detail` carries that cause alongside `reason` without
    /// weakening `reason`'s own guarantee. `None` whenever there is nothing
    /// more specific to say than `reason` already says.
    pub detail: Option<String>,
    pub captured_ms: u64,
    pub backend_ms: u64,
}

fn wait_for_modifier_release(ops: &dyn OsOps) -> bool {
    let start = ops.now_ms();
    loop {
        if !ops.modifiers_down() {
            return true;
        }
        if ops.now_ms().saturating_sub(start) >= MODIFIER_TIMEOUT_MS {
            return false;
        }
        ops.sleep_ms(MODIFIER_POLL_MS);
    }
}

fn wait_for_clipboard(ops: &dyn OsOps) -> Option<String> {
    let start = ops.now_ms();
    loop {
        if let Ok(value) = ops.read_clipboard() {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
        if ops.now_ms().saturating_sub(start) >= CAPTURE_TIMEOUT_MS {
            return None;
        }
        ops.sleep_ms(CAPTURE_POLL_MS);
    }
}

/// Restores the clipboard when it goes out of scope, on every path where we
/// know what to restore.
///
/// This is the Rust equivalent of a `finally` block, and it is deliberately
/// unconditional: there is no `disarm`. Every exit from `run` — an early
/// return, or an unwinding panic from anywhere in the cycle — drops this
/// guard and restores what the user had. Making restoration a property of
/// scope rather than a step someone has to remember on each path removes the
/// entire class of "this abort forgot to restore" bug.
///
/// `original` is `None` only when the very first clipboard read failed — a
/// non-text clipboard (an image, a file list) or one we could not read at
/// all. In that case we never learned what was there, so the guard leaves
/// the clipboard alone rather than guessing and overwriting it with an
/// empty string. Every other path — including a panic after a successful
/// initial read — still restores unconditionally.
struct ClipboardGuard<'a> {
    ops: &'a dyn OsOps,
    original: Option<String>,
}

impl Drop for ClipboardGuard<'_> {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            let _ = self.ops.write_clipboard(original);
        }
    }
}

/// One hotkey press, end to end.
///
/// Every exit restores the original clipboard, and no exit except the happy
/// path reaches `send_paste`.
pub fn run(backend: &dyn Backend, system_prompt: &str, ops: &dyn OsOps) -> CycleOutcome {
    // `.ok()`, not `.unwrap_or_default()`: a read failure (non-text
    // clipboard, or one we cannot read) must not be treated as "the
    // clipboard held an empty string" — see ClipboardGuard.
    let original = ops.read_clipboard().ok();
    let _guard = ClipboardGuard { ops, original };

    let abort = |reason: &'static str, captured_ms: u64, backend_ms: u64| CycleOutcome {
        pasted: false,
        reason,
        detail: None,
        captured_ms,
        backend_ms,
    };
    let abort_detail =
        |reason: &'static str, detail: String, captured_ms: u64, backend_ms: u64| CycleOutcome {
            pasted: false,
            reason,
            detail: Some(detail),
            captured_ms,
            backend_ms,
        };

    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck", 0, 0);
    }

    // Baseline focus is read before the copy is dispatched, not after: the
    // window that had focus when the user triggered the cycle is the window
    // we must still be focused on at paste time. Reading it any later would
    // miss a focus change that happens during the copy/capture step itself.
    let focus_at_capture = ops.foreground_window();

    // I2: GetForegroundWindow() returns 0 (NULL) when no window currently
    // has focus. If the baseline sample is 0 and the pre-paste sample is
    // also 0, a plain equality check reports "unchanged" and the cycle
    // would go on to paste with no known target window at all. Reject a
    // zero baseline outright instead of trusting it.
    if focus_at_capture == 0 {
        return abort("focus-changed", 0, 0);
    }

    // A failed clear is not harmless: if it fails, the clipboard still
    // holds the user's previous content, `wait_for_clipboard` below reads
    // that stale content on its very first poll, and the cycle silently
    // transforms and pastes over it instead of the real selection. Treat a
    // failed clear the same as a failed capture.
    if ops.clear_clipboard().is_err() {
        return abort("capture-timeout", 0, 0);
    }
    if ops.send_copy().is_err() {
        return abort("capture-timeout", 0, 0);
    }

    let capture_start = ops.now_ms();
    let captured = match wait_for_clipboard(ops) {
        Some(text) => text,
        None => return abort("capture-timeout", 0, 0),
    };
    let captured_ms = ops.now_ms().saturating_sub(capture_start);

    let wrapped = sentinel::wrap_system_prompt(system_prompt);
    let backend_start = ops.now_ms();
    let raw = match backend.transform(&wrapped, &captured) {
        Ok(text) => text,
        // Minor: report the real elapsed time, not a hardcoded 0 -- a
        // 120-second Ollama timeout must not log as an instant failure.
        //
        // R5: this used to be `Err(_) =>`, discarding the real
        // `BackendError` entirely -- a bad API key, Ollama not running, and
        // `claude` missing from PATH all surfaced as the identical
        // "backend-error" with nothing to tell them apart. `BackendError`'s
        // `Display` (backend/mod.rs) never interpolates a request secret
        // into any of its variants -- the API key only ever travels in an
        // HTTP header (anthropic.rs, openai.rs) or the CLI's own
        // subscription auth (claude_cli.rs), neither of which these
        // strings ever carry -- so logging it here is safe. Losing it was
        // an accident of pattern-matching, not a deliberate redaction;
        // this keeps the safety and drops the accident.
        Err(e) => {
            let backend_ms = ops.now_ms().saturating_sub(backend_start);
            return abort_detail("backend-error", e.to_string(), captured_ms, backend_ms);
        }
    };
    let backend_ms = ops.now_ms().saturating_sub(backend_start);

    let extracted = match sentinel::extract(&raw) {
        Some(text) => text,
        None => return abort("no-sentinel", captured_ms, backend_ms),
    };
    if extracted.trim().is_empty() {
        return abort("empty-result", captured_ms, backend_ms);
    }

    // The modifier gate runs before the focus check, not after: it can
    // block for up to MODIFIER_TIMEOUT_MS (2s), and a window switch during
    // that wait would otherwise go unnoticed by a focus check that already
    // ran. Checking focus last keeps the smallest possible gap between the
    // check and the paste that follows it.
    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck-before-paste", captured_ms, backend_ms);
    }
    if ops.foreground_window() != focus_at_capture {
        return abort("focus-changed", captured_ms, backend_ms);
    }

    if ops.write_clipboard(&extracted).is_err() {
        // R14: this used to also report "backend-error", contradicting the
        // I1 comment on send_paste's failure path two lines below -- which
        // argues against exactly this mislabelling, because "backend-error"
        // sends the user to debug their AI backend when the fault is
        // actually the clipboard/OS layer. Give it its own reason for the
        // same reason I1 gave "paste-failed" its own.
        return abort("clipboard-write-failed", captured_ms, backend_ms);
    }
    if ops.send_paste().is_err() {
        // I1: `WindowsOps::chord` dispatches Ctrl-down, the letter click
        // and Ctrl-up before checking any of them, then returns Err if any
        // failed -- so a chord where 'v' landed but the Ctrl release
        // failed returns Err with the paste already in flight. Once a
        // paste has been *attempted* we cannot know whether it landed, so
        // we cannot skip the restore delay: doing so lets the guard write
        // the original clipboard back microseconds later, and the target
        // app (which reads the clipboard on its own schedule) can end up
        // pasting the user's old clipboard contents over their selection.
        // A distinct reason string is used deliberately: "backend-error"
        // would send the user to debug their AI backend when the real
        // problem is synthetic input.
        ops.sleep_ms(RESTORE_DELAY_MS);
        return abort("paste-failed", captured_ms, backend_ms);
    }

    // The guard restores after this sleep, when it drops at end of scope.
    ops.sleep_ms(RESTORE_DELAY_MS);

    CycleOutcome { pasted: true, reason: "ok", detail: None, captured_ms, backend_ms }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::backend::{Backend, BackendError};
    use crate::osops::{OsError, OsOps};

    #[derive(Default)]
    struct FakeOps {
        clipboard: Mutex<String>,
        calls: Mutex<Vec<String>>,
        pasted_content: Mutex<Option<String>>,
        now: Mutex<u64>,
        capture_yields: Mutex<Option<String>>,
        modifiers_sequence: Mutex<Vec<bool>>,
        focus: Mutex<isize>,
        focus_after_capture: Mutex<Option<isize>>,
        read_should_fail: Mutex<bool>,
        clear_should_fail: Mutex<bool>,
        paste_should_fail: Mutex<bool>,
        /// R14: fails exactly the next `write_clipboard` call, then clears
        /// itself -- modelling a transient failure (e.g. another process
        /// briefly holding the clipboard) so the guard's later restore call
        /// can still be observed succeeding, the same way a real transient
        /// failure would recover on its own moments later.
        write_should_fail_once: Mutex<bool>,
    }

    impl FakeOps {
        fn new(original: &str, captured: Option<&str>) -> Self {
            let ops = Self::default();
            *ops.clipboard.lock().unwrap() = original.to_string();
            *ops.capture_yields.lock().unwrap() = captured.map(|c| c.to_string());
            *ops.focus.lock().unwrap() = 1;
            ops
        }
        fn log(&self, what: &str) {
            self.calls.lock().unwrap().push(what.to_string());
        }
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl OsOps for FakeOps {
        fn read_clipboard(&self) -> Result<String, OsError> {
            if *self.read_should_fail.lock().unwrap() {
                return Err(OsError::Clipboard("no text content available".into()));
            }
            Ok(self.clipboard.lock().unwrap().clone())
        }
        fn write_clipboard(&self, text: &str) -> Result<(), OsError> {
            self.log(&format!("write({text})"));
            let mut fail_once = self.write_should_fail_once.lock().unwrap();
            if *fail_once {
                *fail_once = false;
                return Err(OsError::Clipboard("clipboard is locked by another process".into()));
            }
            *self.clipboard.lock().unwrap() = text.to_string();
            Ok(())
        }
        fn clear_clipboard(&self) -> Result<(), OsError> {
            self.log("clear");
            if *self.clear_should_fail.lock().unwrap() {
                return Err(OsError::Clipboard("clipboard is locked by another process".into()));
            }
            *self.clipboard.lock().unwrap() = String::new();
            Ok(())
        }
        fn modifiers_down(&self) -> bool {
            let mut seq = self.modifiers_sequence.lock().unwrap();
            if seq.is_empty() { false } else { seq.remove(0) }
        }
        fn foreground_window(&self) -> isize {
            *self.focus.lock().unwrap()
        }
        fn send_copy(&self) -> Result<(), OsError> {
            self.log("copy");
            if let Some(text) = self.capture_yields.lock().unwrap().clone() {
                *self.clipboard.lock().unwrap() = text;
            }
            if let Some(next) = *self.focus_after_capture.lock().unwrap() {
                *self.focus.lock().unwrap() = next;
            }
            Ok(())
        }
        fn send_paste(&self) -> Result<(), OsError> {
            // Mirrors WindowsOps::chord: Ctrl-down, the letter click and
            // Ctrl-up are dispatched (logged) before failure is reported,
            // so a "failed" paste may already be in flight -- see I1.
            self.log("paste");
            if *self.paste_should_fail.lock().unwrap() {
                return Err(OsError::Input("ctrl release failed".into()));
            }
            *self.pasted_content.lock().unwrap() = Some(self.clipboard.lock().unwrap().clone());
            Ok(())
        }
        fn sleep_ms(&self, ms: u64) {
            self.log("sleep");
            *self.now.lock().unwrap() += ms;
        }
        fn now_ms(&self) -> u64 {
            let mut now = self.now.lock().unwrap();
            *now += 20;
            *now
        }
    }

    struct StubBackend {
        result: Result<String, BackendError>,
    }
    impl StubBackend {
        fn ok(text: &str) -> Self {
            Self { result: Ok(text.to_string()) }
        }
        fn failing() -> Self {
            Self { result: Err(BackendError::Timeout) }
        }
        fn failing_with(err: BackendError) -> Self {
            Self { result: Err(err) }
        }
    }
    impl Backend for StubBackend {
        fn name(&self) -> &str { "stub" }
        fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
            self.result.clone()
        }
    }

    #[test]
    fn happy_path_pastes_the_extracted_text() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert!(outcome.pasted);
        assert_eq!(outcome.reason, "ok");
        assert_eq!(ops.pasted_content.lock().unwrap().clone().unwrap(), "Corrected.");
    }

    #[test]
    fn happy_path_restores_the_original_clipboard() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn happy_path_call_order_is_pinned() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert_eq!(
            ops.calls(),
            vec![
                "clear".to_string(),
                "copy".to_string(),
                "write(Corrected.)".to_string(),
                "paste".to_string(),
                "sleep".to_string(),
                "write(ORIGINAL)".to_string(),
            ]
        );
    }

    #[test]
    fn capture_timeout_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", None);
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "capture-timeout");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn clear_clipboard_failure_never_pastes_and_restores() {
        // I1: a failed clear (e.g. another process holds the clipboard)
        // must not silently fall through to capturing whatever stale
        // content is still sitting in the clipboard.
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.clear_should_fail.lock().unwrap() = true;
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "capture-timeout");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn backend_error_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::failing(), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "backend-error");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn backend_error_carries_the_real_error_as_detail_not_just_the_generic_reason() {
        // R5: "backend-error" alone cannot distinguish a bad API key from
        // Ollama being down from `claude` missing from PATH -- all three
        // used to produce the identical outcome. `detail` must carry the
        // real `BackendError` so the log (and eventually the tray/settings
        // window) can actually say which one happened.
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(
            &StubBackend::failing_with(BackendError::Misconfigured("apiKey is required".into())),
            "Fix.",
            &ops,
        );
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "backend-error");
        let detail = outcome.detail.expect("a backend failure must carry a detail string");
        assert!(
            detail.contains("apiKey is required"),
            "detail must carry the real backend error, got: {detail}"
        );
    }

    #[test]
    fn a_successful_outcome_carries_no_detail() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert!(outcome.detail.is_none(), "a successful cycle has nothing extra to say");
    }

    #[test]
    fn clipboard_write_failure_gets_its_own_reason_distinct_from_backend_error() {
        // R14: this used to also report "backend-error" here, contradicting
        // the I1 comment on send_paste's failure path a few lines below --
        // which argues against exactly this mislabelling, because
        // "backend-error" sends the user to debug their AI backend when the
        // real fault is the clipboard/OS layer.
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.write_should_fail_once.lock().unwrap() = true;
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "clipboard-write-failed");
        assert_ne!(outcome.reason, "backend-error");
        assert!(
            ops.pasted_content.lock().unwrap().is_none(),
            "send_paste must never be reached once the write itself failed"
        );
        assert!(
            !ops.calls().iter().any(|c| c == "paste"),
            "the failed write must abort before any paste is attempted"
        );
        assert_eq!(
            *ops.clipboard.lock().unwrap(),
            "ORIGINAL",
            "the write failure was transient (see write_should_fail_once) -- the guard's later restore must still succeed"
        );
    }

    #[test]
    fn missing_sentinel_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("The corrected text: Corrected."), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "no-sentinel");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn truncated_sentinel_never_pastes() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected."), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "no-sentinel");
        assert!(ops.pasted_content.lock().unwrap().is_none());
    }

    #[test]
    fn stuck_modifiers_abort_before_any_clipboard_change() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.modifiers_sequence.lock().unwrap() = vec![true; 500];
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "modifiers-stuck");
        assert!(!ops.calls().contains(&"clear".to_string()));
        assert!(!ops.calls().contains(&"copy".to_string()));
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn focus_change_between_capture_and_paste_never_pastes() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.focus_after_capture.lock().unwrap() = Some(999);
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "focus-changed");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn stuck_modifiers_before_paste_never_pastes_and_restores() {
        // I3: the second modifier gate (immediately before Ctrl+V) is a
        // distinct, independently-binding check from the first one (before
        // Ctrl+C). Let the first gate see the modifiers already released,
        // then keep them stuck for the whole pre-paste wait.
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let mut seq = vec![false];
        seq.extend(std::iter::repeat(true).take(500));
        *ops.modifiers_sequence.lock().unwrap() = seq;
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "modifiers-stuck-before-paste");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert!(!ops.calls().iter().any(|c| c == "paste"));
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn a_panic_mid_cycle_still_restores_the_clipboard() {
        struct Panicking;
        impl Backend for Panicking {
            fn name(&self) -> &str { "panicking" }
            fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
                panic!("backend exploded");
            }
        }
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(&Panicking, "Fix.", &ops)
        }));
        assert!(result.is_err(), "the panic should propagate, not be swallowed");
        assert_eq!(
            *ops.clipboard.lock().unwrap(),
            "ORIGINAL",
            "the guard must restore even when the cycle unwinds"
        );
        assert!(ops.pasted_content.lock().unwrap().is_none());
    }

    #[test]
    fn unreadable_original_clipboard_is_never_overwritten_by_the_guard() {
        // I4: if the very first read fails (a non-text clipboard, e.g. an
        // image, or one we simply cannot read), the guard never learned
        // what was there. It must leave the clipboard alone rather than
        // writing an empty string over it — even on an abort path that
        // otherwise touches nothing else.
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.read_should_fail.lock().unwrap() = true;
        *ops.modifiers_sequence.lock().unwrap() = vec![true; 500];
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "modifiers-stuck");
        assert!(
            !ops.calls().iter().any(|c| c.starts_with("write(")),
            "the guard must not write anything when the original clipboard could not be read; calls were: {:?}",
            ops.calls()
        );
    }

    #[test]
    fn failing_send_paste_still_sleeps_before_the_guard_restores() {
        // I1: WindowsOps::chord dispatches Ctrl-down, the letter click and
        // Ctrl-up before checking any of them, then returns Err if any
        // failed -- so a chord where 'v' landed but the Ctrl release failed
        // returns Err with the paste already in flight. Skipping the
        // restore delay on this path lets the guard overwrite the
        // clipboard with the original before the target app (which reads
        // on its own schedule) has picked up the pasted text. The delay
        // must run on this abort just like it does on the happy path.
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.paste_should_fail.lock().unwrap() = true;
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "paste-failed");
        assert_eq!(
            ops.calls(),
            vec![
                "clear".to_string(),
                "copy".to_string(),
                "write(Corrected.)".to_string(),
                "paste".to_string(),
                "sleep".to_string(),
                "write(ORIGINAL)".to_string(),
            ],
            "the restore delay must still run before the guard restores"
        );
    }

    #[test]
    fn foreground_window_zero_at_capture_aborts_before_any_clipboard_change() {
        // I2: GetForegroundWindow() returns 0 (NULL) when no window has
        // focus. If the baseline sample is 0 and a later sample is also 0
        // (still no foreground window), `0 == 0` reports "unchanged" and
        // the cycle would paste with no known target. The baseline sample
        // itself must be rejected.
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.focus.lock().unwrap() = 0;
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "focus-changed");
        assert!(!ops.calls().contains(&"clear".to_string()));
        assert!(!ops.calls().contains(&"copy".to_string()));
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
    }

    #[test]
    fn backend_error_reports_the_real_elapsed_backend_ms_not_zero() {
        // Minor: a hardcoded 0 here means a 120-second Ollama timeout logs
        // as an instant failure, hiding exactly the information needed to
        // diagnose it.
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::failing(), "Fix.", &ops);
        assert_eq!(outcome.reason, "backend-error");
        assert!(
            outcome.backend_ms > 0,
            "expected a nonzero elapsed backend_ms, got {}",
            outcome.backend_ms
        );
    }

    #[test]
    fn the_wrapped_system_prompt_reaches_the_backend() {
        struct Capturing(Mutex<Option<String>>);
        impl Backend for Capturing {
            fn name(&self) -> &str { "capturing" }
            fn transform(&self, s: &str, _t: &str) -> Result<String, BackendError> {
                *self.0.lock().unwrap() = Some(s.to_string());
                Ok("<output>x</output>".into())
            }
        }
        let backend = Capturing(Mutex::new(None));
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        run(&backend, "Fix grammar.", &ops);
        let seen = backend.0.lock().unwrap().clone().unwrap();
        assert!(seen.contains("Fix grammar."));
        assert!(seen.contains(crate::sentinel::SENTINEL_INSTRUCTION));
    }
}
