# Clipboard AI v2 — manual verification

**Date:** 2026-08-07
**Scope:** the settings window. v1's cycle behaviour was verified by hand earlier and is unchanged.

Everything below needs a real GUI, a real tray and real OS-level hotkey registration, so none of it
could be covered by the automated suites (98 Rust tests, 63 vitest).

## Before you start

**Back up your config.** Several checks below deliberately corrupt or overwrite it:

```
copy "%APPDATA%\com.clipboardai.app\config.json" "%APPDATA%\com.clipboardai.app\config.backup.json"
```

Start the app with `npm run tauri dev` from `E:\Code\clipboard-ai`.

The log is at `%APPDATA%\com.clipboardai.app\clipboard-ai.log` — worth having open in a tail.

---

## 1. The one that matters — a new hotkey works without a restart

This is v2's whole reason for existing. v1 registered hotkeys once at startup.

- [ ] Open Settings from the tray (left-click the icon, **or** tray menu → Settings).
- [ ] Commands tab → **Add command**. Give it a name, pick a backend, set a prompt.
- [ ] Click the hotkey field, press a combination you are not already using. It should display in
      your spelling (`Ctrl+Alt+K`), not `control+alt+KeyK`.
- [ ] Save.
- [ ] **Without restarting**, select text in Notepad and press that combination.

**Pass:** the text is transformed in place.
**Fail:** nothing happens → check the log for `already in use`, and check the tray menu lists the
new command.

## 2. Recording does not fire the command it is recording

- [ ] Note an existing command's hotkey — say `Ctrl+Alt+J`.
- [ ] Open Settings, edit that command, click the hotkey field, and press `Ctrl+Alt+J`.

**Pass:** the combination is captured into the field. No transform runs.
**Fail:** a cycle runs (clipboard changes, text gets pasted somewhere) — that means
`suspend_hotkeys` did not take effect.

## 3. Hotkeys come back after every exit from recording

Test each of these separately. After **each** one, press a real hotkey in Notepad to confirm
shortcuts still work.

- [ ] Record a combination and commit it.
- [ ] Start recording, press `Escape`.
- [ ] Start recording, click elsewhere in the window (blur).
- [ ] Start recording, then close the command dialog with its X / Cancel.
- [ ] **Start recording, then close the whole settings window** (X or Alt+F4).

The last one was a real bug found in review: the window is hidden rather than destroyed, so the
recorder used to stay armed. After this one, reopen Settings and confirm the hotkey field is **not**
still showing "Press a combination…".

**Fail on any of these = no working hotkeys at all**, which is the failure mode v2 was most at risk of.

## 4. A hotkey the OS refuses shows on its own command row

- [ ] Add a command with a hotkey Windows already owns — `Ctrl+Alt+Delete` won't reach us; try
      something another running app holds, or launch a second instance of a program that grabs one.
- [ ] Save.

**Pass:** an error appears **on that command's row** naming the command and your spelling of the
hotkey, plus a page-level summary. The rest of the config still saves.
**Fail:** only the page-level summary appears, or nothing does.

This is the case v2's review found silently broken (the row match compared `Ctrl+Alt+J` against the
parser's `control+alt+KeyJ`), so it is worth actually provoking rather than assuming.

## 5. Unsaved-changes guard

- [ ] Make an edit. Do **not** save. Close the window.
- [ ] Confirm you are asked. Choose **Cancel** — the window stays open, the edit is still there.
- [ ] Close again, choose **Discard**. The window closes.
- [ ] Reopen Settings.

**Pass:** the discarded edit is gone, and no edit dialog is still hanging open from before.
**Fail:** your discarded change reappears, or a dialog is still open showing the old values.

Then:

- [ ] Make an edit, **Save**, then close.

**Pass:** closes immediately with no prompt (saving cleared the dirty flag).

## 6. Delete asks first

- [ ] Delete a command. Confirm you get a confirmation naming it, and that Cancel leaves it alone.
- [ ] Try to delete a backend that a command still uses.

**Pass:** refused, with a message naming the commands that use it.

## 7. Corrupt config recovery — the data-loss path

This is the one to be careful with; take the backup first.

- [ ] Quit the app from the tray.
- [ ] Open `config.json` and break it — add a trailing comma after the last entry in `commands`.
- [ ] Start the app.

**Pass:** the app still reaches a tray icon. Opening Settings shows a **prominent red alert** saying
the config could not be read, and names where it was preserved. Check that
`config.json.corrupt` exists next to it, containing your original content.
**Fail:** the settings window looks like a fresh install with no warning — that is the state where
one click of Save would have destroyed your config.

- [ ] Now click Save (you have a backup, and `config.json.corrupt` also holds your data).

**Pass:** it saves. You are not trapped in a broken state.

- [ ] Restore your real config from `config.json.corrupt` or your backup, and restart.

## 8. Window lifecycle

- [ ] Tray left-click opens Settings. Tray menu → Settings opens the same window.
- [ ] With Settings already open, click the tray again — **one** window, focused, not a second.
- [ ] Alt+Tab reaches the settings window while it is visible.
- [ ] Closing hides it; the app stays in the tray. Quit is tray-only.

## 9. Theme

- [ ] Change Windows to Dark (Settings → Personalisation → Colours), with Clipboard AI's window open.

**Pass:** the window follows the system theme. There is deliberately no in-app toggle.

## 10. Backend editing

- [ ] Add each of the three kinds and confirm the fields change: `openai-compatible` (base URL,
      model, optional key), `anthropic-api` (model, key), `claude-cli` (name and timeout only).
- [ ] Confirm the API key field is masked, and the reveal toggle actually shows it.
- [ ] Try a timeout below 1000 ms — Save should be blocked with a visible reason.
- [ ] Edit a backend, then run a command that uses it, to confirm the change took effect live.

---

## Known and accepted

Not bugs to report — these were found in review and deliberately left:

- **`updateBackend` marks the config dirty even on a no-op edit.** Cosmetic; the dirty badge can
  appear without a real change.
- **Holding only `Ctrl` in the recorder shows "Hold a modifier and press a key"** — which is what
  you are already doing. The message clears as soon as you complete the combination.
- **A frontend that acknowledges the close prompt and then hangs** leaves the settings window open.
  Tray Quit still works. Narrow enough to accept.
- **`config_load_error` persists for the life of the process** — the corrupt-config warning stays
  visible even after a successful save, until you restart. Matches the existing tray-status
  behaviour.
- **One shared Save button** for both tabs, rather than one per screen as the spec sketched.
  Deliberate: there is one config and one dirty state.
- **Validation errors appear at the top of the page**, not on the offending field.

## If something fails

The log records every startup and every cycle: `%APPDATA%\com.clipboardai.app\clipboard-ai.log`.
`STARTUP` lines cover config load, migration, and hotkey registration problems. The tray tooltip
carries the last outcome.
