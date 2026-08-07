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

/// Restores the clipboard when it goes out of scope, on every path.
///
/// This is the Rust equivalent of a `finally` block, and it is deliberately
/// unconditional: there is no `disarm`. Every exit from `run` — an early
/// return, or an unwinding panic from anywhere in the cycle — drops this
/// guard and restores what the user had. Making restoration a property of
/// scope rather than a step someone has to remember on each path removes the
/// entire class of "this abort forgot to restore" bug.
struct ClipboardGuard<'a> {
    ops: &'a dyn OsOps,
    original: String,
}

impl Drop for ClipboardGuard<'_> {
    fn drop(&mut self) {
        let _ = self.ops.write_clipboard(&self.original);
    }
}

/// One hotkey press, end to end.
///
/// Every exit restores the original clipboard, and no exit except the happy
/// path reaches `send_paste`.
pub fn run(backend: &dyn Backend, system_prompt: &str, ops: &dyn OsOps) -> CycleOutcome {
    let original = ops.read_clipboard().unwrap_or_default();
    let _guard = ClipboardGuard { ops, original };

    let abort = |reason: &'static str, captured_ms: u64, backend_ms: u64| CycleOutcome {
        pasted: false,
        reason,
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

    let _ = ops.clear_clipboard();
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
        Err(_) => return abort("backend-error", captured_ms, 0),
    };
    let backend_ms = ops.now_ms().saturating_sub(backend_start);

    let extracted = match sentinel::extract(&raw) {
        Some(text) => text,
        None => return abort("no-sentinel", captured_ms, backend_ms),
    };
    if extracted.trim().is_empty() {
        return abort("empty-result", captured_ms, backend_ms);
    }

    if ops.foreground_window() != focus_at_capture {
        return abort("focus-changed", captured_ms, backend_ms);
    }
    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck-before-paste", captured_ms, backend_ms);
    }

    if ops.write_clipboard(&extracted).is_err() {
        return abort("backend-error", captured_ms, backend_ms);
    }
    if ops.send_paste().is_err() {
        return abort("backend-error", captured_ms, backend_ms);
    }

    // The guard restores after this sleep, when it drops at end of scope.
    ops.sleep_ms(RESTORE_DELAY_MS);

    CycleOutcome { pasted: true, reason: "ok", captured_ms, backend_ms }
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
            Ok(self.clipboard.lock().unwrap().clone())
        }
        fn write_clipboard(&self, text: &str) -> Result<(), OsError> {
            self.log(&format!("write({text})"));
            *self.clipboard.lock().unwrap() = text.to_string();
            Ok(())
        }
        fn clear_clipboard(&self) -> Result<(), OsError> {
            self.log("clear");
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
    fn backend_error_never_pastes_and_restores() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let outcome = run(&StubBackend::failing(), "Fix.", &ops);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, "backend-error");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(*ops.clipboard.lock().unwrap(), "ORIGINAL");
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
