# Clipboard AI — Capture/Paste Spike

**Date:** 2026-08-06
**Status:** Approved, not yet implemented
**Scope:** Sub-project 1 of Clipboard AI. Throwaway code.

## Context

Clipboard AI is a system-wide text assistant: select text in any application, press a
hotkey, have an AI transform it with a preconfigured prompt, and get the result written
back over the selection. Target platforms are Windows, macOS, and Linux. Backends must
cover any common API (Anthropic, OpenAI-compatible, Ollama) plus Claude subscription
usage.

The interesting risk in this project is not the model call, the config format, or the UI.
It is whether *capture selection → transform → paste back* works reliably in the
applications the user actually works in. Every application handles clipboard timing and
focus differently, and the failure modes are silent: stale text, garbled input, or output
landing in the wrong window.

This spike answers that question before any production stack is chosen.

## Goal

Answer three questions, then delete the code.

1. **Does the round trip work in the applications that matter?** Which ones work, which
   fail, and why.
2. **What is the real end-to-end latency, and how much of it is the model?** A per-stage
   timing breakdown, so we know the timing budget the real app has to fit inside.
3. **Does the Claude Agent SDK authenticate off the existing Claude Code login, and how
   much faster is a warm session than a cold `claude -p`?** This decides whether the
   subscription backend is usable for an interactive hotkey tool.

## Non-goals

Explicitly out of scope. These belong to the production design, which happens after this
spike reports back:

- Multiple prompts, prompt selection UI, or more than one hotkey
- The review-before-replace popup
- Configuration files, settings, or persistence
- Provider adapter abstraction (Anthropic / OpenAI-compatible / Ollama)
- macOS and Linux
- Packaging, installers, tray icon, autostart
- Error recovery beyond "abort and do nothing"

## Design

### Shape

A single Python file. Windows only. No config, no UI, no persistence. One hardcoded
hotkey, one hardcoded prompt. Timing data printed to the console.

### Flow

1. Hotkey `Ctrl+Alt+J` fires.
2. Wait for all hotkey modifier keys to be physically released.
3. Save the current clipboard contents.
4. Clear the clipboard.
5. Send synthetic `Ctrl+C`.
6. Poll the clipboard until it becomes non-empty, or until a 1000 ms timeout.
   - **On timeout: abort silently.** Restore the saved clipboard and do nothing else.
     Never paste. A timeout means no text was selected, and pasting stale clipboard
     contents over someone's document is the worst failure this tool can have.
7. Send the captured text to the backend with the prompt.
8. Put the result on the clipboard.
9. Send synthetic `Ctrl+V`.
10. Wait 300 ms, then restore the originally saved clipboard.

### Hardcoded values

- Hotkey: `Ctrl+Alt+J`
- Prompt (system): `Fix grammar and spelling. Return only the corrected text, with no
  preamble, explanation, or commentary.`
- Clipboard capture timeout: 1000 ms, polled every 20 ms
- Clipboard restore delay after paste: 300 ms

### The three hazards this spike exists to measure

These are the reasons the spike is worth doing rather than building straight into the
real app.

**Modifier bleed.** When the hotkey callback runs, `Ctrl` and `Alt` are still physically
held down by the user. Synthesising `Ctrl+C` on top of a held `Alt` does not produce a
copy — it produces whatever `Ctrl+Alt+C` means in the focused app. Step 2 exists to
handle this, and if it turns out not to be solvable reliably, that finding invalidates
the whole hotkey approach and is worth knowing on day one.

**Clipboard race.** After `Ctrl+C` is delivered, the target application writes to the
clipboard on its own schedule. Reading immediately returns stale data. Step 4's clear +
step 6's poll detect the write rather than guessing at it. The measured per-application
latency here is the primary output of the spike: it sets the timing budget for the real
implementation.

**Focus loss.** If focus moves between capture and paste, the result lands in the wrong
window. Worth establishing now whether this happens in normal use or only under
contrived conditions.

### Libraries

- `pynput` — global hotkey registration and synthetic keystrokes. Chosen over the
  `keyboard` package because it does not generally require elevation, and because it is
  cross-platform, so what is learned here carries into the macOS and Linux work later.
- `pyperclip` — clipboard read/write.
- `anthropic` — the Messages API comparison path.
- `claude-agent-sdk` — the subscription path.

### Backends

Two backends, run side by side so their latency can be compared directly.

**A. Anthropic Messages API** (`anthropic` package, API key via `ANTHROPIC_API_KEY`):

- Model `claude-opus-5`
- `thinking: {"type": "adaptive"}` with `output_config: {"effort": "low"}`
- `max_tokens: 16000`, non-streaming

Two notes on those choices. Thinking is left on at low effort rather than disabled:
on Claude Opus 5, disabled thinking can leak `<thinking>` tags into the visible
response, and this text goes straight into the user's document, so leakage is
unacceptable here. Low effort is the recommended lever for cutting latency instead.
Streaming is not used because the result is pasted in one shot, so streaming would not
improve perceived latency. Model choice is itself a latency variable; sweeping cheaper
models is a knob for the production app and a decision for the user, not something to
quietly assume here.

**B. Claude Agent SDK** (`claude-agent-sdk`, using the existing Claude Code login):

Write this against the official Agent SDK documentation at
`code.claude.com/docs/en/agent-sdk` at implementation time. Do not infer its API surface
from the Messages API — they are different libraries with different shapes. Measure
three numbers: cold start, warm session round trip, and for comparison, a cold
`claude -p` subprocess invocation.

### Instrumentation

Record and print a timestamp at each stage:

| Stage | Marker |
|---|---|
| t0 | hotkey fired |
| t1 | modifiers released |
| t2 | `Ctrl+C` sent |
| t3 | clipboard change detected — **the per-app capture latency** |
| t4 | backend response received — **the model latency** |
| t5 | `Ctrl+V` sent |
| t6 | original clipboard restored |

## Test matrix

Ten consecutive runs in each, recording successes, failures, and the failure mode:

- Notepad (control — if this fails, the mechanism is broken, not the app)
- Outlook
- Teams
- Browser: a plain `<textarea>`
- Browser: a rich `contenteditable` (e.g. Gmail compose)
- VS Code
- Word

## Success criteria

- Capture and paste succeed in at least 4 of the 7 targets, 10 consecutive runs each,
  with no manual retry.
- Median capture latency (t3 − t2) recorded per application.
- Agent SDK warm-session latency measured and compared against cold `claude -p` and
  against the Messages API.
- The original clipboard is correctly restored in every run, including aborted ones.
- No run pastes stale or incorrect text. This one is not negotiable — a single
  occurrence is a failure of the whole approach, not a bug to file.

## Deliverables

1. The spike script.
2. A short results note: which applications worked, the timing table, the backend
   latency comparison, and a recommendation on whether in-place replacement is viable
   or whether the production app needs a different interaction model (for example, a
   floating result window rather than pasting over the selection).

A negative result here is a successful spike. Learning that in-place replacement is
unreliable in Outlook is worth far more before the production app is built than after.
