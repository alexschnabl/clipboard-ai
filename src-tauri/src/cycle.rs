use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::Duration;

use crate::backend::{Backend, BackendError};
use crate::config::Formatting;
use crate::osops::{Clip, OsOps};
use crate::{richtext, sentinel};

const CAPTURE_TIMEOUT_MS: u64 = 1000;
const CAPTURE_POLL_MS: u64 = 20;
const MODIFIER_TIMEOUT_MS: u64 = 2000;
const MODIFIER_POLL_MS: u64 = 10;
const RESTORE_DELAY_MS: u64 = 300;
const RICH_RESTORE_DELAY_MS: u64 = 600;
const CANCEL_POLL_MS: u64 = 25;

pub const REASON_CANCELLED: &str = "cancelled";
pub const REASON_FOCUS_RESTORE_FAILED: &str = "focus-restore-failed";

static NOT_CANCELLED: AtomicBool = AtomicBool::new(false);

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
    pub result_html: Option<String>,
}

impl CycleOutcome {
    pub fn result_clip(&self) -> Option<Clip> {
        if !self.pasted || self.reason != "ok" {
            return None;
        }
        self.result_text
            .as_ref()
            .map(|text| Clip { text: text.clone(), html: self.result_html.clone() })
    }
}

#[derive(Debug, Clone)]
pub struct Captured {
    pub clip: Clip,
    pub window: isize,
    pub captured_ms: u64,
}

type Call<'a> = &'a dyn Fn(&str, &str) -> Option<Result<String, BackendError>>;

fn abort(reason: &'static str, captured_ms: u64, backend_ms: u64) -> CycleOutcome {
    CycleOutcome {
        pasted: false,
        reason,
        detail: None,
        captured_ms,
        backend_ms,
        captured_text: None,
        result_text: None,
        result_html: None,
    }
}

fn abort_detail(
    reason: &'static str,
    detail: String,
    captured_ms: u64,
    backend_ms: u64,
) -> CycleOutcome {
    CycleOutcome { detail: Some(detail), ..abort(reason, captured_ms, backend_ms) }
}

fn is_cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::SeqCst)
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

impl<'a> ClipboardGuard<'a> {
    fn new(ops: &'a dyn OsOps) -> Self {
        Self { ops, original: ops.read_clipboard().ok() }
    }
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

#[cfg(test)]
pub fn run(
    backend: &dyn Backend,
    system_prompt: &str,
    formatting: Formatting,
    ops: &dyn OsOps,
) -> CycleOutcome {
    let call = |prompt: &str, text: &str| Some(backend.transform(prompt, text));
    run_with(backend, &call, system_prompt, formatting, ops, &NOT_CANCELLED)
}

pub fn run_cancellable(
    backend: &Arc<dyn Backend>,
    system_prompt: &str,
    formatting: Formatting,
    ops: &dyn OsOps,
    cancel: &AtomicBool,
) -> CycleOutcome {
    let call = |prompt: &str, text: &str| call_cancellable(backend, prompt, text, cancel);
    run_with(backend.as_ref(), &call, system_prompt, formatting, ops, cancel)
}

fn run_with(
    backend: &dyn Backend,
    call: Call<'_>,
    system_prompt: &str,
    formatting: Formatting,
    ops: &dyn OsOps,
    cancel: &AtomicBool,
) -> CycleOutcome {
    let _guard = ClipboardGuard::new(ops);
    match capture(ops, cancel) {
        Err(outcome) => outcome,
        Ok(captured) => {
            let mut outcome =
                transform_and_paste(backend, call, system_prompt, formatting, ops, &captured, cancel);
            outcome.captured_text = Some(captured.clip.text);
            outcome
        }
    }
}

pub fn capture_selection(ops: &dyn OsOps) -> Result<Captured, CycleOutcome> {
    let _guard = ClipboardGuard::new(ops);
    capture(ops, &NOT_CANCELLED)
}

pub fn run_on_captured(
    backend: &Arc<dyn Backend>,
    system_prompt: &str,
    formatting: Formatting,
    ops: &dyn OsOps,
    captured: &Captured,
    cancel: &AtomicBool,
) -> CycleOutcome {
    let call = |prompt: &str, text: &str| call_cancellable(backend, prompt, text, cancel);
    run_on_captured_with(backend.as_ref(), &call, system_prompt, formatting, ops, captured, cancel)
}

fn run_on_captured_with(
    backend: &dyn Backend,
    call: Call<'_>,
    system_prompt: &str,
    formatting: Formatting,
    ops: &dyn OsOps,
    captured: &Captured,
    cancel: &AtomicBool,
) -> CycleOutcome {
    let _guard = ClipboardGuard::new(ops);
    let mut outcome = if captured.window == 0 || ops.foreground_window() != captured.window {
        abort(REASON_FOCUS_RESTORE_FAILED, captured.captured_ms, 0)
    } else {
        transform_and_paste(backend, call, system_prompt, formatting, ops, captured, cancel)
    };
    outcome.captured_text = Some(captured.clip.text.clone());
    outcome
}

pub fn paste_clip(ops: &dyn OsOps, clip: &Clip, expected_window: Option<isize>) -> CycleOutcome {
    let _guard = ClipboardGuard::new(ops);
    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck-before-paste", 0, 0);
    }
    let window = ops.foreground_window();
    if window == 0 || expected_window.is_some_and(|expected| expected != window) {
        return abort("focus-changed", 0, 0);
    }
    if ops.write_clipboard(clip).is_err() {
        return abort("clipboard-write-failed", 0, 0);
    }
    if ops.send_paste().is_err() {
        ops.sleep_ms(restore_delay_ms(clip));
        return abort("paste-failed", 0, 0);
    }
    ops.sleep_ms(restore_delay_ms(clip));
    CycleOutcome {
        pasted: true,
        result_text: Some(clip.text.clone()),
        result_html: clip.html.clone(),
        ..abort("ok", 0, 0)
    }
}

fn call_cancellable(
    backend: &Arc<dyn Backend>,
    prompt: &str,
    text: &str,
    cancel: &AtomicBool,
) -> Option<Result<String, BackendError>> {
    let (tx, rx) = mpsc::channel();
    let backend = Arc::clone(backend);
    let (prompt, text) = (prompt.to_string(), text.to_string());
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend.transform(&prompt, &text)
        }));
        let _ = tx.send(result);
    });
    loop {
        if is_cancelled(cancel) {
            return None;
        }
        match rx.recv_timeout(Duration::from_millis(CANCEL_POLL_MS)) {
            Ok(Ok(result)) => return Some(result),
            Ok(Err(panic)) => std::panic::resume_unwind(panic),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Some(Err(BackendError::Transport(
                    "the backend thread ended without a result".into(),
                )))
            }
        }
    }
}

fn capture(ops: &dyn OsOps, cancel: &AtomicBool) -> Result<Captured, CycleOutcome> {
    if !wait_for_modifier_release(ops) {
        return Err(abort("modifiers-stuck", 0, 0));
    }
    if is_cancelled(cancel) {
        return Err(abort(REASON_CANCELLED, 0, 0));
    }

    let window = ops.foreground_window();
    if window == 0 {
        return Err(abort("focus-changed", 0, 0));
    }

    if ops.clear_clipboard().is_err() {
        return Err(abort("capture-timeout", 0, 0));
    }
    if ops.send_copy().is_err() {
        return Err(abort("capture-timeout", 0, 0));
    }

    let capture_start = ops.now_ms();
    let clip = match wait_for_clipboard(ops) {
        Some(clip) => clip,
        None => return Err(abort("capture-timeout", 0, 0)),
    };
    let captured_ms = ops.now_ms().saturating_sub(capture_start);
    Ok(Captured { clip, window, captured_ms })
}

fn transform_and_paste(
    backend: &dyn Backend,
    call: Call<'_>,
    system_prompt: &str,
    formatting: Formatting,
    ops: &dyn OsOps,
    captured: &Captured,
    cancel: &AtomicBool,
) -> CycleOutcome {
    let captured_ms = captured.captured_ms;
    let prepared = richtext::prepare(&captured.clip, formatting, backend.supports_rich_text());
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
    if is_cancelled(cancel) {
        return abort(REASON_CANCELLED, captured_ms, 0);
    }
    let backend_start = ops.now_ms();
    let raw = match call(&prompt, &prepared.text) {
        None => {
            let backend_ms = ops.now_ms().saturating_sub(backend_start);
            return abort(REASON_CANCELLED, captured_ms, backend_ms);
        }
        Some(Ok(text)) => text,
        Some(Err(e)) => {
            let backend_ms = ops.now_ms().saturating_sub(backend_start);
            return abort_detail("backend-error", e.to_string(), captured_ms, backend_ms);
        }
    };
    let backend_ms = ops.now_ms().saturating_sub(backend_start);

    if !backend.pastes_result() {
        return CycleOutcome { pasted: true, ..abort("sent", captured_ms, backend_ms) };
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

    if is_cancelled(cancel) {
        return abort(REASON_CANCELLED, captured_ms, backend_ms);
    }
    if !wait_for_modifier_release(ops) {
        return abort("modifiers-stuck-before-paste", captured_ms, backend_ms);
    }
    if is_cancelled(cancel) {
        return abort(REASON_CANCELLED, captured_ms, backend_ms);
    }
    if ops.foreground_window() != captured.window {
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
        result_text: Some(result.text),
        result_html: result.html,
        ..abort("ok", captured_ms, backend_ms)
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

    fn arc(backend: impl Backend + 'static) -> Arc<dyn Backend> {
        Arc::new(backend)
    }

    struct Slow {
        delay_ms: u64,
        result: String,
    }
    impl Backend for Slow {
        fn name(&self) -> &str { "slow" }
        fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
            std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
            Ok(self.result.clone())
        }
    }

    #[test]
    fn the_cancellable_run_behaves_like_run_when_nobody_cancels() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let cancel = AtomicBool::new(false);
        let outcome = run_cancellable(
            &arc(StubBackend::ok("<output>Corrected.</output>")),
            "Fix.",
            Formatting::Auto,
            &ops,
            &cancel,
        );
        assert!(outcome.pasted, "aborted with {}", outcome.reason);
        assert_eq!(ops.pasted_content.lock().unwrap().as_deref(), Some("Corrected."));
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_cancel_before_the_run_starts_touches_nothing() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let cancel = AtomicBool::new(true);
        let outcome =
            run_cancellable(&arc(StubBackend::ok("<output>x</output>")), "Fix.", Formatting::Auto, &ops, &cancel);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, REASON_CANCELLED);
        assert!(!ops.calls().contains(&"clear".to_string()));
        assert!(!ops.calls().contains(&"copy".to_string()));
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_cancel_while_the_backend_works_returns_promptly_and_never_pastes() {
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let cancel = Arc::new(AtomicBool::new(false));
        let setter = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(60));
            setter.store(true, Ordering::SeqCst);
        });
        let started = std::time::Instant::now();
        let outcome = run_cancellable(
            &arc(Slow { delay_ms: 5_000, result: "<output>late</output>".into() }),
            "Fix.",
            Formatting::Auto,
            &ops,
            &cancel,
        );
        assert!(
            started.elapsed() < std::time::Duration::from_millis(2_000),
            "the run must not wait for the backend once cancelled, took {:?}",
            started.elapsed()
        );
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, REASON_CANCELLED);
        assert_eq!(outcome.captured_text.as_deref(), Some("corected"));
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert!(!ops.calls().iter().any(|c| c == "paste"));
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_cancel_that_lands_after_the_answer_still_prevents_the_paste() {
        struct CancelsWhileAnswering(Arc<AtomicBool>);
        impl Backend for CancelsWhileAnswering {
            fn name(&self) -> &str { "cancels" }
            fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
                self.0.store(true, Ordering::SeqCst);
                Ok("<output>Corrected.</output>".into())
            }
        }
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let cancel = Arc::new(AtomicBool::new(false));
        let backend = CancelsWhileAnswering(cancel.clone());
        let call = |p: &str, t: &str| Some(backend.transform(p, t));
        let outcome = run_with(&backend, &call, "Fix.", Formatting::Auto, &ops, &cancel);
        assert_eq!(outcome.reason, REASON_CANCELLED);
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_backend_panic_in_the_cancellable_run_still_propagates_and_restores() {
        struct Panicking;
        impl Backend for Panicking {
            fn name(&self) -> &str { "panicking" }
            fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
                panic!("backend exploded");
            }
        }
        let ops = FakeOps::new("ORIGINAL", Some("corected"));
        let cancel = AtomicBool::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_cancellable(&arc(Panicking), "Fix.", Formatting::Auto, &ops, &cancel)
        }));
        assert!(result.is_err());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn capture_selection_hands_back_the_selection_and_restores_the_clipboard_at_once() {
        let ops = FakeOps::new("ORIGINAL", Some("selected text"));
        *ops.capture_html.lock().unwrap() = Some("<b>selected</b> text".into());
        let captured = capture_selection(&ops).expect("a selection was there to copy");
        assert_eq!(captured.clip.text, "selected text");
        assert_eq!(captured.clip.html.as_deref(), Some("<b>selected</b> text"));
        assert_eq!(captured.window, 1);
        assert_eq!(
            ops.clipboard.lock().unwrap().text,
            "ORIGINAL",
            "the user's clipboard must be back while the palette is open"
        );
        assert!(!ops.calls().iter().any(|c| c == "paste"));
    }

    #[test]
    fn capture_selection_reports_why_when_nothing_was_selected() {
        let ops = FakeOps::new("ORIGINAL", None);
        let outcome = capture_selection(&ops).expect_err("nothing to copy");
        assert_eq!(outcome.reason, "capture-timeout");
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    fn captured(window: isize) -> Captured {
        Captured {
            clip: Clip { text: "corected".into(), html: None },
            window,
            captured_ms: 7,
        }
    }

    #[test]
    fn a_palette_run_pastes_into_the_captured_window_once_it_is_back_in_front() {
        let ops = FakeOps::new("ORIGINAL", None);
        let cancel = AtomicBool::new(false);
        let backend = StubBackend::ok("<output>Corrected.</output>");
        let call = |p: &str, t: &str| Some(backend.transform(p, t));
        let outcome =
            run_on_captured_with(&backend, &call, "Fix.", Formatting::Auto, &ops, &captured(1), &cancel);
        assert!(outcome.pasted, "aborted with {}", outcome.reason);
        assert_eq!(outcome.captured_text.as_deref(), Some("corected"));
        assert_eq!(ops.pasted_content.lock().unwrap().as_deref(), Some("Corrected."));
        assert!(
            !ops.calls().iter().any(|c| c == "copy" || c == "clear"),
            "the selection was captured before the palette opened, never again"
        );
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_palette_run_refuses_to_start_when_the_target_window_is_not_in_front() {
        struct MustNotRun;
        impl Backend for MustNotRun {
            fn name(&self) -> &str { "must-not-run" }
            fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
                panic!("the backend must not be called when focus never came back");
            }
        }
        let ops = FakeOps::new("ORIGINAL", None);
        *ops.focus.lock().unwrap() = 42;
        let cancel = AtomicBool::new(false);
        let call = |p: &str, t: &str| Some(MustNotRun.transform(p, t));
        let outcome =
            run_on_captured_with(&MustNotRun, &call, "Fix.", Formatting::Auto, &ops, &captured(1), &cancel);
        assert!(!outcome.pasted);
        assert_eq!(outcome.reason, REASON_FOCUS_RESTORE_FAILED);
        assert_eq!(outcome.captured_text.as_deref(), Some("corected"));
        assert!(ops.pasted_content.lock().unwrap().is_none());
    }

    #[test]
    fn a_palette_run_still_aborts_when_the_user_switches_away_during_the_backend_call() {
        struct SwitchesAway<'a>(&'a FakeOps);
        impl Backend for SwitchesAway<'_> {
            fn name(&self) -> &str { "switches" }
            fn transform(&self, _s: &str, _t: &str) -> Result<String, BackendError> {
                *self.0.focus.lock().unwrap() = 77;
                Ok("<output>Corrected.</output>".into())
            }
        }
        let ops = FakeOps::new("ORIGINAL", None);
        let cancel = AtomicBool::new(false);
        let backend = SwitchesAway(&ops);
        let call = |p: &str, t: &str| Some(backend.transform(p, t));
        let outcome =
            run_on_captured_with(&backend, &call, "Fix.", Formatting::Auto, &ops, &captured(1), &cancel);
        assert_eq!(outcome.reason, "focus-changed");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn a_palette_run_with_a_null_target_never_matches_a_missing_foreground() {
        let ops = FakeOps::new("ORIGINAL", None);
        *ops.focus.lock().unwrap() = 0;
        let cancel = AtomicBool::new(false);
        let backend = StubBackend::ok("<output>x</output>");
        let call = |p: &str, t: &str| Some(backend.transform(p, t));
        let outcome =
            run_on_captured_with(&backend, &call, "Fix.", Formatting::Auto, &ops, &captured(0), &cancel);
        assert_eq!(outcome.reason, REASON_FOCUS_RESTORE_FAILED);
    }

    #[test]
    fn a_successful_rich_run_carries_the_html_for_paste_again() {
        let ops = FakeOps::new("ORIGINAL", Some("helo world"));
        *ops.capture_html.lock().unwrap() = Some("<p>helo <b>world</b></p>".into());
        let outcome = run(
            &StubBackend::ok("<output><p>Hello <b>world</b></p></output>"),
            "Fix.",
            Formatting::Auto,
            &ops,
        );
        let clip = outcome.result_clip().expect("a pasted run has a result to keep");
        assert_eq!(clip.text, "Hello world");
        assert_eq!(clip.html.as_deref(), Some("<p>Hello <b>world</b></p>"));
    }

    #[test]
    fn only_a_real_paste_leaves_a_result_to_paste_again() {
        let ops = FakeOps::new("ORIGINAL", Some("selected"));
        let sent = run(&StubBackend::send_only(""), "Fix.", Formatting::Auto, &ops);
        assert!(sent.result_clip().is_none(), "a send-only run pasted nothing");
        let ops = FakeOps::new("ORIGINAL", Some("selected"));
        let failed = run(&StubBackend::failing(), "Fix.", Formatting::Auto, &ops);
        assert!(failed.result_clip().is_none());
    }

    #[test]
    fn paste_clip_pastes_the_rich_result_and_restores_the_clipboard() {
        let ops = FakeOps::new("ORIGINAL", None);
        let clip = Clip { text: "Hello world".into(), html: Some("<b>Hello</b> world".into()) };
        let outcome = paste_clip(&ops, &clip, None);
        assert!(outcome.pasted);
        assert_eq!(ops.pasted_content.lock().unwrap().as_deref(), Some("Hello world"));
        assert_eq!(ops.pasted_html.lock().unwrap().as_deref(), Some("<b>Hello</b> world"));
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
        assert_eq!(
            ops.calls(),
            vec!["write(Hello world)", "paste", "sleep", "write(ORIGINAL)"],
            "paste-again never copies, so it can never clobber a selection"
        );
    }

    #[test]
    fn paste_clip_waits_out_the_hotkey_modifiers_and_gives_up_if_they_stay_down() {
        let ops = FakeOps::new("ORIGINAL", None);
        *ops.modifiers_sequence.lock().unwrap() = vec![true; 500];
        let clip = Clip { text: "x".into(), html: None };
        let outcome = paste_clip(&ops, &clip, None);
        assert_eq!(outcome.reason, "modifiers-stuck-before-paste");
        assert!(ops.pasted_content.lock().unwrap().is_none());
        assert_eq!(ops.clipboard.lock().unwrap().text, "ORIGINAL");
    }

    #[test]
    fn paste_clip_refuses_a_window_other_than_the_expected_one() {
        let ops = FakeOps::new("ORIGINAL", None);
        *ops.focus.lock().unwrap() = 5;
        let clip = Clip { text: "x".into(), html: None };
        let outcome = paste_clip(&ops, &clip, Some(4));
        assert_eq!(outcome.reason, "focus-changed");
        assert!(ops.pasted_content.lock().unwrap().is_none());

        *ops.focus.lock().unwrap() = 0;
        let outcome = paste_clip(&ops, &clip, None);
        assert_eq!(outcome.reason, "focus-changed", "no foreground window means nowhere to paste");
    }
}
