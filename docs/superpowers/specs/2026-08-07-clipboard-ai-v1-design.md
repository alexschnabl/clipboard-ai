# Clipboard AI v1 — Tray App (Rust / Tauri)

**Date:** 2026-08-07
**Status:** Implemented and shipped 2026-08-07 (see `src-tauri/`; verification checklist in
`docs/superpowers/notes/2026-08-07-v1-verification.md`). Superseded by v2 (see
`2026-08-07-clipboard-ai-v2-settings-design.md`).
**Supersedes:** nothing. Builds on the findings in `docs/superpowers/notes/2026-08-06-spike-results.md`.

## Context

The Python spike proved the interaction model works: select text anywhere, press a hotkey, an AI
transforms it, the result replaces the selection. It also produced hard numbers and a set of safety
rules that only emerged under review.

v1 turns that into a real application: a tray-resident Tauri app, entirely in Rust, driven by a
config file that defines multiple backends and multiple prompts, each prompt with its own hotkey and
its own backend.

### What the spike established, and which this design inherits

- A resident local model (~400 ms) is the only configuration that feels instant. Every hosted path
  lands at 2–4 s. Cold Ollama is 16 s, so keep-warm matters more than model choice.
- `claude -p` prepends preamble text (`The corrected text:`) despite a system prompt forbidding it.
  Any design that pastes raw model output into a document has to solve this.
- The Claude Agent SDK authenticates off an existing Claude Code login, and a persistent session is
  ~35–40% faster than stateless calls — but it exists only for Python and TypeScript.
- Seven safety rules, each earned by a defect found in review. They are restated below because they
  are the most valuable thing carried forward.

## Goal

A usable daily-driver tray app with no UI beyond the tray menu. Success is: the user edits one JSON
file, presses a hotkey in Outlook, and gets correct text back without ever losing clipboard contents
or pasting something wrong.

## Non-goals for v1

Deferred deliberately, each to its own later cycle:

- **Settings UI.** v2. The user has separate specs for it.
- **Formatting preservation.** Rich text arrives as `CF_HTML`; Word and Outlook emit large, messy
  markup that models mangle. The viable approach is an HTML → Markdown → model → Markdown → HTML
  round trip preserving bold, italic, lists and links while losing fonts and exact spacing. Real
  subsystem, explicitly out of scope here.
- **Review-before-replace popup.** The per-prompt write-back mode chosen early in design. Needs a
  window, so it lands with the UI work.
- **A backend sidecar for the persistent Claude session.** See Architecture.
- **macOS and Linux.** Windows only. The design keeps platform-specific calls behind traits so the
  port is additive, but no non-Windows code is written or tested.

## Architecture

One process, one language. Rust does everything.

```
┌─ Tauri app (Rust) ──────────────────────────────────┐
│  tray icon + menu                                    │
│  config store — JSON, watched, hot-reloaded          │
│  hotkey registry — one global shortcut per prompt    │
│         │                                            │
│         ▼                                            │
│  cycle engine — capture, transform, paste, restore   │
│         │                                            │
│         ▼                                            │
│  Backend trait                                       │
│    ├── OpenAiCompatible  (Ollama, LM Studio, …)      │
│    ├── AnthropicApi      (API key)                   │
│    └── ClaudeCli         (claude -p subprocess)      │
└──────────────────────────────────────────────────────┘
```

### Why no sidecar

An earlier draft bundled a compiled Node sidecar to reach the Claude Agent SDK's persistent session.
It was rejected as disproportionate: a ~50 MB bundled runtime, a localhost HTTP server, token auth,
port negotiation, and crash supervision — all to serve one backend, when every other backend is a
plain HTTP call Rust makes natively.

The cost of dropping it is measured and accepted: the subscription path runs at ~3.3–4.1 s via
`claude -p` instead of ~2.1 s via a held session.

**The door stays open by design.** `Backend` is a trait. If session latency ever justifies the
machinery, a sidecar becomes one more implementation plus one more `kind` in the config, with no
rework anywhere else.

### Open experiment (not a commitment)

`claude -p` supports `--input-format stream-json` for multi-turn use over stdin. If a Rust-held
process can serve successive requests that way, it may recover most of the session speed with no
sidecar. Unverified. It is scoped as a timeboxed experiment during implementation, and if it does
not work in under an hour it is abandoned and `claude -p` stays one-shot.

## Configuration

Single JSON file in Tauri's app-config directory. Hand-edited in v1. Watched for changes; a save
re-registers hotkeys without restarting the app.

```json
{
  "version": 1,
  "backends": [
    {
      "id": "ollama-local",
      "name": "Local Ollama",
      "kind": "openai-compatible",
      "baseUrl": "http://localhost:11434/v1",
      "model": "qwen2.5:32b-instruct",
      "apiKey": null,
      "timeoutMs": 120000
    },
    {
      "id": "claude-sub",
      "name": "Claude (subscription)",
      "kind": "claude-cli",
      "timeoutMs": 120000
    }
  ],
  "prompts": [
    {
      "id": "fix-grammar",
      "name": "Fix grammar",
      "backendId": "ollama-local",
      "hotkey": "Ctrl+Alt+J",
      "systemPrompt": "Fix grammar and spelling."
    }
  ]
}
```

`kind` is one of `openai-compatible`, `anthropic-api`, `claude-cli`. `apiKey` is optional and unused
by `claude-cli`.

On first run, if no config exists, the app writes this file pre-populated with the two backends above
and the one prompt, then loads it. The user is never met with an empty file.

### Validation

Config is validated on load and on every reload. Invalid config never replaces a working config in
memory — the app keeps running on the last good version and surfaces the error in the tray. Rejected:
a `backendId` that names no backend, a duplicate hotkey, an unparseable hotkey string, an unknown
`kind`, a duplicate `id`.

## The sentinel output contract

Every backend's system prompt is wrapped by the app:

```
<the prompt's systemPrompt>

Return only the result, wrapped in <output></output> tags. Emit nothing after the closing tag.
```

The app extracts the content between the first `<output>` and the matching `</output>`. Anything
outside the tags — preamble, commentary, reasoning — is discarded. **If the tags are absent or
unbalanced, the request is treated as a refusal and nothing is pasted.**

This replaces heuristic preamble-stripping, which is fragile and backend-specific. It is uniform
across backends, it rescues `claude -p`, and it fails in the same safe direction as everything else
here. Reasoning markup is handled by the same mechanism: `<think>` blocks live outside the sentinel
and are discarded with the rest.

## The cycle engine

One hotkey press, end to end. Every OS operation is injected so the whole engine is testable without
a GUI — the structure that made the spike's safety rules provable.

1. Hotkey fires. Resolve which prompt it maps to.
2. If a cycle is already running, ignore the press and notify. No concurrent cycles.
3. Save the current clipboard.
4. Wait for the hotkey's modifier keys to be physically released.
5. Clear the clipboard, send synthetic `Ctrl+C`.
6. Poll until the clipboard holds non-whitespace content, or 1000 ms elapses.
7. Record the foreground window.
8. Send text plus the wrapped system prompt to the prompt's backend.
9. Extract the sentinel content.
10. Re-check that the foreground window is unchanged and no modifiers are held.
11. Write the result to the clipboard, send synthetic `Ctrl+V`.
12. Wait 300 ms, restore the original clipboard.

### The safety rules, restated

Non-negotiable. Each exists because a review found the corresponding defect in the spike:

- **Never paste on abort.** Capture timeout, backend refusal, empty result, missing sentinel, stuck
  modifiers, or changed focus — all restore the clipboard and paste nothing.
- **Always restore the clipboard**, on every path including an unexpected error. This is a
  guaranteed-on-unwind concern, not a happy-path step.
- **Wait for modifier release before `Ctrl+C`**, and re-check before `Ctrl+V`. The user may be
  holding a key again after a multi-second model call.
- **Clear, then poll.** Never read the clipboard immediately after `Ctrl+C`; the target application
  writes on its own schedule.
- **Abort if focus changed** between capture and paste.
- **Never paste unextracted output.** No sentinel means refuse.
- **Run the cycle off the hotkey callback thread**, with a non-blocking re-entrancy guard.

### Abort reasons

Surfaced in a tray notification and the log: `capture-timeout`, `modifiers-stuck`,
`modifiers-stuck-before-paste`, `focus-changed`, `backend-error`, `no-sentinel`, `empty-result`,
`paste-failed`.

`paste-failed` was added during implementation. A synthetic-input failure was originally reported as
`backend-error`, which would have sent the user to debug Ollama when the real problem was `enigo`.

## Backends

```rust
trait Backend {
    fn name(&self) -> &str;
    fn transform(&self, system_prompt: &str, text: &str) -> Result<String, BackendError>;
}
```

Synchronous at this boundary; the cycle already runs on its own thread. Every failure mode — network
error, non-2xx, timeout, missing executable, malformed response, empty output — becomes a
`BackendError`. Nothing else escapes, because the cycle's abort handling keys on that one type.

- **OpenAiCompatible** — `POST {baseUrl}/chat/completions`, non-streaming. Covers Ollama, LM Studio,
  OpenRouter, Groq, OpenAI itself. `apiKey` sent as a bearer token when present.
- **AnthropicApi** — `POST /v1/messages` with `x-api-key`. Model from config.
- **ClaudeCli** — `claude -p` subprocess with `--system-prompt`, text on stdin, bounded by
  `timeoutMs`. Uses the existing Claude Code login; no key.

## Tray

The only UI in v1.

- Icon with three states: idle, working, error.
- Menu: the app name and version; each configured prompt with its hotkey (informational); **Open
  config file** (opens the JSON in the default editor); **Reload config**; **Show log**; **Quit**.
- Closing the app window — there is none in v1 — is moot, but the app runs with no window and never
  appears in the taskbar. Quit is only from the tray.
- Last abort or error is shown in the menu so a failure is visible rather than silent.

## Errors and logging

Failures are visible, never silent. Every cycle appends a line to a rolling log file with the prompt
name, backend, outcome, and per-stage timings. Aborts and backend errors also raise a brief tray
notification. A backend that fails does not disable its prompt — the next press retries.

## Testing

The pattern that worked for the spike, carried forward:

- **Pure logic, directly unit-tested:** config parse and validation, hotkey string parsing, sentinel
  extraction (including absent, unbalanced, nested, and multiple-block cases), backend request
  construction and response parsing with an injected HTTP client.
- **Cycle engine, tested with injected fakes** for every OS operation. Every abort path asserts both
  that nothing was pasted and that the clipboard was restored. The happy path asserts the exact call
  order — the spike proved that deleting the clipboard clear or the restore delay otherwise passes a
  full green suite.
- **Not unit-tested, verified by hand:** tray behavior, real global-shortcut registration, and
  synthetic input reaching another application. Same boundary as the spike, and for the same reason.

## Risks

- **Tauri's global-shortcut and clipboard behavior is unproven here.** The spike validated the
  approach via `pynput` and `pyperclip`, not this stack. Different libraries, different timing, and
  this is a timing-sensitive design. First implementation task is a spike-equivalent smoke test in
  Rust before anything is built on top.
- **Synthetic input crate choice.** `enigo` is the likely candidate but its Windows behavior under a
  held-modifier scenario needs checking early.
- **Hotkey conflicts with other applications.** Registration can fail if another app owns the
  combination. Must be detected at registration and surfaced, not discovered when a press does
  nothing.
