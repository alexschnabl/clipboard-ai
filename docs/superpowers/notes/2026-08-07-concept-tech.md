# In-flight feedback: technical feasibility (Tauri v2, Windows)

**Date:** 2026-08-07 · **Scope:** what Tauri v2 / Win32 actually offers, not which to use.
**Verified against:** `tauri` 2.11.5 source at
`~/.cargo/registry/src/index.crates.io-*/tauri-2.11.5/src/{window,webview,tray}` (installed copy
matching `src-tauri/Cargo.lock`), `tauri-plugin-notification` 2.3.3 source (downloaded from
crates.io — not currently a project dependency), and https://v2.tauri.app. The cycle already runs
off a spawned `std::thread` from the global-shortcut handler (`src-tauri/src/lib.rs:336`), not the
main thread — relevant to every "must not block main thread" gotcha below.

## 1. Borderless always-on-top overlay window

All requested primitives exist on `WebviewWindowBuilder`/`Window` in 2.11.5: `decorations(bool)`
(window/mod.rs:558), `always_on_top(bool)` (:572), `skip_taskbar(bool)` (:602),
`transparent(bool)` (:915, gated `not(macos)`/`macos-private-api`, fine on Windows),
`.position(x, y)` builder / `set_position()` runtime (:813, :1866), and
`set_ignore_cursor_events(bool)` for click-through (:2225). Cursor-relative placement is free:
`cursor_position()` exists on `AppHandle`/`Window`/`WebviewWindow` (app.rs:908, others).
**Caret-relative placement is not** — Tauri has no caret API; that needs UI Automation
(`TextPattern`) or MSAA per foreign app, a separate, nontrivial engineering effort.

**Time-to-visible.** Two very different numbers:
- *Pre-created hidden window, reused every cycle*: `show()`/`hide()` exist (window/mod.rs:1784,
  1789) and are cheap Win32 calls; toggling visibility + repositioning an already-live WebView2
  window is on the order of one to a few compositor frames (~16–30 ms). Comfortably inside even
  the 400 ms floor.
- *Building a new `WebviewWindow` per cycle* (`build()`, webview_window.rs:438): this spins up a
  new WebView2 controller/HWND, which is materially more expensive — commonly ~100 ms and up,
  variable with disk/AV. That eats a quarter or more of the 400 ms budget and is a bad fit;
  pre-warming at startup and reusing the window is the only way this is reliably fast.

**Gotcha (source-confirmed):** `WebviewWindowBuilder::new(...).build()` and `add_child()` carry an
explicit doc warning — "On Windows, this function deadlocks when used in a synchronous command or
event handler[; use] async commands and separate threads" (webview/mod.rs:289, 367;
window/mod.rs:152, 233). This app's cycle already runs on its own spawned thread, so calling
`build()`/`show()` there matches the documented-safe pattern — but window mutation still executes
on the main thread under the hood via `run_on_main_thread`, blocking the caller until serviced.

**Verdict:** viable and the strongest option, provided the window is created once at startup and
reused (show/hide/reposition), not rebuilt per cycle.

## 2. Tray icon animation / state swap

Confirmed: `TrayIcon::set_icon(Option<Image<'_>>) -> Result<()>` (tray/mod.rs:499). It runs via a
`run_item_main_thread!` macro — a blocking dispatch to the main thread, cheap (an `NIM_MODIFY` to
the shell), well within a few ms per call. A timer thread calling `set_icon` on pre-built frames
every ~100 ms is a well-worn pattern (Slack/Dropbox-style tray spinners) and costs little.

**Gotcha:** frames must be pre-built `Image` values (loaded once, not decoded per tick) or the
per-frame cost grows. No documented rate limit was found, but very fast intervals (<~50 ms) risk
shell-side coalescing/flicker — untested here, a tuning question, not a blocker.

**Verdict:** viable, cheap, good for "something is happening" but not spatially anchored to the
user's work — the tray may be off-screen (secondary monitor) or hidden in the overflow chevron.

## 3. Tray tooltip updates

Confirmed: `TrayIcon::set_tooltip(Option<S>) -> Result<()>` (tray/mod.rs:523), doc note "Linux:
unsupported" implying Windows is supported. Mechanically identical cost to `set_icon`. **But a
tray tooltip is a native `NOTIFYICONDATA` hover tooltip — it is not visible without the user
resting the mouse over the tray icon.** No API was found to force a tooltip to display. Stated
plainly: this cannot push information onto the screen, only change what appears if hovered.

**Verdict:** not viable as an in-flight feedback channel by itself — useful only as a
"why did that take so long" explanation after the fact, for a user who goes looking.

## 4. Windows native toast notifications

Not currently a dependency; verified by downloading `tauri-plugin-notification` 2.3.3 source. On
desktop (`src/desktop.rs`), `Notification<R>` exposes only `builder()`, `request_permission()`,
`permission_state()`; `NotificationBuilder::show()` builds a `notify_rust::Notification` and calls
`tauri::async_runtime::spawn(async move { let _ = notification.show(); })` — **fire-and-forget,
no handle returned, no id-based update**. The `cancel`/`get-pending`/`get-active`/`remove-active`
permissions on the plugin's docs page belong to `mobile.rs` (Android/iOS), not desktop — confirmed
by `lib.rs`, which wires only `commands::{notify, request_permission, is_permission_granted}` into
the invoke handler; `desktop.rs` defines no cancel/remove methods. **On Windows there is no
programmatic way, through this plugin, to update or dismiss an already-shown toast.**

Windows toasts also route through the OS notification pipeline (COM activation → Action Center
broker) rather than painting directly, so appearance is commonly several hundred ms after the call
returns — not fast, not guaranteed. They display briefly (~5 s default) then persist in Action
Center, and can be silently suppressed by Focus Assist. `show()` itself returns near-instantly
(it just spawns the async task) — return does not mean rendered.

**Verdict:** not viable for sub-second, self-clearing feedback; wrong latency profile and wrong
lifecycle (can't be taken back once shown).

## 5. Cursor change

Tauri only exposes `Window::set_cursor_icon(CursorIcon)` (window/mod.rs:2207) for **the app's own
window** — it changes the cursor while hovering that window's client area. There is no Tauri API
to change the cursor over a **different application's** focused window (the whole point here — the
target app has focus, not ours). Doing that means bypassing Tauri and calling raw Win32
(`SetSystemCursor`, reachable via the `windows` crate already a project dependency), which replaces
a cursor shape **system-wide**, not per-app — every application sees it until explicitly restored
via `SystemParametersInfoW(SPI_SETCURSORS, ...)`. If the process crashes or a restore call is
skipped on an error path, the user is left with a stuck busy cursor until logoff or another app
resets it. High blast radius for a cosmetic cue.

**Verdict:** technically possible only via raw Win32 outside Tauri's surface, invasive
(system-wide, not scoped to the foreground app), and carries real stuck-cursor risk. Not
recommended as a first option.

## 6. Other mechanisms found

- **Taskbar flash**: `Window::request_user_attention(Some(UserAttentionType::Informational |
  Critical))`, confirmed (window/mod.rs:1942; enum in `tauri-runtime` 2.11.3 lib.rs:82), maps to
  Win32 `FlashWindow`. Gotcha: flashes a taskbar button/border — a tray-resident app with
  `skip_taskbar(true)` and no visible main window has nothing for this to flash.
- **Taskbar overlay badge**: `Window::set_overlay_icon(Option<Image>)`, Windows-only
  (window/mod.rs:2255); `set_badge_count`'s own doc says "Windows: Unsupported, use
  `set_overlay_icon` instead." Same taskbar-presence gotcha as above.
- **Thin progress line at a screen edge**: not a distinct primitive — the §1 overlay window sized
  as a strip. Same cost/latency profile as §1's pre-warmed case.
- **Audio (system beep)**: no Tauri audio API; the `windows` crate (already a dependency) exposes
  `MessageBeep`/`PlaySound`, effectively instant to trigger and perceived within ~50–150 ms.
  Non-visual, easy to make obnoxious on repeat use, but genuinely fast and simple.

## Bottom line on latency

| Mechanism | Time-to-visible | Notes |
|---|---|---|
| Pre-warmed overlay show/reposition | ~16–30 ms | needs startup pre-creation |
| Overlay built fresh per cycle | ~100 ms+ | too slow for 400 ms cycles |
| Tray icon swap | few ms | not spatially anchored |
| Tray tooltip | N/A (hover-gated) | not proactive |
| Toast notification | several hundred ms+ | can't update/dismiss once shown |
| Cursor change (own window) | ~ms | wrong window (target app has focus) |
| Cursor change (system-wide) | ~ms | works but invasive, stuck-cursor risk |
| Taskbar flash / overlay badge | ~ms | needs a taskbar-visible window; conflicts with tray-only design |
| System beep | ~50–150 ms perceived | non-visual |
