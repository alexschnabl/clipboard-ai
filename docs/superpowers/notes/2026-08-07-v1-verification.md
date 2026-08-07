# Clipboard AI v1 — Verification Checklist

**Date:** 2026-08-07
**Status:** Code complete, 71 tests passing, reviewed. Manual verification **[TO DO]**.
**Code:** `src-tauri/`, commits `a9b22dc..9980fdc` (12 commits)

Run these in order. Do not skip to Outlook.

---

## Step 0 — Build

```powershell
cd E:\Code\clipboard-ai\src-tauri
cargo test          # expect: 71 passed
```

If cargo fails at the **link** step, this machine's shells lack MSVC paths. Use a Developer
PowerShell prompt, or prefix in a single call:

```
cmd /c '"D:\Applications\Work\Visual Studio\18\VC\Auxiliary\Build\vcvars64.bat" && cd /d E:\Code\clipboard-ai\src-tauri && cargo test'
```

---

## Step 1 — The input-stack gate (never run; do this first)

Task 1's smoke test was handed over and not performed, so **the Rust input stack is still unproven
against a real application.** Everything else was built on that assumption. Two minutes:

```powershell
cargo run --bin smoke
```

Type `hello world` in Notepad, select it, click into Notepad during the 3-second countdown.

- **Pass:** selection becomes `[hello world]`, console prints `OK`, clipboard unchanged.
- **`FAIL: nothing captured`** — `enigo`'s Ctrl+C is not reaching the window. Stop; that is a
  design-level finding, not a bug to work around.

---

## Step 2 — Start the app

```powershell
cd E:\Code\clipboard-ai
npm run tauri dev
```

Expect: no window, a tray icon. The tray menu shows a status line, one informational entry per
configured prompt, Open config file, Reload config, Show log, Quit.

Config lives at `%APPDATA%\com.clipboardai.app\config.json` and is written with defaults on first
run — one Ollama backend, one `claude -p` backend, one `Fix grammar` prompt on `Ctrl+Alt+J`.

---

## Step 3 — The abort gate, in Notepad, before anything real

Click into an **empty** Notepad with **nothing selected**. Press `Ctrl+Alt+J`.

Expect: tray status shows `capture-timeout`, **nothing pasted**, clipboard untouched.

**If anything pastes here, stop.** This is the failure every design decision in this project exists
to prevent.

---

## Step 4 — The happy path

Type `this sentance have two mistake` in Notepad, select it, press `Ctrl+Alt+J`.

Expect: the selection is replaced with corrected text, the tray status shows `ok` with a timing,
and your clipboard holds whatever it held before.

---

## Step 5 — The focus guard

Select text, press `Ctrl+Alt+J`, then immediately click into another window.

Expect: `focus-changed`, nothing pasted anywhere.

---

## Step 6 — The sentinel, against `claude -p`

Tray → Open config file. Point `fix-grammar`'s `backendId` at `claude-sub`. Save, restart the app
(reload is not live in v1). Repeat Step 4.

Expect: **just the corrected sentence.** Measured raw `claude -p` output during the build was:

```
**This sentence has two mistakes.**

Corrections made:
- "sentance" → "sentence" (spelling)
```

If any of that markdown or commentary reaches your document, the sentinel is not working.

---

## Step 7 — A dead backend must not kill the app

Stop Ollama. Press the hotkey.

Expect: `backend-error` in the tray status, nothing pasted, clipboard intact, **and the app still
running** — the next press retries. Check tray → Show log records it.

---

## Step 8 — A broken config must still give you a tray

Tray → Open config file, introduce a trailing comma, save, restart.

Expect: the app still starts, a tray icon still appears, and the status reads `config error — see
log`. It must not die silently.

---

## Step 9 — The built app, not just dev

```powershell
npm run tauri build
```

Run the binary from `src-tauri/target/release/`. **This is the configuration that matters** — there
is no console, so the tray status and log file are the only way to see anything. Repeat Steps 3, 4
and 7 against it.

---

## Step 10 — Multiple prompts

Add a second prompt to the config with a different hotkey and a different backend, e.g.:

```json
{ "id": "concise", "name": "Make concise", "backendId": "claude-sub",
  "hotkey": "Ctrl+Alt+K", "systemPrompt": "Rewrite this text to be more concise." }
```

Restart. Verify both hotkeys work and each uses its own prompt and backend.

---

## Step 11 — Only now, the real applications

Outlook, Word, Teams, a browser textarea, a rich contenteditable, VS Code.

Record for each: does it work, and the capture timing from the log.

---

## Before you start — three things to know

**Turn off third-party clipboard managers.** The capture loop returns the *first* non-empty
clipboard value after clearing, so anything else writing within that 1000 ms window gets transformed
and pasted over your selection. Inherited from the spike; not defended against in code.

**A non-text clipboard is destroyed.** If you have an image or file list copied and then use the
hotkey, the clear wipes it and it cannot be restored — `arboard` is text-only. Known v1 limitation.

**A press that does nothing may mean a cycle is still running.** Check tray → Show log; you will see
`busy; hotkey ignored`. Cold Ollama can take 15+ seconds.

---

## Known open items (parked, none load-bearing)

- If a model echoes the instruction's `<output></output>` pair verbatim, extraction refuses the
  whole response rather than finding the real block. Fails safe — refuses, logged, surfaced — but
  burns the cycle.
- Tray menu entries iterate a hash map, so their order varies between launches, and hotkey labels
  show the plugin's normalized spelling (`control+alt+j`) rather than your config's.
- A hotkey rejected by the OS because another app owns it is still listed in the tray, though the
  failure is logged and counted in the status line.
- Config reload is not live: hotkey changes need a restart. Lands with the v2 settings UI.
- `claude_cli` can leave abandoned reader threads if a killed process leaves an orphan descendant
  holding the pipe. Bounded per call; a Windows Job Object would close it properly.

## What v1 deliberately excludes

Settings UI (v2 — you have specs), formatting preservation, the review-before-replace popup, a
persistent-session sidecar for the faster Claude path, and macOS/Linux.
