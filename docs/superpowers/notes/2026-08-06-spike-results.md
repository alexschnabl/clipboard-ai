# Clipboard AI Spike — Results

**Date:** 2026-08-06
**Status:** Instrument built and reviewed. Manual run not yet done — sections marked **[TO FILL]** need you.
**Code:** `spike/`, 57 tests passing, commits `cb0dd4c..915dca0` (17 commits)

---

## Part A — Pre-flight (about 2 minutes)

```bash
cd E:\Code\clipboard-ai
.venv/Scripts/python -m pytest -q          # expect: 57 passed
ollama list                                 # expect: qwen2.5:32b-instruct present
echo $env:ANTHROPIC_API_KEY                 # expect: empty — a set key invalidates the subscription test
```

---

## Part B — The one measurement still missing

Every backend has been timed except the warm Agent SDK session, which is the spike's third
question. `AgentSDKSessionBackend` was added late and verified only against an in-process fake,
never a live authenticated session.

```bash
.venv/Scripts/python -c "import time; from spike.backends import AgentSDKSessionBackend; b=AgentSDKSessionBackend(); [print(f'{n}: {(lambda s: (b.transform(\"this sentance have two mistake\"), (time.perf_counter()-s)*1000))(time.perf_counter())[1]:.0f} ms') for n in ('cold','warm-1','warm-2','warm-3')]"
```

Record the four figures in the table below. **This is the number that decides whether the
subscription backend is viable for a hotkey tool.** Everything else about it is already known.

---

## Part C — Safety checks, in Notepad, before touching anything real

Do these three in order. Do not skip to Part D if any of them misbehaves.

**C1 — Abort must not paste.** Run `.venv/Scripts/python -m spike.main --backend ollama`, click
into an empty Notepad with **nothing selected**, press `Ctrl+Alt+J`.
Expect: console prints `ABORTED (capture-timeout)`, nothing appears in Notepad, your clipboard is
untouched. If anything pastes here, stop — that is the failure the whole design guards against.

**C2 — Happy path.** Type `this sentance have two mistake` in Notepad, select it, press
`Ctrl+Alt+J`.
Expect: the selection is replaced with a corrected sentence, the console prints `PASTED` and a
timing breakdown, and your clipboard holds whatever it held before.

**C3 — Focus guard.** Select text in Notepad, press `Ctrl+Alt+J`, then immediately click into a
different window before the result arrives.
Expect: `ABORTED (focus-changed)` and nothing pasted anywhere. This is new instrumentation — the
spec named focus loss as a hazard to measure, and this turns it into a recorded result.

---

## Part D — Application matrix **[TO FILL]**

Ten consecutive runs per application, `--backend ollama`. Local and unauthenticated, so the
matrix measures clipboard mechanics rather than network variance.

Read the median **capture latency (t2→t3)** from the console output. That number is the spike's
primary result — it sets the timing budget for the production app.

| Application | Successes /10 | Median t2→t3 | Failure modes seen |
|---|---|---|---|
| Notepad (control) | | | |
| Outlook | | | |
| Teams | | | |
| Browser — plain `<textarea>` | | | |
| Browser — rich `contenteditable` (Gmail) | | | |
| VS Code | | | |
| Word | | | |

If Notepad fails, the mechanism is broken, not the application.

---

## Part E — Backend latency

Measured. Only the last row is outstanding.

| Backend | Cold | Warm | Notes |
|---|---|---|---|
| Ollama `qwen2.5:32b-instruct` | 16,036 ms | 399 ms | Ollama unloads idle models after a few minutes |
| `claude -p` subprocess | 4,389 ms | — | Fresh process per request by definition |
| Agent SDK `query()` | 4,527 ms | 3,999 / 2,998 ms | **All three are cold** — see below |
| Agent SDK persistent session | **[TO FILL]** | **[TO FILL]** | Part B |

**On the `query()` row:** the SDK documents `query()` as *"Stateless: each query is independent"*,
spawning a fresh `claude` CLI subprocess per call. The figures labelled warm during the build were
not warm; they differ only by OS cache warming. This was a defect in the plan's measurement design,
caught in review, and it is why Part B exists.

Also worth carrying forward: disabling the built-in toolset (`tools=[]`) was a correctness fix, not
a latency win — 3,480/8,451/3,384 ms before versus 4,527/3,999/2,998 ms after, i.e. no meaningful
change against high run-to-run variance.

---

## Part F — The three questions

**1. Does the round trip work in the applications that matter?** **[TO FILL from Part D]**

**2. What is the real end-to-end latency, and how much is the model?** Partly answered. The model
dominates in every configuration except a resident local model. The one configuration that feels
instant is Ollama with the model already loaded (399 ms); everything else sits at 3–4.5 seconds,
where the wait is noticeable. Cold Ollama at 16 s is unusable.

Notably, **local is not automatically faster**: a cold 30B load is nearly 4× slower than shelling
out to the Claude CLI. Keep-warm is therefore the single most important backend decision for the
production app, not model choice.

**[TO FILL: the t2→t3 capture cost from Part D, which is the part the model does not explain.]**

**3. Does the Agent SDK authenticate off the Claude Code login, and is a warm session faster?**
Half answered. **Authentication works** — confirmed repeatedly with `ANTHROPIC_API_KEY` unset, no
auth error, correct output. The warm-session half is **[TO FILL from Part B]**.

---

## Part G — Recommendation **[TO FILL]**

Is in-place replacement viable, or does the production app need a different interaction model such
as a floating result window? If the answer is mixed, say which applications each model would serve.

---

## Part H — Carried-forward costs

Things the production app must solve that the spike sidestepped or discovered.

**Platform**
- `modifiers_down()` and `get_foreground_window()` are Windows-only (`ctypes` / `GetAsyncKeyState`
  / `GetForegroundWindow`). Both need a per-OS implementation for macOS and Linux. On Wayland,
  synthetic input and global hotkeys are largely blocked by design — expect the Linux story to be
  degraded or compositor-specific.

**Measurement gaps**
- No Anthropic Messages API backend was built or measured, so there is no hosted-API latency figure.
  Comparison is Agent SDK vs `claude -p` vs local only.
- Capture latency is quantized to the 20 ms poll interval. A median of "40 ms" means roughly 20–60 ms.

**Correctness and safety**
- `<think>` stripping is applied only in `OllamaBackend`. The constraint is written unconditionally,
  so production should move it into the cycle, after `backend.transform`, to cover every provider.
- **Non-text clipboards cannot be restored.** pyperclip is `CF_UNICODETEXT`-only, so an image or file
  list on the clipboard reads as empty and the "restore" overwrites it with empty text. The spec's
  criterion "the original clipboard is correctly restored in every run" is not literally achievable
  with this library.
- Truncation mid-`thinking` tag name (e.g. `<thinki`) is not caught by the guard. Low probability.

**Operational warnings for the matrix run**
- **Disable third-party clipboard managers.** `wait_for_content` returns the *first* non-empty
  clipboard value after the clear; anything else writing within that 1000 ms window gets transformed
  and pasted over your selection.
- **`--backend agent-sdk-session` has a known hazard.** If a request times out, the abandoned task is
  never cancelled and the session is never reset, so a later request can reuse a poisoned session
  whose zombie consumer steals messages from the shared stream. Most likely outcome is repeated safe
  refusals; a lower-probability path exists to a garbled response being pasted. If you see a refusal
  on that backend, restart the process before continuing. Parked deliberately — the fix budget for
  this run was spent.
- If the hotkey appears to stop working, check the console. A cycle still in flight prints
  `busy; hotkey ignored`.

**Design signals for production**
- The warm session reuses a single session id, so context accumulates across calls. Latency will
  climb over a long run and later turns can be contaminated by earlier ones. Production should scope
  a session per request, or reset it periodically.
- A backend that can invoke tools is wrong for this use case. `tools=[]` is the right default for a
  text transformer.

---

## Part I — What the build itself taught us

Worth recording, because it bears on how the production app should be built rather than what it
should do.

The review process caught four defects that the tests did not, all in code that passed its own
suite:

- Deleting `clear_clipboard()` or the 300 ms restore delay left all 38 tests passing. Those two
  lines are the entire mechanism preventing stale-clipboard capture — the failure the spec calls
  non-negotiable. Now pinned by an explicit call-order test.
- An Ollama outage mid-run would have permanently replaced the clipboard with the captured
  selection, because only two exception types were handled and `httpx` raises neither.
- Running the cycle on pynput's message-pump thread blocks the keyboard hook, which can inflate
  the primary measurement by ~300 ms per event and may silently remove the hook mid-run.
- A truncated `<think` fragment reached the paste path twice — once in the original design, and
  once reintroduced by the fix that closed the orphan-closing-tag hole.

The pattern: every one of these lived at a boundary — between two modules, between a library's
documented behavior and its actual behavior, or between the tests and the code they were supposed
to constrain. None were visible from inside a single file.
