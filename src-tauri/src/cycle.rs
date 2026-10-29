use crate::backend::Backend;
use crate::config::Formatting;
use crate::osops::{Clip, OsOps};
use crate::{richtext, sentinel};

const CAPTURE_TIMEOUT_MS: u64 = 1000;
const CAPTURE_POLL_MS: u64 = 20;
const MODIFIER_TIMEOUT_MS: u64 = 2000;
const MODIFIER_POLL_MS: u64 = 10;
const RESTORE_DELAY_MS: u64 = 300;
const RICH_RESTORE_DELAY_MS: u64 = 600;

fn restore_delay_ms(result: &Clip) -> u64 {
    if result.html.is_some() {
        RICH_RESTORE_DELAY_MS
    } else {
        RESTORE_DELAY_MS
    }
}

#[derive(Debug, Clone)]
pub struct CycleOutcome {
    pub pasted: bool,
    pub reason: &'static str,
    pub detail: Option<String>,
    pub captured_ms: u64,
    pub backend_ms: u64,
    pub captured_text: Option<String>,
    pub result_text: Option<String>,
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

fn wait_for_clipboard(ops: &dyn OsOps) -> Option<Clip> {
    let start = ops.now_ms();
    loop {
        if let Ok(value) = ops.read_clipboard() {
            if !value.text.trim().is_empty() {
                return Some(value);
            }
        }
        if ops.now_ms().saturating_sub(start) >= CAPTURE_TIMEOUT_MS {
            return None;
        }
        ops.sleep_ms(CAPTURE_POLL_MS);
    }
}

struct ClipboardGuard<'a> {
    ops: &'a dyn OsOps,
    original: Option<Clip>,
}

impl Drop for ClipboardGuard<'_> {
    fn drop(&mut self) {
        if let Some(original) = &self.original {
            if self.ops.write_clipboard(original).is_err() && original.html.is_some() {
                let text_only = Clip { text: original.text.clone(), html: None };
                let _ = self.ops.write_clipboard(&text_only);
            }
        }
    }
}

pub fn run(
    backend: &dyn Backend,
    system_prompt: &str,
    formatting: Formatting,
    ops: &dyn OsOps,
) -> CycleOutcome {
    let mut captured_text = None;
    let mut outcome = run_inner(backend, system_prompt, formatting, ops, &mut captured_text);
    outcome.captured_text = captured_text;
    outcome
}

fn run_inner(
    backend: &dyn Backend,
    system_prompt: &str,
    formatting: Formatting,
    ops: &dyn OsOps,
    captured_out: &mut Option<String>,
) -> CycleOutcome {
    let original = ops.read_clipboard().ok();
    let _guard = ClipboardGuard { ops, original };

    let abort = |reason: &'static str, captured_ms: u64, backend_ms: u64| CycleOutcome {
        pasted: false,
        reason,
        detail: None,
        captured_ms,
        backend_ms,
        captured_text: None,
        result_text: None,
    };
    let abort_detail =
        |reason: &'static str, detail: String, captured_ms: u64, backend_ms: u64| CycleOutcome {
            pasted: false,
            reason,
            detail: Some(detail),
            captured_ms,
            backend_ms,
            captured_text: None,
            result_text: None,
        };

    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck", 0, 0);
    }

    let focus_at_capture = ops.foreground_window();

    if focus_at_capture == 0 {
        return abort("focus-changed", 0, 0);
    }

    if ops.clear_clipboard().is_err() {
        return abort("capture-timeout", 0, 0);
    }
    if ops.send_copy().is_err() {
        return abort("capture-timeout", 0, 0);
    }

    let capture_start = ops.now_ms();
    let captured = match wait_for_clipboard(ops) {
        Some(clip) => clip,
        None => return abort("capture-timeout", 0, 0),
    };
    let captured_ms = ops.now_ms().saturating_sub(capture_start);
    *captured_out = Some(captured.text.clone());

    let prepared = richtext::prepare(&captured, formatting, backend.supports_rich_text());
    let mut prompt = system_prompt.to_string();
    if let Some(instruction) = prepared.instruction {
        prompt.push_str("\n\n");
        prompt.push_str(instruction);
    }
    let prompt = if backend.uses_sentinel() {
        sentinel::wrap_system_prompt(&prompt)
    } else {
        prompt
    };
    let backend_start = ops.now_ms();
    let raw = match backend.transform(&prompt, &prepared.text) {
        Ok(text) => text,
        Err(e) => {
            let backend_ms = ops.now_ms().saturating_sub(backend_start);
            return abort_detail("backend-error", e.to_string(), captured_ms, backend_ms);
        }
    };
    let backend_ms = ops.now_ms().saturating_sub(backend_start);

    if !backend.pastes_result() {
        return CycleOutcome {
            pasted: true,
            reason: "sent",
            detail: None,
            captured_ms,
            backend_ms,
            captured_text: None,
            result_text: None,
        };
    }

    let extracted = if backend.uses_sentinel() {
        match sentinel::extract(&raw) {
            Some(text) => text,
            None => return abort("no-sentinel", captured_ms, backend_ms),
        }
    } else {
        raw.trim().to_string()
    };
    if extracted.trim().is_empty() {
        return abort("empty-result", captured_ms, backend_ms);
    }
    let result = richtext::finish(&extracted, &prepared);
    if result.text.trim().is_empty() {
        return abort("empty-result", captured_ms, backend_ms);
    }

    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck-before-paste", captured_ms, backend_ms);
    }
    if ops.foreground_window() != focus_at_capture {
        return abort("focus-changed", captured_ms, backend_ms);
    }

    if ops.write_clipboard(&result).is_err() {
        return abort("clipboard-write-failed", captured_ms, backend_ms);
    }
    if ops.send_paste().is_err() {
        ops.sleep_ms(restore_delay_ms(&result));
        return abort("paste-failed", captured_ms, backend_ms);
    }

    ops.sleep_ms(restore_delay_ms(&result));

    CycleOutcome {
        pasted: true,
        reason: "ok",
        detail: None,
        captured_ms,
        backend_ms,
        captured_text: None,
        result_text: Some(result.text),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::backend::{Backend, BackendError};
    use crate::osops::{OsError, OsOps};

    #[derive(Default)]
    struct FakeOps {
        clipboard: Mutex<Clip>,
        calls: Mutex<Vec<String>>,
        pasted_content: Mutex<Option<String>>,
        pasted_html: Mutex<Option<String>>,
        now: Mutex<u64>,
        capture_yields: Mutex<Option<String>>,
        capture_html: Mutex<Option<String>>,
        modifiers_sequence: Mutex<Vec<bool>>,
        focus: Mutex<isize>,
        focus_after_capture: Mutex<Option<isize>>,
        read_should_fail: Mutex<bool>,
        clear_should_fail: Mutex<bool>,
        paste_should_fail: Mutex<bool>,
        write_should_fail_once: Mutex<bool>,
        rich_write_should_fail: Mutex<bool>,
    }

    impl FakeOps {
        fn new(original: &str, captured: Option<&str>) -> Self {
            let ops = Self::default();
            ops.clipboard.lock().unwrap().text = original.to_string();
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
        fn read_clipboard(&self) -> Result<Clip, OsError> {
            if *self.read_should_fail.lock().unwrap() {
                return Err(OsError::Clipboard("no text content available".into()));
            }
            Ok(self.clipboard.lock().unwrap().clone())
        }
        fn write_clipboard(&self, clip: &Clip) -> Result<(), OsError> {
            self.log(&format!("write({})", clip.text));
            if clip.html.is_some() && *self.rich_write_should_fail.lock().unwrap() {
                return Err(OsError::Clipboard("rich formats rejected".into()));
            }
            let mut fail_once = self.write_should_fail_once.lock().unwrap();
            if *fail_once {
                *fail_once = false;
                return Err(OsError::Clipboard("clipboard is locked by another process".into()));
            }
            *self.clipboard.lock().unwrap() = clip.clone();
            Ok(())
        }
        fn clear_clipboard(&self) -> Result<(), OsError> {
            self.log("clear");
            if *self.clear_should_fail.lock().unwrap() {
                return Err(OsError::Clipboard("clipboard is locked by another process".into()));
            }
            *self.clipboard.lock().unwrap() = Clip::default();
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
                let html = self.capture_html.lock().unwrap().clone();
                *self.clipboard.lock().unwrap() = Clip { text, html };
            }
            if let Some(next) = *self.focus_after_capture.lock().unwrap() {
                *self.focus.lock().unwrap() = next;
            }
            Ok(())
        }
        fn send_paste(&self) -> Result<(), OsError> {
            self.log("paste");
            if *self.paste_should_fail.lock().unwrap() {
                return Err(OsError::Input("ctrl release failed".into()));
            }
            let clip = self.clipboard.lock().unwrap().clone();
            *self.pasted_content.lock().unwrap() = Some(clip.text);
            *self.pasted_html.lock().unwrap() = clip.html;
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
        uses_sentinel: bool,
        pastes_result: bool,
    }
    impl StubBackend {
        fn ok(text: &str) -> Self {
            Self { result: Ok(text.to_string()), uses_sentinel: true, pastes_result: true }
        }
        fn failing() -> Self {
            Self { result: Err(BackendError::Timeout), uses_sentinel: true, pastes_result: true }
        }
        fn failing_with(err: BackendError) -> Self {
            Self { result: Err(err), uses_sentinel: true, pastes_result: true }
        }
        fn raw(text: &str) -> Self {
            Self { result: Ok(text.to_string()), uses_sentinel: false, pastes_result: true }
        }
        fn send_only(text: &str) -> Self {
            Self { result: Ok(text.to_string()), uses_sentinel: false, pastes_result: false }
        }
    }
    impl Backend for StubBackend {
        fn name(&self) -> &str { "stub" }
        fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
            self.result.clone()
        }
        fn uses_sentinel(&self) -> bool {
            self.uses_sentinel
        }
        fn pastes_result(&self) -> bool {
            self.pastes_result
        }
    }

    #[test]
    fn happy_path_pastes_the_extracted_text() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
        assert!(outcome.pasted);
        assert_eq!(outcome.reason, "ok");
        assert_eq!(ops.pasted_content.lock().unwrap().clone().unwrap(), "Corrected.");
    }

    #[test]
    fn happy_path_restores_the_original_clipboard() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn happy_path_call_order_is_pinned() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
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
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "capture-timeout");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn clear_clipboard_failure_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.clear_should_fail.lock().unwrap() = true;
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "capture-timeout");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn backend_error_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::failing(), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "backend-error");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn backend_error_carries_the_real_error_as_detail_not_just_the_generic_reason() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(
            &StubBackend::failing_with(BackendError::Misconfigured("apiKey is required".into())),
            "Fix.",
            Formatting::Auto,
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
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
        assert!(outcome.detail.is_none(), "a successful cycle has nothing extra to say");
    }

    #[test]
    fn clipboard_write_failure_gets_its_own_reason_distinct_from_backend_error() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.write_should_fail_once.lock().unwrap() = true;
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
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
            ops.clipboard.lock().unwrap().text,
            "ORIGINAL",
            "the write failure was transient (see write_should_fail_once) -- the guard's later restore must still succeed"
        );
    }

    #[test]
    fn missing_sentinel_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("The corrected text: Corrected."), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "no-sentinel");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn truncated_sentinel_never_pastes() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected."), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "no-sentinel");
        assert!(ops.pasted_content.lock().unwrap().is_none());
    }

    #[test]
    fn stuck_modifiers_abort_before_any_clipboard_change() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.modifiers_sequence.lock().unwrap() = vec![true; 500];
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "modifiers-stuck");
        assert!(!ops.calls().contains(&"clear".to_string()));
        assert!(!ops.calls().contains(&"copy".to_string()));
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn focus_change_between_capture_and_paste_never_pastes() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.focus_after_capture.lock().unwrap() = Some(999);
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "focus-changed");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn stuck_modifiers_before_paste_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let mut seq = vec![false];
        seq.extend(std::iter::repeat_n(true, 500));
        *ops.modifiers_sequence.lock().unwrap() = seq;
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "modifiers-stuck-before-paste");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert!(!ops.calls().iter().any(|c| c == "paste"));
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
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
            run(&Panicking, "Fix.", Formatting::Auto, &ops)
        }));
        assert!(result.is_err(), "the panic should propagate, not be swallowed");
        assert_eq!(
            ops.clipboard.lock().unwrap().text,
            "ORIGINAL",
            "the guard must restore even when the cycle unwinds"
        );
        assert!(ops.pasted_content.lock().unwrap().is_none());
    }

    #[test]
    fn unreadable_original_clipboard_is_never_overwritten_by_the_guard() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.read_should_fail.lock().unwrap() = true;
        *ops.modifiers_sequence.lock().unwrap() = vec![true; 500];
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", Formatting::Auto, &ops);
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
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.paste_should_fail.lock().unwrap() = true;
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
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
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.focus.lock().unwrap() = 0;
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "focus-changed");
        assert!(!ops.calls().contains(&"clear".to_string()));
        assert!(!ops.calls().contains(&"copy".to_string()));
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn backend_error_reports_the_real_elapsed_backend_ms_not_zero() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::failing(), "Fix.", Formatting::Auto, &ops);
        assert_eq!(outcome.reason, "backend-error");
        assert!(
            outcome.backend_ms > 0,
            "expected a nonzero elapsed backend_ms, got {}",
            outcome.backend_ms
        );
    }

    #[test]
    fn a_successful_cycle_carries_both_captured_and_result_text() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", Formatting::Auto, &ops);
        assert_eq!(outcome.captured_text.as_deref(), Some("corected"));
        assert_eq!(outcome.result_text.as_deref(), Some("Corrected."));
    }

    #[test]
    fn an_abort_after_capture_carries_the_capture_but_no_result() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("no sentinel here"), "Fix.", Formatting::Auto, &ops);
        assert_eq!(outcome.reason, "no-sentinel");
        assert_eq!(outcome.captured_text.as_deref(), Some("corected"));
        assert!(outcome.result_text.is_none());
    }

    #[test]
    fn an_early_abort_before_capture_carries_neither_text_field() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.modifiers_sequence.lock().unwrap() = vec![true; 500];
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", Formatting::Auto, &ops);
        assert_eq!(outcome.reason, "modifiers-stuck");
        assert!(outcome.captured_text.is_none());
        assert!(outcome.result_text.is_none());
    }

    #[test]
    fn a_raw_backend_pastes_the_trimmed_response_without_sentinel_extraction() {
        let ops = FakeOps::new("ORIGINAL", Some("selected"));
        let outcome = run(&StubBackend::raw("  transformed text \n"), "Fix.", Formatting::Auto, &ops);
        assert!(outcome.pasted);
        assert_eq!(outcome.reason, "ok");
        assert_eq!(ops.pasted_content.lock().unwrap().clone().unwrap(), "transformed text");
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_raw_backend_with_an_empty_response_aborts_instead_of_pasting_nothing() {
        let ops = FakeOps::new("ORIGINAL", Some("selected"));
        let outcome = run(&StubBackend::raw("   \n"), "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "empty-result");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_send_only_backend_succeeds_without_touching_paste_or_the_result_clipboard() {
        let ops = FakeOps::new("ORIGINAL", Some("selected"));
        let outcome = run(&StubBackend::send_only(""), "Fix.", Formatting::Auto, &ops);
        assert!(outcome.pasted, "a successful send-only cycle records as a success");
        assert_eq!(outcome.reason, "sent");
        assert!(outcome.result_text.is_none(), "there is no result to record");
        assert_eq!(outcome.captured_text.as_deref(), Some("selected"));
        assert!(ops.pasted_content.lock().unwrap().is_none(), "nothing must be pasted");
        assert!(
            !ops.calls().iter().any(|c| c == "paste"),
            "send_paste must never run for a send-only backend"
        );
        assert_eq!(
            ops.clipboard.lock().unwrap().text,
            "ORIGINAL",
            "the guard must still restore the original clipboard"
        );
    }

    #[test]
    fn a_send_only_backend_failure_still_reports_backend_error() {
        struct FailingSendOnly;
        impl Backend for FailingSendOnly {
            fn name(&self) -> &str { "failing-send-only" }
            fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
                Err(BackendError::BadResponse("HTTP 500".into()))
            }
            fn uses_sentinel(&self) -> bool { false }
            fn pastes_result(&self) -> bool { false }
        }
        let ops = FakeOps::new("ORIGINAL", Some("selected"));
        let outcome = run(&FailingSendOnly, "Fix.", Formatting::Auto, &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "backend-error");
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_raw_backend_receives_the_unwrapped_prompt() {
        struct Capturing(Mutex<Option<String>>);
        impl Backend for Capturing {
            fn name(&self) -> &str { "capturing-raw" }
            fn transform(&self, s: &str, _t: &str) -> Result<String, BackendError> {
                *self.0.lock().unwrap() = Some(s.to_string());
                Ok("result".into())
            }
            fn uses_sentinel(&self) -> bool { false }
        }
        let backend = Capturing(Mutex::new(None));
        let ops = FakeOps::new("ORIGINAL", Some("selected"));
        run(&backend, "Fix grammar.", Formatting::Auto, &ops);
        let seen = backend.0.lock().unwrap().clone().unwrap();
        assert_eq!(seen, "Fix grammar.", "no sentinel instruction may be appended");
        assert!(!seen.contains(crate::sentinel::SENTINEL_INSTRUCTION));
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
        run(&backend, "Fix grammar.", Formatting::Auto, &ops);
        let seen = backend.0.lock().unwrap().clone().unwrap();
        assert!(seen.contains("Fix grammar."));
        assert!(seen.contains(crate::sentinel::SENTINEL_INSTRUCTION));
    }

    #[test]
    fn an_html_selection_is_pasted_back_as_html_with_a_text_fallback() {
        let ops = FakeOps::new("ORIGINAL", Some("helo world"));
        *ops.capture_html.lock().unwrap() = Some("<p>helo <b>world</b></p>".into());
        let outcome = run(
            &StubBackend::ok("<output><p>Hello <b>world</b></p></output>"),
            "Fix.",
            Formatting::Auto,
            &ops,
        );
        assert!(outcome.pasted, "aborted with {}", outcome.reason);
        assert_eq!(
            ops.pasted_html.lock().unwrap().as_deref(),
            Some("<p>Hello <b>world</b></p>")
        );
        assert_eq!(ops.pasted_content.lock().unwrap().as_deref(), Some("Hello world"));
        assert_eq!(outcome.captured_text.as_deref(), Some("helo world"));
        assert_eq!(outcome.result_text.as_deref(), Some("Hello world"));
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn plain_formatting_ignores_the_html_on_the_clipboard() {
        let ops = FakeOps::new("ORIGINAL", Some("helo world"));
        *ops.capture_html.lock().unwrap() = Some("<p>helo <b>world</b></p>".into());
        let outcome = run(
            &StubBackend::ok("<output>Hello world</output>"),
            "Fix.",
            Formatting::Plain,
            &ops,
        );
        assert!(outcome.pasted);
        assert!(ops.pasted_html.lock().unwrap().is_none());
        assert_eq!(ops.pasted_content.lock().unwrap().as_deref(), Some("Hello world"));
    }

    #[test]
    fn the_keep_instruction_and_simplified_html_reach_the_backend_only_for_html_selections() {
        struct Capturing(Mutex<Option<(String, String)>>);
        impl Backend for Capturing {
            fn name(&self) -> &str { "capturing" }
            fn transform(&self, s: &str, t: &str) -> Result<String, BackendError> {
                *self.0.lock().unwrap() = Some((s.to_string(), t.to_string()));
                Ok("<output><p>x</p></output>".into())
            }
        }
        let backend = Capturing(Mutex::new(None));
        let ops = FakeOps::new("ORIGINAL", Some("helo"));
        *ops.capture_html.lock().unwrap() =
            Some("<p style=\"mso-x: y; color: red\"><b>helo</b></p>".into());
        run(&backend, "Fix.", Formatting::Auto, &ops);
        let (prompt, text) = backend.0.lock().unwrap().clone().unwrap();
        assert!(prompt.contains(crate::richtext::KEEP_INSTRUCTION));
        assert!(prompt.contains(crate::sentinel::SENTINEL_INSTRUCTION));
        assert_eq!(text, "<p style=\"color: red\"><b>helo</b></p>");

        let backend = Capturing(Mutex::new(None));
        let ops = FakeOps::new("ORIGINAL", Some("helo"));
        run(&backend, "Fix.", Formatting::Auto, &ops);
        let (prompt, text) = backend.0.lock().unwrap().clone().unwrap();
        assert!(!prompt.contains(crate::richtext::KEEP_INSTRUCTION));
        assert_eq!(text, "helo");
    }

    #[test]
    fn a_rich_original_clipboard_is_restored_with_its_html() {
        let ops = FakeOps::new("ORIGINAL", Some("helo"));
        ops.clipboard.lock().unwrap().html = Some("<i>ORIGINAL</i>".into());
        run(&StubBackend::ok("<output>Hello</output>"), "Fix.", Formatting::Auto, &ops);
        let restored = ops.clipboard.lock().unwrap().clone();
        assert_eq!(restored.text, "ORIGINAL");
        assert_eq!(restored.html.as_deref(), Some("<i>ORIGINAL</i>"));
    }

    #[test]
    fn a_backend_without_rich_support_gets_plain_text_even_in_auto() {
        struct PlainOnly(Mutex<Option<String>>);
        impl Backend for PlainOnly {
            fn name(&self) -> &str { "plain-only" }
            fn transform(&self, _s: &str, t: &str) -> Result<String, BackendError> {
                *self.0.lock().unwrap() = Some(t.to_string());
                Ok("result".into())
            }
            fn uses_sentinel(&self) -> bool { false }
            fn supports_rich_text(&self) -> bool { false }
        }
        let backend = PlainOnly(Mutex::new(None));
        let ops = FakeOps::new("ORIGINAL", Some("helo"));
        *ops.capture_html.lock().unwrap() = Some("<p><b>helo</b></p>".into());
        let outcome = run(&backend, "Fix.", Formatting::Auto, &ops);
        assert!(outcome.pasted);
        assert_eq!(backend.0.lock().unwrap().as_deref(), Some("helo"));
        assert!(ops.pasted_html.lock().unwrap().is_none());
    }

    #[test]
    fn an_html_result_whose_text_is_empty_aborts_instead_of_pasting_empty_markup() {
        let ops = FakeOps::new("ORIGINAL", Some("helo"));
        *ops.capture_html.lock().unwrap() = Some("<p><b>helo</b></p>".into());
        let outcome = run(
            &StubBackend::ok("<output><p></p></output>"),
            "Fix.",
            Formatting::Auto,
            &ops,
        );
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "empty-result");
        assert!(ops.pasted_content.lock().unwrap().is_none());
    }

    #[test]
    fn a_failed_rich_restore_falls_back_to_restoring_the_text() {
        let ops = FakeOps::new("ORIGINAL", Some("helo"));
        ops.clipboard.lock().unwrap().html = Some("<i>ORIGINAL</i>".into());
        *ops.rich_write_should_fail.lock().unwrap() = true;
        run(&StubBackend::ok("<output>Hello</output>"), "Fix.", Formatting::Auto, &ops);
        let restored = ops.clipboard.lock().unwrap().clone();
        assert_eq!(restored.text, "ORIGINAL", "the text must come back even when the rich write fails");
        assert!(restored.html.is_none());
    }
}
