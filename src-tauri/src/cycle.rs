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

pub fn run(backend: &dyn Backend, system_prompt: &str, ops: &dyn OsOps) -> CycleOutcome {
    let mut captured_text = None;
    let mut outcome = run_inner(backend, system_prompt, ops, &mut captured_text);
    outcome.captured_text = captured_text;
    outcome
}

fn run_inner(
    backend: &dyn Backend,
    system_prompt: &str,
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
        Some(text) => text,
        None => return abort("capture-timeout", 0, 0),
    };
    let captured_ms = ops.now_ms().saturating_sub(capture_start);
    *captured_out = Some(captured.clone());

    let wrapped = sentinel::wrap_system_prompt(system_prompt);
    let backend_start = ops.now_ms();
    let raw = match backend.transform(&wrapped, &captured) {
        Ok(text) => text,
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

    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck-before-paste", captured_ms, backend_ms);
    }
    if ops.foreground_window() != focus_at_capture {
        return abort("focus-changed", captured_ms, backend_ms);
    }

    if ops.write_clipboard(&extracted).is_err() {
        return abort("clipboard-write-failed", captured_ms, backend_ms);
    }
    if ops.send_paste().is_err() {
        ops.sleep_ms(RESTORE_DELAY_MS);
        return abort("paste-failed", captured_ms, backend_ms);
    }

    ops.sleep_ms(RESTORE_DELAY_MS);

    CycleOutcome {
        pasted: true,
        reason: "ok",
        detail: None,
        captured_ms,
        backend_ms,
        captured_text: None,
        result_text: Some(extracted),
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
    fn a_successful_cycle_carries_both_captured_and_result_text() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("<output>Corrected.</output>"), "Fix.", &ops);
        assert_eq!(outcome.captured_text.as_deref(), Some("corected"));
        assert_eq!(outcome.result_text.as_deref(), Some("Corrected."));
    }

    #[test]
    fn an_abort_after_capture_carries_the_capture_but_no_result() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::ok("no sentinel here"), "Fix.", &ops);
        assert_eq!(outcome.reason, "no-sentinel");
        assert_eq!(outcome.captured_text.as_deref(), Some("corected"));
        assert!(outcome.result_text.is_none());
    }

    #[test]
    fn an_early_abort_before_capture_carries_neither_text_field() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        *ops.modifiers_sequence.lock().unwrap() = vec![true; 500];
        let outcome = run(&StubBackend::ok("<output>x</output>"), "Fix.", &ops);
        assert_eq!(outcome.reason, "modifiers-stuck");
        assert!(outcome.captured_text.is_none());
        assert!(outcome.result_text.is_none());
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
