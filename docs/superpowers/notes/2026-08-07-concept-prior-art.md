# Prior Art: Signaling In-Flight Work After a Hotkey

How comparable tools acknowledge a hotkey and show progress/failure for background work. Claims are sourced; inferences labeled.

## Text/AI transform tools

**Raycast AI.** Two-tier system: `Toast` (Animated → Success/Failure, non-modal, dismissible, can carry a "copy error" action) for in-progress work, and `HUD` (compact bottom-of-screen message) for when Raycast's window closes as a side effect — closest analog to "paste and get out of the way." No documented pre-toast delay. Changelog shows real regressions: a "Script Running…" toast stuck past completion (v1.60), toast/HUD on the wrong monitor (fixed v1.62). (developers.raycast.com/api-reference/toast; raycast.com/changelog)

**Alfred / ChatFred.** "Large Type" (Cmd+L) shows results centered on screen, persisting until clicked — not a timed toast. Alfred's forum documents the failure modes to avoid: a perceptible delay before appearing (users asked for instant), a fade-out regression, and wrong-screen placement on multi-monitor rigs (fixed v2.0.7). Community workflow **ChatFred** shows a static "💭 Stay tuned... ChatGPT is thinking" string (configurable/disableable), with an optional streaming window. (alfredforum.com/topic/13063; github.com/chrislemke/ChatFred)

**Espanso.** Forms have no documented appearance delay or animation. Two open issues are on point: #958, the search bar **steals focus** from the active window (breaks apps that auto-close dialogs on blur) — cites PhraseExpress's non-focus-stealing menu as the model; #983, forms always render on the **primary monitor**, ignoring which screen the user was typing on. (github.com/espanso/espanso/issues/958, /983)

**TextExpander / PhraseExpress.** Weaker analogs — TextExpander's AI drafts snippets ("appear in seconds"), not live transforms; no loading UI documented. PhraseExpress notifications anchor to its **menu-bar icon**, not cursor/caret. (textexpander.com; phraseexpress.com/mac/doc/macros/output)

## Overlay / inline AI tools

**Grammarly.** Ambient, not hotkey-triggered: a floating "G" icon docks near text fields, turning green to red as issues are found; a stuck-spinning icon is the de facto "something's wrong" signal, with no other documented error UI. (support.grammarly.com, "G icon constantly spinning")

**Apple Writing Tools.** Documented feedback is minimal but concrete: completed edits show as text **"underlined with a glowing line"** on the selection — feedback lives on the text, not a separate overlay. A third-party analysis (unverified vs. primary Apple sources) describes a ~1.8s rotating "shimmer" while processing, fading ~0.4s on completion. Failure is plain-language: **"Writing Tools Unavailable — try again later."** (support.apple.com/guide/mac-help; discussions.apple.com)

**Windows Copilot.** A "thinking"/typing-indicator bubble exists but is undocumented in detail; Microsoft Q&A confirms a bug where it sticks after the reply finishes, with no no-network error state documented. (learn.microsoft.com/answers/questions/4375799)

**PowerToys Advanced Paste.** Hotkey opens a visible window (not silent). Long-running actions, including "Paste with AI," get a **cancel (X) button**; a progress ring exists but per docs is wired only to media transcoding — AI paste has cancellability but no fractional progress. Errors are inline strings ("API key quota exceeded," "Invalid API key or endpoint") users call too opaque to debug. An auto-close-on-focus-loss setting shows focus was a deliberate concern. (learn.microsoft.com/windows/powertoys/advanced-paste; github.com/microsoft/PowerToys/issues/35461)

## Clipboard managers

**Ditto.** Popup position is configurable: at caret, at cursor, or at last position, falling back to screen-center if caret can't be found. Not silent on failure — a real hotkey-conflict dialog ("Error Registering DittoHotKey") is shown. An open issue reports several-second delays with no maintainer response. (github.com/sabrogden/Ditto/issues/439, /777)

**CopyQ.** No cursor-anchored popup by default (a feature request cites Win+V as the model). Its "show under cursor" mode is buggy on first use and historically primary-monitor-only. CopyQ's own docs admit its window **steals keyboard focus**, listing casualties: aborted file renames, closed Start menus, dismissed dialogs — contrasted unfavorably with Windows' own clipboard tool. (copyq.readthedocs.io/en/latest/known-issues.html; github.com/hluk/CopyQ/issues/2737)

**Windows Clipboard History (Win+V).** Defaults to bottom-right corner (secondary sources only; no primary MS doc for exact placement/timing). Failure modes are silent-ish: a 24H2 bug shows a **blank panel with no error**, and users report the panel appearing for half a second and self-closing, root cause never identified. (learn.microsoft.com/answers/questions/3903760; notebookcheck.net)

## Screenshot / capture tools

**ShareX.** Switched from Windows balloon tips to a custom corner toast (v13.6) with configurable duration/placement; v14 auto-hides during a new capture to stop toasts leaking into the next shot. An option suppresses the toast under fullscreen apps. Upload failures surface raw HTTP error text — diagnostic, not actionable. Fixed to the **primary monitor only**, a recurring feature request. (getsharex.com/changelog; github.com/ShareX/ShareX/issues/8342)

**Windows Snipping Tool.** Screen dims almost immediately on Win+Shift+S; after capture a corner toast offers "click to edit," with standard Windows ~5s auto-dismiss. Failure is essentially unsignaled: Focus Assist silently swallows the toast, "auto-save" silently disables it, and Microsoft Q&A documents notifications arriving up to 5 minutes late under rapid presses, with no distinguishable error state. (learn.microsoft.com/answers; support.microsoft.com)

**Flameshot.** Hotkey-to-overlay latency is inconsistent — ~1s extra lag via keyboard vs. tray icon, and one v11.0.0 report of a **25-second hang** after capture. A bright flash + shutter sound at capture time was called "unpleasant" in dark/quiet settings; maintainers closed the request to disable it as **Won't Fix**. Multiple "wrong monitor" bugs on multi-monitor/Wayland setups. (github.com/flameshot-org/flameshot/issues/2425, /3426)

**Bonus — launchers (PowerToys Run, Wox).** Same shape, no AI: issues describe 1–5s hotkey-to-window delay, especially cold-start, with **no separate acknowledgment** from the final UI — any lag reads as "hotkey didn't register." PowerToys Run also regressed from top-of-screen to dead-center positioning, called "extremely annoying." (github.com/microsoft/PowerToys/issues/10429; github.com/Wox-launcher/Wox/issues/424)

## Patterns worth stealing

1. **Two-tier feedback (Raycast).** A cheap, instant "acknowledged" signal separate from the eventual result — Toast-while-waiting, HUD-on-completion — rather than one UI doing both jobs.
2. **Delay-gated appearance (inferred, not directly documented anywhere).** No tool specifies a "wait N ms before showing a spinner" threshold, but the need is implied everywhere: every tool skipping an instant-ack (PowerToys Run, Wox, Ditto, CopyQ) draws "did my hotkey even work?" complaints. A short suppression window, long enough a 400ms response never flashes, looks like the right inferred design.
3. **Feedback lives on the text (Apple Writing Tools).** A glowing underline on the edited selection avoids a separate floating panel — directly relevant since this app pastes back over a selection.
4. **Plain-language failure states (Apple, Ditto).** "Writing Tools Unavailable — try again later" and "Error Registering DittoHotKey" beat ShareX's raw HTTP dumps or Snipping Tool's silent no-op.
5. **Cancel affordance for long ops (PowerToys Advanced Paste).** A visible cancel control for anything that can run long, even without full progress reporting.
6. **Auto-suppress on rapid re-trigger (ShareX v14).** Hide/replace the prior indicator on a new cycle so stale toasts don't pollute the next action.

## Anti-patterns

1. **Wrong monitor.** The most repeated bug class: Alfred, Espanso, CopyQ's cursor mode, ShareX toasts, and Flameshot's capture/GUI all independently shipped fixed-to-primary-monitor bugs later reported as complaints.
2. **Focus stealing.** CopyQ's docs list casualties (aborted renames, closed Start menu, dismissed dialogs); Espanso #958 stays open for the same reason. Grabbing focus during a fast hotkey action reliably draws complaints.
3. **Indicator outlives the work (Raycast, Alfred).** Both shipped "still showing progress after it finished" bugs — worse than no indicator at all.
4. **Silent failure indistinguishable from success.** Windows Clipboard History's blank panel and Snipping Tool under Focus Assist leave users unable to tell "missed the toast" from "silently failed."
5. **Flash/sound as acknowledgment.** Flameshot's capture flash+shutter sound drew a Won't-Fix complaint for being jarring in dark/quiet rooms.
6. **Opaque errors.** ShareX's raw HTTP text and PowerToys' "Invalid API key or endpoint" drew complaints for being present but undiagnosable.
