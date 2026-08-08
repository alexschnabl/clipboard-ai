# Clipboard AI v2 — manual verification

**Date:** 2026-08-07
**Scope:** the settings window, plus the parts of the v1 cycle that changed underneath it.

Everything below needs a real GUI, a real tray, and real OS-level hotkey registration, so none of it
is covered by the automated suites (163 Rust tests, 113 frontend tests, all passing).

## Before you start

**Back up your config.** Section 7 deliberately corrupts it:

```
copy "%APPDATA%\com.clipboardai.app\config.json" "%APPDATA%\com.clipboardai.app\config.backup.json"
```

Start with `npm run tauri dev` from `E:\Code\clipboard-ai`. The log is at
`%APPDATA%\com.clipboardai.app\clipboard-ai.log` — worth tailing.

---

## 1. The one that matters — a new hotkey works without a restart

This is why v2 exists. v1 registered hotkeys once, at startup.

- [ ] Open Settings from the tray (left-click the icon, or tray menu → Settings).
- [ ] Commands tab → **Add command**. Name it, pick a backend, write a prompt.
- [ ] Click the hotkey field and press a combination you don't already use. It must display in
      **your** spelling (`Ctrl+Alt+K`), not `control+alt+KeyK`.
- [ ] Save. The dialog closes.
- [ ] **Without restarting**, select text in Notepad and press that combination.

**Pass:** the text is transformed in place.
**Fail:** nothing happens → check the tray menu lists the new command, and the log for
`already in use`.

## 2. Saving is immediate — there is only one Save

The two-stage save is gone: the dialog's Save writes to disk. There is no second Save button.

- [ ] Add or edit a backend, click Save, then **quit the app from the tray and restart it**.

**Pass:** your change is still there. No separate "Save" step was needed and there is no
"Unsaved changes" badge anywhere.

Then provoke a rejection:

- [ ] Edit a command's hotkey to one **another command already uses**, and Save.

**Pass:** the dialog **stays open** with the error shown inside it, and the list behind it still
shows the old value — the change is rolled back, not left dangling.
**Fail:** the dialog closes, or the list shows a change that didn't survive a restart.

## 3. Recording does not fire the command being recorded

- [ ] Note an existing command's hotkey, say `Ctrl+Alt+J`.
- [ ] Edit that command, click the hotkey field, press `Ctrl+Alt+J`.

**Pass:** captured into the field. No transform runs, your clipboard is untouched.
**Fail:** a cycle runs → `suspend_hotkeys` isn't taking effect.

## 4. Escape cancels recording — it does not throw your form away

This was a real bug: Escape used to close the whole dialog and discard everything typed.

- [ ] Add a command, type a name and a prompt, click the hotkey field, then press **Escape**.

**Pass:** recording stops; the dialog stays open with your name and prompt intact.
**Fail:** the dialog closes and your typing is gone.

- [ ] While recording, press **Tab**.

**Pass:** focus moves normally (Tab is not swallowed).

## 5. Hotkeys come back after every exit from recording

Test each separately, and after **each one** press a real hotkey in Notepad to confirm shortcuts
still work.

- [ ] Record a combination and commit it.
- [ ] Start recording, press `Escape`.
- [ ] Start recording, click elsewhere in the window.
- [ ] Start recording, then close the dialog with Cancel or its X.
- [ ] **Start recording, then close the whole settings window** (X or Alt+F4).

The last one was a real bug — the window is hidden, not destroyed, so the recorder used to stay
armed over live hotkeys. After it, reopen Settings and confirm the field is **not** still showing
"Press a combination…".

**Failure on any of these means no working hotkeys at all** — the worst failure mode in v2.

## 6. A failed hotkey registration is visible on its own command

- [ ] Add a command using a hotkey another running application already owns, and Save.

**Pass:** an error appears **on that command's row**, naming the command and *your* spelling of the
hotkey. The rest of the config still saves.

- [ ] Now **restart the app** with that command still configured.

**Pass:** the problem is visible in the settings window immediately on open — not only after you
save something unrelated. (This was broken twice: once because the message text could never match
the row, once because startup problems were never sent to the UI at all.)

## 7. Delete asks first

- [ ] Delete a command. Confirm you get a confirmation naming it, and that Cancel leaves it alone.
- [ ] Try to delete a backend that a command still uses.

**Pass:** refused, naming the commands that use it.

## 8. Corrupt config recovery — the data-loss path

Take the backup first.

- [ ] Quit from the tray. Edit `config.json` and add a trailing comma after the last entry.
- [ ] Start the app.

**Pass:** the app still reaches a tray icon. Settings shows a **prominent red alert** saying the
config could not be read and naming where it was preserved. `config.json.corrupt` exists alongside
it, containing your original content.
**Fail:** the window looks like a fresh install with no warning — that is the state where one Save
would have destroyed your config.

- [ ] Save something anyway (you have two copies).

**Pass:** it saves. You are not trapped in a broken state.

- [ ] Restore from `config.json.corrupt` or your backup and restart.

## 9. Window lifecycle

- [ ] Tray left-click opens Settings; tray menu → Settings opens the same window.
- [ ] With it already open, click the tray again — **one** window, focused, not a second.
- [ ] Alt+Tab reaches it while visible.
- [ ] Closing hides it; the app stays in the tray. Quit is tray-only.
- [ ] Quit from the tray **while a cycle is running** (start a slow one, then quit).

**Pass:** the app exits and your clipboard still holds what it did before the cycle started.
Known limit: a cycle still inside a long backend call can exceed the 2-second wait and lose it.

## 10. Appearance

The shadcn preset changed to `rhea` — teal accents, Roboto, cooler greys.

- [ ] With the theme set to **System** (Settings section), switch Windows between Light and Dark
      with the window open.

**Pass:** the window follows the OS theme, both readable. (An in-app System/Light/Dark control
now lives in the Settings section — an explicit Light or Dark choice there overrides the OS.)

- [ ] Give a command a very long name.

**Pass:** the name truncates with an ellipsis; Edit and Delete stay reachable.

## 11. Backend editing

- [ ] Add each kind and confirm the fields change: `openai-compatible` (base URL, model, optional
      key), `anthropic-api` (model, key), `claude-cli` (name and timeout only).
- [ ] Confirm the API key is masked and the reveal toggle works.
- [ ] Set a timeout below 5000 ms.

**Pass:** Save is blocked with a visible reason. (5 s is the floor; below it even a warm backend
cannot succeed. A cold local model takes ~16 s, so leave the 120 s default unless you know better.)

- [ ] Edit a backend, then run a command that uses it — the change applies without a restart.

---

## Known and accepted

Not bugs to report:

- **The corrupt-config warning persists for the life of the process** — it stays visible after a
  successful save until you restart.
- **Validation errors appear at the top of the page** as well as in the dialog.
- **A `claude-cli` timeout cannot kill grandchild processes** — no Windows Job Object — so a `claude`
  subprocess can outlive its timeout.
- **Holding only `Ctrl` in the recorder** briefly shows "Hold a modifier and press a key", which is
  what you are already doing. It clears as soon as you complete the combination.

## If something fails

`%APPDATA%\com.clipboardai.app\clipboard-ai.log` records every startup and cycle. `STARTUP` lines
cover config load, migration and hotkey registration. The tray tooltip carries the last outcome.
Backend failures now carry their real cause (wrong key, connection refused, `claude` not on PATH)
rather than a bare `backend-error`.

## Addendum: 2026-08-09 — security hardening and later work

The checks above still apply as written. These cover what changed since: the ACL/capability
split (`capabilities/default.json` + `capabilities/overlay.json` plus `build.rs`'s app permission
manifest), the cursor-following indicator, whole-window i18n, and history/cycle concurrency.

- [ ] **All six IPC commands still work after the ACL hardening.** From the settings window,
      exercise each of `get_config`, `save_config`, `suspend_hotkeys`/`resume_hotkeys` (record a
      hotkey), `get_history`, `clear_history`. **Pass:** all six behave normally. Then check the
      failure side is visible, not silent: temporarily remove one `allow-*` permission line from
      `capabilities/default.json`, restart, and try the matching action.
      **Pass:** it fails with a visible error, not a silent no-op.
- [ ] **The in-flight indicator is a small pulsing dot that follows the cursor.** Run a command
      and watch it track the mouse. **Pass:** a slow backend call visibly escalates after ~2.5s;
      an aborted cycle and a real backend failure render with distinctly different colors, not
      the same treatment.
- [ ] **Switching language translates the whole window, not just the sidebar.** Change the
      language in Settings. **Pass:** the History tab and the Settings panel itself (not only
      Commands/Backends) are translated too.
- [ ] **History survives a Clear-during-cycle attempt.** Start a slow-backend command, then click
      "Clear" in the History tab while it is still running. **Pass:** `history.json` is not left
      corrupt or duplicated once the cycle finishes; the app does not crash.
