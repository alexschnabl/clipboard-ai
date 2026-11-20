use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::config::{Language, Settings, Theme};
use crate::cycle::{CycleOutcome, REASON_CANCELLED};
use crate::overlay::{self, Anchor};

pub const WINDOW_LABEL: &str = "hud";
pub const VIEW_EVENT: &str = "hud://view";
pub const LEAVE_EVENT: &str = "hud://leave";
pub const REASON_NO_LAST_RESULT: &str = "no-last-result";
pub const REASON_PALETTE_FOCUS_FAILED: &str = "palette-focus-failed";

const GAP_LOGICAL: f64 = 8.0;
const FADE_MS: u64 = 220;
const LINGER_DONE_MS: u64 = 1_800;
const LINGER_CANCELLED_MS: u64 = 1_400;
const LINGER_STOPPED_MS: u64 = 4_200;
const MAX_DIFF_CELLS: usize = 4_000_000;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum HudView {
    Working { command: String, backend: String, cancellable: bool },
    Done { words_changed: Option<usize>, sent: bool, again: bool },
    Stopped { reason: String },
}

impl HudView {
    pub fn linger_ms(&self) -> u64 {
        match self {
            HudView::Working { .. } => 0,
            HudView::Done { .. } => LINGER_DONE_MS,
            HudView::Stopped { reason } if reason == REASON_CANCELLED => LINGER_CANCELLED_MS,
            HudView::Stopped { .. } => LINGER_STOPPED_MS,
        }
    }

    pub fn from_outcome(outcome: &CycleOutcome) -> Self {
        if outcome.pasted && outcome.reason == "sent" {
            return HudView::Done { words_changed: None, sent: true, again: false };
        }
        if outcome.pasted {
            let words_changed = match (&outcome.captured_text, &outcome.result_text) {
                (Some(before), Some(after)) => changed_words(before, after),
                _ => None,
            };
            return HudView::Done { words_changed, sent: false, again: false };
        }
        HudView::Stopped { reason: outcome.reason.to_string() }
    }
}

pub fn changed_words(before: &str, after: &str) -> Option<usize> {
    let a: Vec<&str> = before.split_whitespace().collect();
    let b: Vec<&str> = after.split_whitespace().collect();
    if a.len().saturating_mul(b.len()) > MAX_DIFF_CELLS {
        return None;
    }
    let mut row = vec![0usize; b.len() + 1];
    for word in &a {
        let mut diagonal = 0;
        for (j, other) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if word == other { diagonal + 1 } else { above.max(row[j]) };
            diagonal = above;
        }
    }
    let common = row[b.len()];
    Some((a.len() - common).max(b.len() - common))
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    pub theme: Theme,
    pub language: Language,
    #[serde(skip)]
    pub enabled: bool,
}

impl Look {
    pub fn from_settings(settings: &Settings) -> Self {
        Self { theme: settings.theme, language: settings.language, enabled: settings.hud }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewEvent {
    id: u64,
    fresh: bool,
    #[serde(flatten)]
    look: Look,
    view: HudView,
}

struct Inner {
    app: AppHandle,
    generation: AtomicU64,
    anchor: Mutex<Option<Anchor>>,
}

#[derive(Clone)]
pub struct Hud {
    inner: Arc<Inner>,
}

impl Hud {
    pub fn new(app: AppHandle) -> Self {
        Self {
            inner: Arc::new(Inner {
                app,
                generation: AtomicU64::new(0),
                anchor: Mutex::new(None),
            }),
        }
    }

    fn raw_window(&self) -> Option<isize> {
        let window = self.inner.app.get_webview_window(WINDOW_LABEL)?;
        window.hwnd().ok().map(|h| h.0 as isize)
    }

    fn emit(&self, view: HudView, look: Look, fresh: bool) -> u64 {
        let id = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.inner.app.emit_to(WINDOW_LABEL, VIEW_EVENT, ViewEvent { id, fresh, look, view });
        id
    }

    pub fn begin(&self, anchor: Anchor, view: HudView, look: Look) -> Option<u64> {
        if !look.enabled {
            return None;
        }
        *crate::recover_lock(self.inner.anchor.lock(), "hud anchor", None) = Some(anchor);
        Some(self.emit(view, look, true))
    }

    pub fn finish(&self, view: HudView, look: Look) {
        if !look.enabled {
            return;
        }
        let has_anchor = crate::recover_lock(self.inner.anchor.lock(), "hud anchor", None).is_some();
        if !has_anchor {
            return;
        }
        let linger = view.linger_ms();
        let id = self.emit(view, look, false);
        self.schedule_leave(id, linger);
    }

    pub fn notice(&self, anchor: Anchor, view: HudView, look: Look) {
        if !look.enabled {
            return;
        }
        let linger = view.linger_ms();
        if let Some(id) = self.begin(anchor, view, look) {
            self.schedule_leave(id, linger);
        }
    }

    fn schedule_leave(&self, id: u64, linger_ms: u64) {
        let hud = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(linger_ms));
            if hud.inner.generation.load(Ordering::SeqCst) != id {
                return;
            }
            let _ = hud.inner.app.emit_to(WINDOW_LABEL, LEAVE_EVENT, id);
            std::thread::sleep(Duration::from_millis(FADE_MS));
            if hud.inner.generation.load(Ordering::SeqCst) != id {
                return;
            }
            let app = hud.inner.app.clone();
            let _ = app.run_on_main_thread(move || {
                if hud.inner.generation.load(Ordering::SeqCst) == id {
                    if let Some(raw) = hud.raw_window() {
                        overlay::hide_without_activating(raw);
                    }
                }
            });
        });
    }

    pub fn dismiss(&self) {
        let id = self.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let hud = self.clone();
        let _ = self.inner.app.run_on_main_thread(move || {
            if hud.inner.generation.load(Ordering::SeqCst) == id {
                if let Some(raw) = hud.raw_window() {
                    overlay::hide_without_activating(raw);
                }
            }
        });
    }

    pub fn measured(&self, id: u64, width: f64, height: f64, inset: f64) {
        if self.inner.generation.load(Ordering::SeqCst) != id {
            return;
        }
        let Some(anchor) = *crate::recover_lock(self.inner.anchor.lock(), "hud anchor", None) else {
            return;
        };
        let Some(raw) = self.raw_window() else { return };
        let screen = overlay::screen_for(anchor.rect);
        let s = screen.scale;
        let inset_px = overlay::to_physical(inset, s);
        let content = (
            overlay::to_physical(width, s) - 2 * inset_px,
            overlay::to_physical(height, s) - 2 * inset_px,
        );
        let placement = overlay::place(
            anchor.rect,
            screen.work,
            content,
            inset_px,
            overlay::to_physical(GAP_LOGICAL, s),
            None,
        );
        overlay::show_without_activating(raw, placement);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(pasted: bool, reason: &'static str) -> CycleOutcome {
        CycleOutcome {
            pasted,
            reason,
            detail: None,
            captured_ms: 0,
            backend_ms: 0,
            captured_text: Some("Their going to ship it on friday.".into()),
            result_text: Some("They're going to ship it on Friday.".into()),
            result_html: None,
            backend_name: None,
            app: None,
            rule: None,
        }
    }

    #[test]
    fn identical_text_changes_no_words() {
        assert_eq!(changed_words("same words here", "same  words\nhere"), Some(0));
    }

    #[test]
    fn substitutions_count_once_each() {
        assert_eq!(changed_words("Their going to ship it on friday.", "They're going to ship it on Friday."), Some(2));
    }

    #[test]
    fn insertions_and_deletions_count_the_larger_side() {
        assert_eq!(changed_words("send the the report", "send the report"), Some(1));
        assert_eq!(changed_words("a b", "a x y b"), Some(2));
        assert_eq!(changed_words("", "brand new text"), Some(3));
    }

    #[test]
    fn huge_inputs_skip_the_count_instead_of_stalling_the_hud() {
        let big = "word ".repeat(3_000);
        assert_eq!(changed_words(&big, &big), None);
    }

    #[test]
    fn a_pasted_outcome_becomes_done_with_the_word_count() {
        assert_eq!(
            HudView::from_outcome(&outcome(true, "ok")),
            HudView::Done { words_changed: Some(2), sent: false, again: false }
        );
    }

    #[test]
    fn a_send_only_outcome_says_sent_and_offers_no_undo() {
        assert_eq!(
            HudView::from_outcome(&outcome(true, "sent")),
            HudView::Done { words_changed: None, sent: true, again: false }
        );
    }

    #[test]
    fn an_aborted_outcome_carries_the_same_reason_code_history_records() {
        let view = HudView::from_outcome(&outcome(false, "focus-changed"));
        assert_eq!(view, HudView::Stopped { reason: "focus-changed".into() });
    }

    #[test]
    fn a_cancelled_run_fades_sooner_than_a_failure_and_working_never_fades() {
        let cancelled = HudView::Stopped { reason: REASON_CANCELLED.into() };
        let failed = HudView::Stopped { reason: "backend-error".into() };
        let working = HudView::Working { command: "x".into(), backend: "y".into(), cancellable: true };
        assert!(cancelled.linger_ms() < failed.linger_ms());
        assert_eq!(working.linger_ms(), 0);
    }

    #[test]
    fn the_view_serializes_with_a_kind_tag_and_camel_case_fields() {
        let json = serde_json::to_value(HudView::Done { words_changed: Some(3), sent: false, again: true }).unwrap();
        assert_eq!(json, serde_json::json!({"kind": "done", "wordsChanged": 3, "sent": false, "again": true}));
        let json = serde_json::to_value(HudView::Working { command: "Fix".into(), backend: "API".into(), cancellable: true }).unwrap();
        assert_eq!(json["kind"], "working");
        assert_eq!(json["cancellable"], true);
    }

    #[test]
    fn the_event_carries_theme_and_language_but_not_the_enabled_switch() {
        let event = ViewEvent {
            id: 4,
            fresh: true,
            look: Look { theme: Theme::Dark, language: Language::De, enabled: true },
            view: HudView::Stopped { reason: "cancelled".into() },
        };
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["theme"], "dark");
        assert_eq!(json["language"], "de");
        assert!(json.get("enabled").is_none());
        assert_eq!(json["view"]["reason"], "cancelled");
    }
}
