# Clipboard AI

A Windows tray app that transforms selected text in any application using an AI backend,
in place. Press a configurable global hotkey, the current selection is copied, sent to a
configured backend with a per-command system prompt, and the result is pasted back over
the selection. No window steals focus in the normal case — the whole interaction happens
without leaving the app you're typing in.

## What it does

1. You select text in any application and press a hotkey (e.g. `Ctrl+Alt+J` for a "Fix
   grammar" command).
2. The app copies the selection, sends it to the command's configured AI backend with
   the command's system prompt.
3. The model's reply is pasted back over the original selection.

Multiple **commands** (name + hotkey + system prompt + backend) can be configured, each
bound to its own hotkey, each pointing at one of possibly several configured
**backends**. The app is tray-resident: no window is visible until you open its settings
from the tray icon. Windows only today — see `docs/superpowers/` for what a
cross-platform port would need.

## Running it

Commands are from `package.json`.

- **Dev:** `npm run tauri dev` — builds and launches the whole app (Rust backend, tray
  icon, live hotkeys, and the hidden SvelteKit settings window). Running `npm run dev`
  alone only starts the Vite dev server for the settings UI in a browser; it has no tray,
  no hotkeys, and no Rust backend behind it.
- **Build:** `npm run tauri build` — produces a release bundle (runs the frontend build,
  then `cargo build`, per `beforeBuildCommand` in `src-tauri/tauri.conf.json`).
- **Check:** `npm run check` — `svelte-kit sync` + `svelte-check` across the frontend.

**Config file:** `%APPDATA%\com.clipboardai.app\config.json` — created with defaults on
first run. Hand-editing it is a supported path; the same validation that guards the
settings UI also runs on load. Editable live from the tray ("Settings"), or by hand
("Open config file" in the tray menu applies changes without restarting the app).

**Log file:** `%APPDATA%\com.clipboardai.app\clipboard-ai.log` — one line per hotkey
press (`PASTED`/`ABORTED`, reason, timings), truncated once it exceeds ~1 MB. "Show log"
in the tray menu opens it.

## Architecture

### Rust (`src-tauri/src/`)

- `lib.rs` — Tauri app wiring: builds `AppState`, registers the global-shortcut plugin
  and its hotkey-dispatch closure, constructs the tray icon/menu, and owns the settings
  window's show/hide lifecycle.
- `config.rs` — the config schema (currently v2), parsing, validation, v1→v2 migration,
  and atomic load/save.
- `commands.rs` — the Tauri IPC commands the settings window calls: `get_config`,
  `save_config`, `suspend_hotkeys`, `resume_hotkeys`.
- `cycle.rs` — the capture → transform → paste engine: one hotkey press, end to end.
- `sentinel.rs` — the `<output>` wrap/extract contract between the app and the backend
  (see Safety model below).
- `osops.rs` — the `OsOps` trait plus its Windows implementation: clipboard read/write,
  synthetic copy/paste, and reads of focus and held-down modifier keys.
- `status.rs` — log-line formatting and log file writes.
- `backend/` — the `Backend` trait plus one adapter per backend kind.

### Frontend (`src/`)

SvelteKit + shadcn-svelte + Tailwind, prerendered to static HTML (`adapter-static`,
`fallback: index.html`) and loaded into the settings window — Tauri has no Node server
at runtime.

- `src/routes/+page.svelte` — the settings screen (Backends and Commands tabs).
- `src/lib/components/` — `BackendList`/`BackendForm`, `CommandList`/`CommandForm`,
  `HotkeyRecorder`: the app's own UI, hand-written.
- `src/lib/state/config.svelte.ts` — the single reactive source of truth for the
  settings window's config state (load, save, dirty tracking).
- `src/lib/components/shadcn/` — generated shadcn-svelte primitives; regenerate via the
  shadcn CLI (`components.json`) rather than hand-editing.

### Backend kinds

Each configured backend has a `kind`, one of:

- `openai-compatible` — any OpenAI-style `/chat/completions` endpoint (Ollama, LM
  Studio, OpenAI itself).
- `anthropic-api` — the Anthropic Messages API directly, using an API key.
- `claude-cli` — shells out to the `claude` CLI, using its own subscription auth (no
  API key needed).

## The safety model

This is the app's defining property, worth understanding before touching `cycle.rs`,
`osops.rs`, or `sentinel.rs`: the app types into other applications' windows and
overwrites the user's real selection. Every stage is built to **abort rather than
paste** whenever anything is uncertain — a failed guard produces nothing on screen,
never a guess.

- **Clipboard-restore guard.** `ClipboardGuard` in `cycle.rs` reads the clipboard once
  at the start of a cycle and restores it unconditionally on every exit — an early
  return or a panic — via `Drop`. There is no "disarm"; restoration is a property of
  scope, not a step any one code path has to remember.
- **Focus and modifier checks.** The foreground window is sampled before the copy and
  re-checked immediately before the paste; if it changed, the cycle aborts
  (`focus-changed`) instead of pasting into whatever now has focus. Held-down modifier
  keys are checked both before capture and again right before paste
  (`modifiers-stuck`/`modifiers-stuck-before-paste`), so a hotkey chord the OS hasn't
  finished releasing can't leak into the paste.
- **The `<output>` sentinel contract.** Every system prompt is wrapped with an
  instruction to return the result inside `<output>…</output>` tags.
  `sentinel::extract` returns `None` — treated as a refusal, never empty text — when the
  tags are missing, or when more than one `</output>` appears (ambiguous which block is
  the real answer). Either case aborts the cycle (`no-sentinel`) instead of pasting
  whatever the model actually said.
- Every abort has its own reason string (`backend-error`, `capture-timeout`,
  `clipboard-write-failed`, `paste-failed`, `no-sentinel`, …), logged, so a failure is
  diagnosable instead of collapsing into one generic "it didn't work."

## Testing

- Frontend: `npm run test` (Vitest, tests under `test/unit/`).
- Rust: `cargo test`, run from `src-tauri/`. Requires the MSVC linker — if `cargo`
  can't find `link.exe`, load `vcvars64.bat` from your Visual Studio installation in the
  *same* shell invocation before running cargo, from PowerShell rather than the
  MSYS/Bash tool (path quoting around the "Visual Studio" space breaks under MSYS). See
  `CLAUDE.md` for the exact command verified on this machine.
- `npm run check` — svelte-check across the frontend.

## Design history

`docs/superpowers/` holds the project's design history, not its current state: a
chronological spike → v1 → v2 trail of specs, plans and verification notes, each
building on and partly superseding the last. Read it to understand *why* something is
built the way it is; read this file and the code for what it *is* today.
