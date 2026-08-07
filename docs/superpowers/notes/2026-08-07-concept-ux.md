# Feedback UX during a cycle — design concepts

**Date:** 2026-08-07 · **Status:** Concept exploration, no code
**Grounding:** v1 ships tray-icon-state + tray-menu-status only. Timings and abort reasons are
measured (spike results + v1 design spec).

**Direct answer first:** a fast cycle (~400 ms) should show **nothing**. Below the ~250 ms
threshold where people notice a gap at all, any indicator arrives after the paste is done — that's
not feedback, it's a flash. Every concept below targets the other rows — 2.1 s, 3.3–4.1 s, 16 s —
plus making silence never mean "the hotkey is dead."

---

## Concept 1 — Delayed caret badge (escalating, anchored to text)

A small badge at the caret/selection, not the tray, appears only if the cycle outlives a delay.

- **0–250 ms:** nothing.
- **250 ms:** a 16×16 dot fades in (100 ms) at the end of the original selection.
- **2,500 ms:** grows to 20×20, neutral spin → amber ring (above the 2.1 s Claude-session band, so
  that path still reads "normal-slow").
- **8,000 ms:** becomes a pill: `still working… (Esc to cancel)` — reached only by cold Ollama or a
  hung backend.
- **Completion:** fades out over 150 ms, timed to coincide with the paste, so "indicator gone" and
  "text changed" read as one event.
- **Abort:** a red-outlined glyph for ~1.5 s next to the untouched selection, then gone.

```
t=0      t=250ms    t=2500ms         t=8000ms
[sel]    [sel]•     [sel](amber ring) [sel][still working… Esc]
no UI    dot in      escalates         escalates again
```

**400 ms:** invisible. **16 s:** the only concept with a visible three-stage escalation, so a long
wait never reads as an unexplained slow version of a short one, and offers a cancel path.
**Failures:** one abort glyph plus reason text in the tray/log; a glyph per reason is possible but
adds an icon set. Cancel requires the cycle engine to support cancellation, which it does not today.

**Costs/risks:** needs an always-on-top, click-through overlay positioned at caret coordinates —
the hardest ask here (DPI, multi-monitor, and caret position isn't uniformly exposed — Word,
`contenteditable`, and Notepad already differ on simpler things per the spike's matrix). No caret
position, no fallback placement. Most exposed to "stuck on screen": a torn-down-late overlay after
a crash or missed event leaves a ghost dot over the document — what the fail-safe philosophy
forbids.

---

## Concept 2 — Tray-only, but louder and reason-coded

All feedback stays in the tray/OS-notification layer — no overlay, ever — but the existing
idle/working/error icon gets richer, and every non-happy outcome pairs with a native toast.

- **Press:** icon → "working" immediately (already true in v1); cheap, off-document, no fast-path
  cost.
- **Fast finish (≤~1 s):** icon reverts, no toast — a toast here reintroduces the flash problem.
- **Slow finish (>4 s, above the Claude-session band):** icon stays "working," one toast fires:
  `Fix grammar — still processing (Claude, subscription)`. Never a stream.
- **Success:** icon reverts, no toast — the pasted text *is* the confirmation.
- **Abort/failure:** icon flips to "error" and **stays** until acknowledged (menu opened, or the
  next success) — the fix for "silence = dead hotkey." An immediate toast names the reason in plain
  language: *Nothing was selected* / *Focus changed before the result arrived* / *A key was still
  held down* / *Ollama didn't respond* / *Still busy with the last request*.

**400 ms:** icon flicker only. **16 s:** one "still working" toast, then an accurate icon — weaker
than Concept 1 at ongoing reassurance, its real gap on the 16 s case. **Failures:** best-in-class —
one toast per reason, plain language, plus a persistent error icon, directly answering the
dead-hotkey tension. A busy-ignored press also now gets a toast instead of vanishing.

**Costs/risks:** the user's eyes are on the document, not the tray — real, but bounded, since v1
users already glance at the tray for last-outcome. Never touches the document, so it can't
misposition, obscure content, or land on the wrong monitor — the most robust concept to its own
failure. Miscalibrated thresholds risk toast fatigue; capped at one per cycle.

---

## Concept 3 — Adaptive, backend-history-aware threshold (a policy layer, not standalone)

Neither concept above uses the fact that the app knows which backend is about to run. Make the
show-delay a function of that backend's own rolling median, not a fixed constant.

- Keep a rolling median per backend id (last ~5 cycles).
- Median < 700 ms (resident Ollama): stay silent until 600 ms — assume fast, reveal only if this
  call is an outlier.
- Median 2–4.5 s (Claude session / `claude -p`): show presence immediately — the user already
  knows this prompt is "the slow one."
- Current call blows past 3× its own backend's median (a normally-warm Ollama that just unloaded):
  escalate with a specific toast — `Local model is reloading, ~15s` — the moment expectation is
  most violated.
- No history yet: fall back to a fixed default. Reuses Concept 1's or 2's presentation; this
  concept only changes *when* signals fire.

**400 ms:** silent for the usually-fast case, and calls out the *surprising* slow case — the most
confusing moment today (same keypress, wildly different wait, no explanation). **16 s:** best
distinguishes "routine cold start" from "surprise cold start," which flat-delay concepts can't tell
from a hang. **Failures:** inherits the base concept.

**Costs/risks:** a persisted rolling-latency store, window-size/outlier decisions, a cold-start case
with no data. Not shippable alone — it's "Concept 1 or 2, plus adaptive timing." Log the threshold
decision alongside the existing cycle log, or "why did it show this time" becomes undebuggable.

---

## Concept 4 — Selection pulse (no chrome at all)

No overlay, no toast, no tray change mid-cycle. Instead, reuse the synthetic-input path the cycle
already owns to send a transient re-selection pulse (deselect/reselect the same range) at 500 ms
and again at 3 s if still running — a low-fidelity "heartbeat" via the document's own highlight
color, at the exact spot the user is looking.

**400 ms:** no pulse. **16 s:** several weak flickers. **Failures:** cannot use this channel at
all — there's no selection to flicker after a focus-changed abort, and pulsing right before *not*
pasting reads as a glitch. Must delegate all failure signaling to the tray.

**Costs/risks:** the riskiest idea here. It adds a purely cosmetic synthetic-input operation to a
codebase whose safety record comes from *minimizing* synthetic input during unpredictable states
(the spike's review caught four defects at exactly this kind of boundary), and can race the
modifier/focus checks the cycle depends on. Many apps render selection-color changes with lag or
not at all — including Word and Outlook, the named priority targets.

---

## Comparison

| | 1: Caret badge | 2: Loud tray | 3: Adaptive timing | 4: Selection pulse |
|---|---|---|---|---|
| Fast-path (400 ms) UI | none, 250 ms gate | icon flicker | none, backend-aware gate | none, 500 ms gate |
| 16 s handling | 3-stage escalation + cancel | one toast + accurate icon | per-backend calibrated | repeated weak flicker |
| Eye location | on the text | tray corner | inherits base | on the text |
| Distinguishes abort reasons | yes | yes, best-in-class | inherits base | no — tray only |
| Fixes "hotkey looks dead" | partial | yes, strongest | inherits base | no |
| New OS surface | overlay window | toast + tray | none (policy) | none (reuses input) |
| Stuck-on-screen risk | real | low | inherits base | low, wrong-state risk instead |
| Fights safety rules | no | no | no | yes |
| Build cost | highest | low–medium | medium | medium, adversarial |

---

## Ranked recommendation

**1. Concept 2 — build first, keep as the permanent baseline.** Needs nothing beyond what's already
assumed (tray icon, native toast), never touches the document, and is the only concept that
squarely closes tension #4: a persistent error icon plus a reason-coded toast means a failed or
ignored press is never silent again.

**2. Concept 3, layered on Concept 2, once the timing log has real data.** Not shippable alone, but
it resolves the sharpest form of the stated problem — the same hotkey taking 400 ms yesterday and
16 s now — cheaply, once per-backend history exists.

**3. Concept 1, conditionally.** The most honest treatment of the 400 ms-vs-16 s tension and best
match to "eyes are on the document," but it hinges on reliably positioning an overlay at caret
coordinates across arbitrary apps — an open feasibility question, not an assumption. Build only
after that comes back positive, and ship as an *addition* to Concept 2's toasts, never a
replacement, so a silent overlay failure still has a fallback net.

**Would not build: Concept 4.** It asks a codebase whose safety record comes from minimizing and
carefully ordering synthetic input to add a cosmetic synthetic-input operation that races the same
checks four review passes already patched — for a weaker payoff than Concept 2 at near-zero risk.
