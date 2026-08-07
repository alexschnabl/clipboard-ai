# Clipboard AI v2 — Settings UI

**Date:** 2026-08-07
**Status:** Implemented and shipped 2026-08-07 (see `src-tauri/` and `src/`; verification checklist
in `docs/superpowers/notes/2026-08-07-v2-verification.md`). Current design — nothing supersedes it
as of this writing.
**Builds on:** `2026-08-07-clipboard-ai-v1-design.md`. v1 is verified working — Ollama and the Claude
subscription both confirmed end to end.

## Context

v1 is a tray-resident app with no UI. Backends and commands are configured by hand-editing
`%APPDATA%\com.clipboardai.app\config.json`, and hotkeys are registered once at startup, so any
change needs a restart.

v2 adds a settings window: create, edit and delete backends and commands, set each command's
backend, hotkey and prompt, and have changes take effect immediately.

## Goal

The user never has to open the config file or restart the app. Success is: add a command, record a
hotkey, press it, and it works — without touching anything outside the window.

## Non-goals

- **In-flight feedback during a cycle.** Currently the user gets nothing between pressing a hotkey
  and the text changing, which with a cold local model is sixteen seconds of silence. This is a real
  gap and is being designed separately; it is excluded here so it does not hold up the settings work.
- **Multiple prompts per command.** The schema names the concept `command` so this can be added as a
  field later without a second rename, but v2 keeps exactly one prompt per command.
- **Secure key storage.** The Anthropic API key stays in `config.json`, masked in the UI with a
  reveal toggle. Deliberate: this is a single-user local tool, hand-editing the file remains
  supported, and a keychain dependency buys little here.
- **macOS and Linux.** Windows only, as v1.
- **A review-before-replace mode.** Still deferred.

## Frontend stack

Matches `E:\Code\M42Toolkit` so both projects share conventions:

- SvelteKit with `@sveltejs/adapter-static` and `fallback: 'index.html'` — Tauri has no Node server,
  so the frontend is prerendered.
- Svelte 5 runes. State lives in singleton classes exported from `$lib/state/*.svelte.ts`.
- shadcn-svelte, components under `$lib/components/shadcn`, base colour zinc, style `lyra`,
  icon library remixicon.
- Tailwind v4 via `@tailwindcss/vite`.
- `mode-watcher` for theming, with `<ModeWatcher />` in the root layout. **System theme, no toggle** —
  the app follows the OS.

This replaces the vanilla scaffold currently in the repo.

## Window lifecycle

One window, `settings`, **created hidden**. It is a panel the user summons, not the app's main
surface — the app stays as tray-resident as it is now.

- Shown by a left click on the tray icon, or the tray menu's **Settings** item.
- The close button **hides** the window rather than exiting. Quit remains tray-only.
- `skip_taskbar` is false while visible so it can be alt-tabbed to.
- If the window is already open, showing it focuses the existing one rather than creating a second.

## Configuration — schema version 2

The key `prompts` is renamed to `commands`, and `PromptConfig` becomes `CommandConfig`. The concept
is a command that owns a backend, a hotkey and a prompt; naming it `prompts` would leave the UI
saying "Commands" while the file said something else, and the future shape (a command owning several
prompts) would have forced the rename anyway.

```json
{
  "version": 2,
  "backends": [
    { "id": "ollama-local", "name": "Local Ollama", "kind": "openai-compatible",
      "baseUrl": "http://localhost:11434/v1", "model": "qwen2.5:32b-instruct",
      "apiKey": null, "timeoutMs": 120000 }
  ],
  "commands": [
    { "id": "fix-grammar", "name": "Fix grammar", "backendId": "ollama-local",
      "hotkey": "Ctrl+Alt+J", "systemPrompt": "Fix grammar and spelling." }
  ]
}
```

**Migration.** On load, a file with `"version": 1` is read with `prompts` mapped to `commands`,
rewritten as version 2, and loaded. Migration happens once, silently, and is logged. A file with an
unknown future version is rejected rather than guessed at — the app keeps running with an empty
command set and reports it, per v1's rule that a bad config never kills the app.

## Rust — the substantive work

The screens are forms. The real work is that v1 registers hotkeys once and never again.

### Commands exposed to the frontend

| Command | Purpose |
|---|---|
| `get_config() -> Config` | Current in-memory config |
| `save_config(config: Config) -> SaveResult` | Validate, persist, and apply live |
| `suspend_hotkeys()` | Unregister all shortcuts, for hotkey recording |
| `resume_hotkeys()` | Re-register from current config |

`SaveResult` carries `{ ok: bool, error: Option<String>, problems: Vec<String> }` — `error` for a
validation failure that rejected the whole save, `problems` for per-command issues that did not
(a hotkey the OS refused, a backend that would not build).

### Live apply

`save_config` runs: validate through the existing `config::parse` → write the file → unregister every
shortcut → rebuild the backend map → register the new shortcuts → return problems.

**Validation stays in Rust.** It is already written and tested; duplicating it in TypeScript would
create a second source of truth that drifts.

**A cycle running during a save is safe by construction.** The cycle thread holds an
`Arc<dyn Backend>` cloned before it started, so replacing the map underneath it cannot affect it.
This falls out of v1's design rather than needing new machinery. A save while a cycle is in flight
therefore takes effect from the *next* press.

### Hotkey suspension

Press-to-record needs the app's own shortcuts off, or recording a combination that is already
assigned fires a transform instead of capturing it. `suspend_hotkeys` unregisters everything;
`resume_hotkeys` restores from config. The frontend must call `resume_hotkeys` on every exit path
from recording — commit, cancel, blur, and window close — or the app is left with no working
hotkeys and no visible reason. This is the sharpest failure mode in v2 and gets its own test.

## Screens

Two tabs. There are only two things to edit and a sidebar would be ceremony.

**Backends.** A list with add, edit and delete. Fields switch on `kind`:

| Kind | Fields |
|---|---|
| `openai-compatible` | name, base URL, model, optional API key, timeout |
| `anthropic-api` | name, model (default `claude-opus-5`), API key (masked, reveal toggle), timeout |
| `claude-cli` | name, timeout |

Deleting a backend still referenced by a command is refused, naming the commands that use it.

**Commands.** A list with add, edit and delete. Fields: name, backend (select), hotkey
(press-to-record), prompt (textarea).

Each screen has its own Save, a dirty indicator, and an unsaved-changes confirmation when closing
the window with pending edits.

## Hotkey recording

Click the field, press a combination, it is captured. Mechanically:

1. On focus, call `suspend_hotkeys`.
2. Listen for `keydown`, `preventDefault()`, and ignore events where the key is itself a modifier —
   otherwise the first `Ctrl` press commits `Ctrl` alone.
3. Map the browser event to the string Rust expects: modifiers in a fixed order
   (`Ctrl`, `Alt`, `Shift`, `Super`) joined with `+`, then the key.
4. Require at least one modifier. A bare letter would swallow that key system-wide.
5. `Escape` cancels and restores the previous value. Blur commits what has been captured.
6. Call `resume_hotkeys` on every exit path.

**The browser-to-Rust mapping is the fiddly part and it is pure logic**, so it is unit-tested against
the spellings `tauri-plugin-global-shortcut` accepts, rather than discovered by clicking.

## Errors

Everything the user can get wrong is surfaced in the window, next to the thing that is wrong:

- Validation failure — the save is rejected, nothing is written, the message names the field.
- A hotkey the OS refuses (owned by another app) — the save succeeds, that command shows an inline
  problem. This matters: it is the silent-failure class v1 spent its whole review budget removing,
  and a settings UI that let it pass silently would reintroduce it.
- A backend that will not build — same treatment.

The tray status and log file from v1 remain, unchanged, as the record of what happened at runtime.

## Testing

- **Rust, unit-tested:** the v1→v2 migration (both directions of the version check, and an unknown
  future version rejected), `save_config`'s validate-then-apply ordering, and that a rejected config
  leaves the in-memory config untouched.
- **Frontend, unit-tested with vitest:** the keyboard-event-to-hotkey-string mapping, including
  modifier-only presses ignored, modifier ordering normalised, and a bare key rejected. Plus the
  dirty-state logic.
- **Not unit-tested, verified by hand:** the window showing and hiding, tray interaction, real
  shortcut registration, and that recording does not fire a transform.

## Risks

- **Recording leaves hotkeys suspended.** If a path out of the recorder misses `resume_hotkeys`, the
  app silently has no shortcuts. Mitigated by resuming on every exit path and by a defensive resume
  when the settings window hides.
- **The browser's key names do not match the plugin's spelling.** Punctuation and numpad keys are
  where this will bite. Handled by unit tests over the mapping and by rejecting anything that does
  not map, rather than registering a guess.
- **Rebuilding backends on every save** re-creates HTTP clients and drops any warm state. Harmless
  today; worth remembering if a persistent-session backend is added later.
