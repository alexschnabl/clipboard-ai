# Clipboard AI Capture/Paste Spike — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a throwaway Windows Python spike that measures whether the capture → transform → paste-back round trip works reliably in real applications, and how the three Claude backends compare on latency.

**Architecture:** Five small modules under `spike/`. The OS-touching layer (clipboard, keystrokes) is separated from the orchestration layer so the safety-critical rules — never paste stale text, never paste on a refusal — are covered by fast unit tests with injected fakes. Every function that waits on time or the OS takes its clock, sleep, and probe as parameters, so no test sleeps.

**Tech Stack:** Python 3.11+, pytest, pynput, pyperclip, httpx, claude-agent-sdk.

## Global Constraints

Copied verbatim from `docs/superpowers/specs/2026-08-06-clipboard-ai-spike-design.md`. Every task's requirements implicitly include this section.

- Windows only. macOS and Linux are out of scope.
- Hotkey: `Ctrl+Alt+J`
- System prompt: `Fix grammar and spelling. Return only the corrected text, with no preamble, explanation, or commentary.`
- Clipboard capture timeout: 1000 ms, polled every 20 ms
- Clipboard restore delay after paste: 300 ms
- Ollama backend: OpenAI-compatible endpoint at `http://localhost:11434/v1`, model `qwen2.5:32b-instruct`, non-streaming
- Any `<think>...</think>` block must be stripped from backend output before it can be pasted. Two of the models installed locally are reasoning models that emit them.
- **Never paste on abort.** A capture timeout or a backend refusal must restore the original clipboard and paste nothing. A single occurrence of pasting stale or wrong text is a failure of the whole approach.
- The original clipboard must be restored in every run, including aborted ones.
- Throwaway code. No config files, no UI, no persistence, no provider abstraction beyond the three concrete backends.

## File Structure

| File | Responsibility |
|---|---|
| `spike/timing.py` | `StageTimer` — records named stage marks, reports deltas |
| `spike/clipboard.py` | Clipboard read/write/clear, and the poll-until-content loop |
| `spike/keys.py` | Modifier-release wait and synthetic copy/paste keystrokes |
| `spike/backends.py` | The three backends and the shared system prompt |
| `spike/cycle.py` | `run_cycle()` — orchestrates one hotkey press end to end |
| `spike/main.py` | Hotkey registration, wiring, console report |
| `tests/fakes.py` | `FakeClock` and fake response objects shared across tests |
| `tests/test_*.py` | One test module per source module |

`cycle.py` is deliberately separate from `main.py`: `run_cycle()` takes every dependency as a parameter and touches no globals, which is what makes the abort rules testable. `main.py` is the untestable shell — hotkey registration and printing.

---

### Task 1: Project scaffold and StageTimer

**Files:**
- Create: `requirements.txt`
- Create: `spike/__init__.py` (empty)
- Create: `spike/timing.py`
- Create: `tests/__init__.py` (empty)
- Create: `tests/fakes.py`
- Test: `tests/test_timing.py`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `spike.timing.StageTimer(clock: Callable[[], float] = time.perf_counter)`
  - `StageTimer.mark(stage: str) -> None`
  - `StageTimer.elapsed_ms(from_stage: str, to_stage: str) -> float`
  - `StageTimer.report() -> str`
  - `tests.fakes.FakeClock(step: float = 0.02)` — callable, returns 0.0 then advances by `step` on each call

- [ ] **Step 1: Create the virtualenv and requirements file**

Create `requirements.txt`:

```
pytest==8.3.4
pynput==1.7.7
pyperclip==1.9.0
httpx==0.28.1
claude-agent-sdk
```

Then run:

```bash
python -m venv .venv
.venv/Scripts/python -m pip install -r requirements.txt
```

`claude-agent-sdk` is unpinned deliberately — the spike wants the current version.

If `pynput==1.7.7` fails to install on Python 3.14, drop the pin and install the latest `pynput` instead, then record which version you got in your report. Do not substitute a different library: pynput was chosen precisely because it carries forward to macOS and Linux.

- [ ] **Step 2: Create the empty package markers**

Create `spike/__init__.py` and `tests/__init__.py`, both empty files.

- [ ] **Step 3: Write the fake clock**

Create `tests/fakes.py`:

```python
class FakeClock:
    """Monotonic clock that advances a fixed step on every call.

    First call returns 0.0, so `elapsed` arithmetic in tests is predictable.
    """

    def __init__(self, step: float = 0.02):
        self.now = 0.0
        self.step = step

    def __call__(self) -> float:
        value = self.now
        self.now += self.step
        return value
```

- [ ] **Step 4: Write the failing tests**

Create `tests/test_timing.py`:

```python
import pytest

from spike.timing import StageTimer
from tests.fakes import FakeClock


def test_elapsed_ms_between_two_stages():
    timer = StageTimer(clock=FakeClock(step=0.25))
    timer.mark("t0")  # 0.00
    timer.mark("t1")  # 0.25
    assert timer.elapsed_ms("t0", "t1") == pytest.approx(250.0)


def test_elapsed_ms_skipping_a_stage():
    timer = StageTimer(clock=FakeClock(step=0.1))
    timer.mark("t0")  # 0.0
    timer.mark("t1")  # 0.1
    timer.mark("t2")  # 0.2
    assert timer.elapsed_ms("t0", "t2") == pytest.approx(200.0)


def test_elapsed_ms_raises_for_unknown_stage():
    timer = StageTimer(clock=FakeClock())
    timer.mark("t0")
    with pytest.raises(KeyError):
        timer.elapsed_ms("t0", "nope")


def test_report_lists_marked_stages_in_order():
    timer = StageTimer(clock=FakeClock(step=0.1))
    timer.mark("t0")
    timer.mark("t1")
    report = timer.report()
    assert "t0" in report
    assert "t1" in report
    assert report.index("t0") < report.index("t1")
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `.venv/Scripts/python -m pytest tests/test_timing.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'spike.timing'`

- [ ] **Step 6: Write the implementation**

Create `spike/timing.py`:

```python
import time
from typing import Callable


class StageTimer:
    """Records named timestamps and reports the gaps between them."""

    def __init__(self, clock: Callable[[], float] = time.perf_counter):
        self._clock = clock
        self._marks: dict[str, float] = {}
        self._order: list[str] = []

    def mark(self, stage: str) -> None:
        self._marks[stage] = self._clock()
        self._order.append(stage)

    def elapsed_ms(self, from_stage: str, to_stage: str) -> float:
        return (self._marks[to_stage] - self._marks[from_stage]) * 1000.0

    def report(self) -> str:
        if not self._order:
            return "(no stages recorded)"
        first = self._order[0]
        lines = []
        for stage in self._order:
            since_start = (self._marks[stage] - self._marks[first]) * 1000.0
            lines.append(f"  {stage:<28} {since_start:8.1f} ms")
        return "\n".join(lines)
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `.venv/Scripts/python -m pytest tests/test_timing.py -v`
Expected: PASS, 4 tests

- [ ] **Step 8: Commit**

```bash
git add requirements.txt spike/ tests/
git commit -m "feat: add StageTimer and spike project scaffold"
```

---

### Task 2: Clipboard module

**Files:**
- Create: `spike/clipboard.py`
- Test: `tests/test_clipboard.py`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `spike.clipboard.ClipboardTimeout` — exception
  - `spike.clipboard.read() -> str`
  - `spike.clipboard.write(text: str) -> None`
  - `spike.clipboard.clear() -> None`
  - `spike.clipboard.wait_for_content(timeout_ms: int = 1000, poll_ms: int = 20, reader: Callable[[], str] = read, clock: Callable[[], float] = time.perf_counter, sleep: Callable[[float], None] = time.sleep) -> str` — returns the first non-empty clipboard value seen, raises `ClipboardTimeout` if none appears in time

- [ ] **Step 1: Write the failing tests**

Create `tests/test_clipboard.py`:

```python
import pytest

from spike.clipboard import ClipboardTimeout, wait_for_content
from tests.fakes import FakeClock


def make_reader(values):
    """Returns a reader that yields each value once, then repeats the last."""
    remaining = list(values)

    def reader() -> str:
        if len(remaining) > 1:
            return remaining.pop(0)
        return remaining[0]

    return reader


def test_returns_content_once_it_appears():
    reader = make_reader(["", "", "hello world"])
    result = wait_for_content(
        timeout_ms=1000,
        poll_ms=20,
        reader=reader,
        clock=FakeClock(step=0.02),
        sleep=lambda _: None,
    )
    assert result == "hello world"


def test_returns_immediately_if_content_is_already_present():
    reader = make_reader(["already here"])
    result = wait_for_content(
        timeout_ms=1000,
        poll_ms=20,
        reader=reader,
        clock=FakeClock(step=0.02),
        sleep=lambda _: None,
    )
    assert result == "already here"


def test_raises_timeout_when_clipboard_stays_empty():
    with pytest.raises(ClipboardTimeout):
        wait_for_content(
            timeout_ms=1000,
            poll_ms=20,
            reader=lambda: "",
            clock=FakeClock(step=0.02),
            sleep=lambda _: None,
        )


def test_treats_whitespace_only_clipboard_as_empty():
    with pytest.raises(ClipboardTimeout):
        wait_for_content(
            timeout_ms=1000,
            poll_ms=20,
            reader=lambda: "   \n\t  ",
            clock=FakeClock(step=0.02),
            sleep=lambda _: None,
        )


def test_sleeps_between_polls():
    naps = []
    reader = make_reader(["", "", "text"])
    wait_for_content(
        timeout_ms=1000,
        poll_ms=20,
        reader=reader,
        clock=FakeClock(step=0.02),
        sleep=naps.append,
    )
    assert naps, "expected wait_for_content to sleep between polls"
    assert all(nap == pytest.approx(0.02) for nap in naps)
```

The whitespace test matters: a selection of only spaces is not text worth sending to a model, and treating it as content would send an empty prompt and paste the model's confused reply into the user's document.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/Scripts/python -m pytest tests/test_clipboard.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'spike.clipboard'`

- [ ] **Step 3: Write the implementation**

Create `spike/clipboard.py`:

```python
import time
from typing import Callable

import pyperclip


class ClipboardTimeout(Exception):
    """No clipboard content appeared before the timeout elapsed."""


def read() -> str:
    return pyperclip.paste()


def write(text: str) -> None:
    pyperclip.copy(text)


def clear() -> None:
    pyperclip.copy("")


def wait_for_content(
    timeout_ms: int = 1000,
    poll_ms: int = 20,
    reader: Callable[[], str] = read,
    clock: Callable[[], float] = time.perf_counter,
    sleep: Callable[[float], None] = time.sleep,
) -> str:
    """Poll the clipboard until it holds non-whitespace content.

    Raises ClipboardTimeout if nothing arrives within timeout_ms. Callers must
    treat that as "no selection" and abort without pasting.
    """
    deadline_s = timeout_ms / 1000.0
    start = clock()
    while True:
        value = reader()
        if value.strip():
            return value
        if clock() - start >= deadline_s:
            raise ClipboardTimeout(f"no clipboard content after {timeout_ms} ms")
        sleep(poll_ms / 1000.0)
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `.venv/Scripts/python -m pytest tests/test_clipboard.py -v`
Expected: PASS, 5 tests

- [ ] **Step 5: Commit**

```bash
git add spike/clipboard.py tests/test_clipboard.py
git commit -m "feat: add clipboard polling with timeout"
```

---

### Task 3: Keys module

**Files:**
- Create: `spike/keys.py`
- Test: `tests/test_keys.py`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `spike.keys.modifiers_down() -> bool` — Windows-only, true while Ctrl, Alt, or Shift is physically held
  - `spike.keys.wait_for_modifier_release(probe: Callable[[], bool] = modifiers_down, timeout_ms: int = 2000, poll_ms: int = 10, clock: Callable[[], float] = time.perf_counter, sleep: Callable[[float], None] = time.sleep) -> bool` — returns True if released in time, False on timeout
  - `spike.keys.send_copy() -> None`
  - `spike.keys.send_paste() -> None`

`modifiers_down()` uses `GetAsyncKeyState` through `ctypes` rather than pynput. pynput's hotkey listener does not expose current physical key state, and this check is the difference between the spike working and producing garbage. It is Windows-only; the production app will need a per-OS implementation, and that is a known cost recorded in the results note.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_keys.py`:

```python
from spike.keys import wait_for_modifier_release
from tests.fakes import FakeClock


def make_probe(values):
    """Probe that yields each value once, then repeats the last."""
    remaining = list(values)

    def probe() -> bool:
        if len(remaining) > 1:
            return remaining.pop(0)
        return remaining[0]

    return probe


def test_returns_true_once_modifiers_are_released():
    probe = make_probe([True, True, False])
    assert wait_for_modifier_release(
        probe=probe,
        timeout_ms=2000,
        poll_ms=10,
        clock=FakeClock(step=0.01),
        sleep=lambda _: None,
    ) is True


def test_returns_true_immediately_when_nothing_is_held():
    assert wait_for_modifier_release(
        probe=lambda: False,
        timeout_ms=2000,
        poll_ms=10,
        clock=FakeClock(step=0.01),
        sleep=lambda _: None,
    ) is True


def test_returns_false_when_modifiers_are_never_released():
    assert wait_for_modifier_release(
        probe=lambda: True,
        timeout_ms=2000,
        poll_ms=10,
        clock=FakeClock(step=0.01),
        sleep=lambda _: None,
    ) is False
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/Scripts/python -m pytest tests/test_keys.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'spike.keys'`

- [ ] **Step 3: Write the implementation**

Create `spike/keys.py`:

```python
import ctypes
import time
from typing import Callable

from pynput.keyboard import Controller, Key

_VK_SHIFT = 0x10
_VK_CONTROL = 0x11
_VK_MENU = 0x12  # Alt

_keyboard = Controller()


def modifiers_down() -> bool:
    """True while Ctrl, Alt, or Shift is physically held. Windows only."""
    get_state = ctypes.windll.user32.GetAsyncKeyState
    return any(get_state(vk) & 0x8000 for vk in (_VK_SHIFT, _VK_CONTROL, _VK_MENU))


def wait_for_modifier_release(
    probe: Callable[[], bool] = modifiers_down,
    timeout_ms: int = 2000,
    poll_ms: int = 10,
    clock: Callable[[], float] = time.perf_counter,
    sleep: Callable[[float], None] = time.sleep,
) -> bool:
    """Block until no modifier is held. Returns False if that never happens."""
    deadline_s = timeout_ms / 1000.0
    start = clock()
    while True:
        if not probe():
            return True
        if clock() - start >= deadline_s:
            return False
        sleep(poll_ms / 1000.0)


def _send_with_ctrl(letter: str) -> None:
    with _keyboard.pressed(Key.ctrl):
        _keyboard.press(letter)
        _keyboard.release(letter)


def send_copy() -> None:
    _send_with_ctrl("c")


def send_paste() -> None:
    _send_with_ctrl("v")
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `.venv/Scripts/python -m pytest tests/test_keys.py -v`
Expected: PASS, 3 tests

- [ ] **Step 5: Manually verify synthetic keystrokes reach another application**

`send_copy` and `send_paste` cannot be unit tested — they only mean anything against a real focused window. Verify by hand:

1. Open Notepad, type `hello`, and select it with the mouse.
2. In a second terminal run:

```bash
.venv/Scripts/python -c "import time; from spike import clipboard, keys; time.sleep(3); keys.send_copy(); time.sleep(0.3); print(repr(clipboard.read()))"
```

3. Click into Notepad within the 3-second window so it has focus.

Expected: the terminal prints `'hello'`. If it prints the previous clipboard contents instead, the 0.3 s wait was too short — that is the clipboard race the spike exists to measure, and Task 7 handles it properly with `wait_for_content`. Note what you observe; it is data.

- [ ] **Step 6: Commit**

```bash
git add spike/keys.py tests/test_keys.py
git commit -m "feat: add modifier-release wait and synthetic copy/paste"
```

---

### Task 4: Ollama backend

**Files:**
- Create: `spike/backends.py`
- Test: `tests/test_backends.py`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `spike.backends.SYSTEM_PROMPT: str`
  - `spike.backends.BackendRefusal` — exception raised when a backend produces nothing usable
  - `spike.backends.strip_think_blocks(text: str) -> str`
  - `spike.backends.OllamaBackend(base_url="http://localhost:11434/v1", model="qwen2.5:32b-instruct", http_post=None)` with `.name: str` and `.transform(text: str) -> str`

This targets Ollama's **OpenAI-compatible** endpoint rather than its native `/api/chat`, because the production app needs an "any OpenAI-compatible endpoint" adapter anyway — writing it against that shape now means the spike produces something reusable rather than throwaway-only. Use plain `httpx` here, not an Anthropic SDK: this is deliberately a non-Anthropic provider.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_backends.py`:

```python
import pytest

from spike.backends import SYSTEM_PROMPT, BackendRefusal, OllamaBackend


def chat_response(content: str) -> dict:
    return {"choices": [{"message": {"role": "assistant", "content": content}}]}


def test_returns_the_message_content():
    backend = OllamaBackend(http_post=lambda url, payload: chat_response("Corrected."))
    assert backend.transform("corected") == "Corrected."


def test_strips_think_blocks():
    raw = "<think>The user wants grammar fixed.</think>Corrected."
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    assert backend.transform("corected") == "Corrected."


def test_raises_when_response_has_no_choices():
    backend = OllamaBackend(http_post=lambda url, payload: {"choices": []})
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_raises_when_content_is_only_a_think_block():
    raw = "<think>thinking out loud</think>   "
    backend = OllamaBackend(http_post=lambda url, payload: chat_response(raw))
    with pytest.raises(BackendRefusal):
        backend.transform("x")


def test_sends_the_expected_payload():
    seen = {}

    def http_post(url, payload):
        seen["url"] = url
        seen["payload"] = payload
        return chat_response("ok")

    OllamaBackend(http_post=http_post).transform("input text")
    assert seen["url"] == "http://localhost:11434/v1/chat/completions"
    assert seen["payload"]["model"] == "qwen2.5:32b-instruct"
    assert seen["payload"]["stream"] is False
    assert seen["payload"]["messages"] == [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": "input text"},
    ]
```

The `<think>`-stripping tests are load-bearing, not defensive padding. Two of the three models installed locally (`qwen3:30b`, `qwen3-coder:30b`) are reasoning models that emit `<think>` blocks in their output, and this text goes directly into the user's document. The fourth test covers the case where stripping leaves nothing behind — that must raise rather than paste an empty string over the selection.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/Scripts/python -m pytest tests/test_backends.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'spike.backends'`

- [ ] **Step 3: Write the implementation**

Create `spike/backends.py`:

```python
import re

import httpx

SYSTEM_PROMPT = (
    "Fix grammar and spelling. Return only the corrected text, "
    "with no preamble, explanation, or commentary."
)

_THINK_BLOCK = re.compile(r"<think>.*?</think>", re.DOTALL | re.IGNORECASE)


class BackendRefusal(Exception):
    """The backend produced nothing usable. Never paste after this."""


def strip_think_blocks(text: str) -> str:
    """Remove <think>...</think> spans emitted by reasoning models."""
    return _THINK_BLOCK.sub("", text)


def _default_http_post(url: str, payload: dict) -> dict:
    response = httpx.post(url, json=payload, timeout=120.0)
    response.raise_for_status()
    return response.json()


class OllamaBackend:
    """Local Ollama through its OpenAI-compatible chat completions endpoint."""

    name = "ollama"

    def __init__(
        self,
        base_url: str = "http://localhost:11434/v1",
        model: str = "qwen2.5:32b-instruct",
        http_post=None,
    ):
        self._url = f"{base_url.rstrip('/')}/chat/completions"
        self._model = model
        self._http_post = http_post or _default_http_post

    def transform(self, text: str) -> str:
        payload = self._http_post(
            self._url,
            {
                "model": self._model,
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": text},
                ],
                "stream": False,
            },
        )
        choices = payload.get("choices") or []
        if not choices:
            raise BackendRefusal(f"ollama returned no choices: {payload}")
        content = strip_think_blocks(choices[0].get("message", {}).get("content", ""))
        if not content.strip():
            raise BackendRefusal("ollama returned empty content")
        return content.strip()
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `.venv/Scripts/python -m pytest tests/test_backends.py -v`
Expected: PASS, 5 tests

- [ ] **Step 5: Verify against the running Ollama server**

Ollama is already running locally with `qwen2.5:32b-instruct` available.

```bash
.venv/Scripts/python -c "import time; from spike.backends import OllamaBackend; s=time.perf_counter(); print(OllamaBackend().transform('this sentance have two mistake')); print(f'{(time.perf_counter()-s)*1000:.0f} ms')"
```

Expected: a corrected sentence, nothing else, and a millisecond figure. Record two numbers — the first call after the model has been idle (Ollama unloads models after a few minutes, so this includes model load time) and a second call immediately after (model resident). That cold/warm gap is a real latency factor for a hotkey tool and belongs in the results note.

If the output contains a preamble like "Here is the corrected text:", or any leftover `<think>` markup, record it — that is a finding, not something to silently patch around.

- [ ] **Step 6: Commit**

```bash
git add spike/backends.py tests/test_backends.py
git commit -m "feat: add Ollama backend with think-block stripping"
```

---

### Task 5: Claude CLI backend

**Files:**
- Modify: `spike/backends.py`
- Modify: `tests/test_backends.py`

**Interfaces:**
- Consumes: `spike.backends.SYSTEM_PROMPT`
- Produces: `spike.backends.ClaudeCLIBackend(runner=None)` with `.name: str` and `.transform(text: str) -> str`

This is the baseline the Agent SDK gets compared against — a cold `claude -p` subprocess per request. It is expected to be the slowest of the three, and quantifying by how much is one of the spike's three questions.

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_backends.py`:

```python
from spike.backends import ClaudeCLIBackend


def test_cli_backend_invokes_claude_p_with_the_prompt():
    calls = []

    def runner(argv, input_text):
        calls.append((argv, input_text))
        return "Corrected.\n"

    assert ClaudeCLIBackend(runner=runner).transform("corected") == "Corrected."
    argv, input_text = calls[0]
    assert argv[0] == "claude"
    assert "-p" in argv
    assert "corected" in input_text


def test_cli_backend_raises_when_the_cli_returns_nothing():
    with pytest.raises(BackendRefusal):
        ClaudeCLIBackend(runner=lambda argv, input_text: "   ").transform("x")
```

Empty CLI output raises rather than returning `""`, for the same reason as the refusal case: an empty result must never reach the paste step.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/Scripts/python -m pytest tests/test_backends.py -v`
Expected: FAIL with `ImportError: cannot import name 'ClaudeCLIBackend'`

- [ ] **Step 3: Write the implementation**

Add to `spike/backends.py`:

```python
import subprocess


def _run_claude_cli(argv: list[str], input_text: str) -> str:
    result = subprocess.run(
        argv,
        input=input_text,
        capture_output=True,
        text=True,
        timeout=120,
    )
    if result.returncode != 0:
        raise BackendRefusal(f"claude exited {result.returncode}: {result.stderr}")
    return result.stdout


class ClaudeCLIBackend:
    """Cold `claude -p` subprocess per request. The latency baseline."""

    name = "claude-cli"

    def __init__(self, runner=None):
        self._runner = runner or _run_claude_cli

    def transform(self, text: str) -> str:
        output = self._runner(
            ["claude", "-p", "--system-prompt", SYSTEM_PROMPT],
            text,
        )
        if not output.strip():
            raise BackendRefusal("claude returned no output")
        return output.strip()
```

No `shell=True`. On this machine `claude` resolves to a real executable (`C:\Users\Work\.local\bin\claude.exe`), so `subprocess` finds it directly. If `FileNotFoundError` occurs anyway, pass the absolute path to the executable rather than reaching for `shell=True`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `.venv/Scripts/python -m pytest tests/test_backends.py -v`
Expected: PASS, 7 tests

- [ ] **Step 5: Verify against the real CLI**

```bash
.venv/Scripts/python -c "import time; from spike.backends import ClaudeCLIBackend; s=time.perf_counter(); print(ClaudeCLIBackend().transform('this sentance have two mistake')); print(f'{(time.perf_counter()-s)*1000:.0f} ms')"
```

Record the millisecond figure. This is the cold-start number the Agent SDK has to beat.

- [ ] **Step 6: Commit**

```bash
git add spike/backends.py tests/test_backends.py
git commit -m "feat: add claude -p CLI backend as latency baseline"
```

---

### Task 6: Agent SDK backend

**Files:**
- Modify: `spike/backends.py`
- Modify: `tests/test_backends.py`

**Interfaces:**
- Consumes: `spike.backends.SYSTEM_PROMPT`
- Produces: `spike.backends.AgentSDKBackend(run_query=None)` with `.name: str` and `.transform(text: str) -> str`, where `run_query` has the signature `(system_prompt: str, user_text: str) -> str`

**Do not write the SDK call from memory.** The Claude Agent SDK is a different library from the `anthropic` Messages API package, with a different API shape. Fetch its documentation before implementing, and do not infer method names, parameter names, or return types from the Messages API. If the docs are unreachable, stop and report rather than guessing — an invented API surface is worse than an unfinished task, because it fails at runtime in a way that looks like an auth problem.

The design isolates this risk: `AgentSDKBackend` takes an injectable `run_query` callable, so our adapter contract is fully tested regardless, and only the small `_default_run_query` function depends on the real SDK.

- [ ] **Step 1: Read the Agent SDK documentation**

Fetch `https://code.claude.com/docs/en/agent-sdk` and find:

1. The Python entry point for a single-shot query and its exact import path
2. How to pass a system prompt
3. How to extract assistant text from the result
4. Whether the entry point is async, and whether a persistent-session client exists

Write down the answers before proceeding — Step 4 depends on all four.

- [ ] **Step 2: Write the failing tests**

Append to `tests/test_backends.py`:

```python
from spike.backends import SYSTEM_PROMPT, AgentSDKBackend


def test_agent_sdk_backend_passes_prompt_and_text_to_run_query():
    calls = []

    def run_query(system_prompt, user_text):
        calls.append((system_prompt, user_text))
        return "Corrected.\n"

    assert AgentSDKBackend(run_query=run_query).transform("corected") == "Corrected."
    assert calls == [(SYSTEM_PROMPT, "corected")]


def test_agent_sdk_backend_raises_on_empty_output():
    with pytest.raises(BackendRefusal):
        AgentSDKBackend(run_query=lambda s, t: "").transform("x")
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `.venv/Scripts/python -m pytest tests/test_backends.py -v`
Expected: FAIL with `ImportError: cannot import name 'AgentSDKBackend'`

- [ ] **Step 4: Write the implementation**

Add to `spike/backends.py`. The `AgentSDKBackend` class below is fixed — copy it verbatim. `_default_run_query` is the only part that depends on the real SDK: **write its body now**, using the four answers recorded in Step 1.

Its contract, which the rest of the module depends on and which must not change:

- Signature exactly `(system_prompt: str, user_text: str) -> str`
- Synchronous. If the SDK entry point is async, wrap the call in `asyncio.run()` inside this function so the signature stays sync.
- Returns the assistant's reply as a plain string. If the SDK yields structured messages or a stream, collect the assistant text and join it here.
- Raises on transport or auth failure — do not catch and return `""`, because an empty string reaching the caller is indistinguishable from a legitimate empty reply, and Task 7 relies on that distinction to decide whether to paste.

```python
def _default_run_query(system_prompt: str, user_text: str) -> str:
    """Single-shot query through the Claude Agent SDK.

    Uses the existing Claude Code login rather than an API key.
    Written against https://code.claude.com/docs/en/agent-sdk — see Step 1.
    """
    # Body goes here, per the contract above and the Step 1 notes.
    # Do not infer the SDK's API from the anthropic Messages API package —
    # they are different libraries with different shapes.


class AgentSDKBackend:
    """Claude Agent SDK, authenticating off the existing Claude Code login."""

    name = "agent-sdk"

    def __init__(self, run_query=None):
        self._run_query = run_query or _default_run_query

    def transform(self, text: str) -> str:
        output = self._run_query(SYSTEM_PROMPT, text)
        if not output.strip():
            raise BackendRefusal("agent sdk returned no output")
        return output.strip()
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `.venv/Scripts/python -m pytest tests/test_backends.py -v`
Expected: PASS, 9 tests

Note that these tests inject `run_query` and therefore never exercise `_default_run_query`. They prove the adapter contract, not the SDK integration. Step 6 is the only thing that proves the real path works — do not treat a green suite here as evidence that the subscription backend functions.

- [ ] **Step 6: Verify against the real SDK and answer the subscription question**

Ensure you are logged into Claude Code and that `ANTHROPIC_API_KEY` is **unset** for this check — a set API key takes precedence over the subscription login, which would make the test pass for the wrong reason.

```bash
.venv/Scripts/python -c "import time; from spike.backends import AgentSDKBackend; b=AgentSDKBackend(); [print(f'{n}: {(lambda s: (b.transform(\"this sentance have two mistake\"), (time.perf_counter()-s)*1000))(time.perf_counter())[1]:.0f} ms') for n in ('cold','warm-1','warm-2')]"
```

Record three numbers: cold, and two subsequent warm calls. Compare against the `claude -p` figure from Task 5.

This step answers the spike's third question. Three outcomes, all useful:

- **Authenticates and warm calls are meaningfully faster than `claude -p`** — the subscription backend is viable, and the production design should keep a session warm.
- **Authenticates but every call costs the same as `claude -p`** — the SDK spawns per call in this usage; note it and check whether a persistent-session client exists.
- **Does not authenticate off the Claude Code login** — the subscription backend is not viable as designed. This is a significant finding; record the exact error and stop rather than working around it.

- [ ] **Step 7: Commit**

```bash
git add spike/backends.py tests/test_backends.py
git commit -m "feat: add Agent SDK backend using Claude Code login"
```

---

### Task 7: Cycle orchestration

**Files:**
- Create: `spike/cycle.py`
- Test: `tests/test_cycle.py`

**Interfaces:**
- Consumes: `spike.clipboard.ClipboardTimeout`, `spike.backends.BackendRefusal`, `spike.timing.StageTimer`
- Produces:
  - `spike.cycle.CycleResult(pasted: bool, reason: str, timer: StageTimer)` — a dataclass
  - `spike.cycle.run_cycle(backend, timer, deps: CycleDeps) -> CycleResult`
  - `spike.cycle.CycleDeps` — dataclass bundling the injectable OS operations: `read_clipboard`, `write_clipboard`, `clear_clipboard`, `wait_for_content`, `wait_for_modifier_release`, `send_copy`, `send_paste`, `sleep`

This is the most important task in the plan. The two abort rules from the spec live here, and these tests are the only thing standing between the spike and pasting wrong text into someone's email.

- [ ] **Step 1: Write the failing tests**

Create `tests/test_cycle.py`:

```python
import pytest

from spike.backends import BackendRefusal
from spike.clipboard import ClipboardTimeout
from spike.cycle import CycleDeps, run_cycle
from spike.timing import StageTimer
from tests.fakes import FakeClock


class FakeOS:
    """Records clipboard and keystroke effects so tests can assert on them."""

    def __init__(self, original="ORIGINAL", captured="corected text",
                 capture_raises=False, modifiers_release=True):
        self.clipboard = original
        self._captured = captured
        self._capture_raises = capture_raises
        self._modifiers_release = modifiers_release
        self.pastes = 0
        self.copies = 0

    def read_clipboard(self):
        return self.clipboard

    def write_clipboard(self, text):
        self.clipboard = text

    def clear_clipboard(self):
        self.clipboard = ""

    def wait_for_content(self):
        if self._capture_raises:
            raise ClipboardTimeout("nothing selected")
        self.clipboard = self._captured
        return self._captured

    def wait_for_modifier_release(self):
        return self._modifiers_release

    def send_copy(self):
        self.copies += 1

    def send_paste(self):
        self.pastes += 1

    def as_deps(self):
        return CycleDeps(
            read_clipboard=self.read_clipboard,
            write_clipboard=self.write_clipboard,
            clear_clipboard=self.clear_clipboard,
            wait_for_content=self.wait_for_content,
            wait_for_modifier_release=self.wait_for_modifier_release,
            send_copy=self.send_copy,
            send_paste=self.send_paste,
            sleep=lambda _: None,
        )


class StubBackend:
    name = "stub"

    def __init__(self, result="Corrected text.", raises=None):
        self._result = result
        self._raises = raises
        self.seen = None

    def transform(self, text):
        self.seen = text
        if self._raises:
            raise self._raises
        return self._result


def new_timer():
    return StageTimer(clock=FakeClock(step=0.01))


def test_happy_path_pastes_the_transformed_text():
    os_ = FakeOS()
    backend = StubBackend("Corrected text.")
    result = run_cycle(backend, new_timer(), os_.as_deps())
    assert result.pasted is True
    assert backend.seen == "corected text"
    assert os_.pastes == 1


def test_happy_path_restores_the_original_clipboard():
    os_ = FakeOS(original="ORIGINAL")
    run_cycle(StubBackend(), new_timer(), os_.as_deps())
    assert os_.clipboard == "ORIGINAL"


def test_capture_timeout_never_pastes():
    os_ = FakeOS(capture_raises=True)
    result = run_cycle(StubBackend(), new_timer(), os_.as_deps())
    assert result.pasted is False
    assert result.reason == "capture-timeout"
    assert os_.pastes == 0


def test_capture_timeout_restores_the_original_clipboard():
    os_ = FakeOS(original="ORIGINAL", capture_raises=True)
    run_cycle(StubBackend(), new_timer(), os_.as_deps())
    assert os_.clipboard == "ORIGINAL"


def test_backend_refusal_never_pastes():
    os_ = FakeOS()
    backend = StubBackend(raises=BackendRefusal("declined"))
    result = run_cycle(backend, new_timer(), os_.as_deps())
    assert result.pasted is False
    assert result.reason == "backend-refusal"
    assert os_.pastes == 0


def test_backend_refusal_restores_the_original_clipboard():
    os_ = FakeOS(original="ORIGINAL")
    backend = StubBackend(raises=BackendRefusal("declined"))
    run_cycle(backend, new_timer(), os_.as_deps())
    assert os_.clipboard == "ORIGINAL"


def test_stuck_modifiers_abort_before_sending_copy():
    os_ = FakeOS(modifiers_release=False)
    result = run_cycle(StubBackend(), new_timer(), os_.as_deps())
    assert result.pasted is False
    assert result.reason == "modifiers-stuck"
    assert os_.copies == 0
    assert os_.pastes == 0


def test_records_all_timing_stages_on_the_happy_path():
    os_ = FakeOS()
    result = run_cycle(StubBackend(), new_timer(), os_.as_deps())
    report = result.timer.report()
    for stage in ("t0-hotkey", "t1-modifiers-released", "t2-copy-sent",
                  "t3-clipboard-captured", "t4-backend-done",
                  "t5-paste-sent", "t6-clipboard-restored"):
        assert stage in report
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `.venv/Scripts/python -m pytest tests/test_cycle.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'spike.cycle'`

- [ ] **Step 3: Write the implementation**

Create `spike/cycle.py`:

```python
from dataclasses import dataclass
from typing import Callable

from spike.backends import BackendRefusal
from spike.clipboard import ClipboardTimeout
from spike.timing import StageTimer

RESTORE_DELAY_S = 0.3


@dataclass
class CycleDeps:
    read_clipboard: Callable[[], str]
    write_clipboard: Callable[[str], None]
    clear_clipboard: Callable[[], None]
    wait_for_content: Callable[[], str]
    wait_for_modifier_release: Callable[[], bool]
    send_copy: Callable[[], None]
    send_paste: Callable[[], None]
    sleep: Callable[[float], None]


@dataclass
class CycleResult:
    pasted: bool
    reason: str
    timer: StageTimer


def run_cycle(backend, timer: StageTimer, deps: CycleDeps) -> CycleResult:
    """One hotkey press, end to end.

    Every abort path restores the original clipboard and pastes nothing.
    """
    timer.mark("t0-hotkey")
    original = deps.read_clipboard()

    def abort(reason: str) -> CycleResult:
        deps.write_clipboard(original)
        return CycleResult(pasted=False, reason=reason, timer=timer)

    if not deps.wait_for_modifier_release():
        return abort("modifiers-stuck")
    timer.mark("t1-modifiers-released")

    deps.clear_clipboard()
    deps.send_copy()
    timer.mark("t2-copy-sent")

    try:
        captured = deps.wait_for_content()
    except ClipboardTimeout:
        return abort("capture-timeout")
    timer.mark("t3-clipboard-captured")

    try:
        transformed = backend.transform(captured)
    except BackendRefusal:
        return abort("backend-refusal")
    timer.mark("t4-backend-done")

    if not transformed.strip():
        return abort("empty-result")

    deps.write_clipboard(transformed)
    deps.send_paste()
    timer.mark("t5-paste-sent")

    deps.sleep(RESTORE_DELAY_S)
    deps.write_clipboard(original)
    timer.mark("t6-clipboard-restored")

    return CycleResult(pasted=True, reason="ok", timer=timer)
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `.venv/Scripts/python -m pytest tests/test_cycle.py -v`
Expected: PASS, 8 tests

- [ ] **Step 5: Run the whole suite**

Run: `.venv/Scripts/python -m pytest -v`
Expected: PASS, 29 tests

- [ ] **Step 6: Commit**

```bash
git add spike/cycle.py tests/test_cycle.py
git commit -m "feat: add cycle orchestration with abort-without-pasting guarantees"
```

---

### Task 8: Hotkey entry point

**Files:**
- Create: `spike/main.py`

**Interfaces:**
- Consumes: everything from Tasks 1–7
- Produces: `python -m spike.main [--backend ollama|agent-sdk|claude-cli]`

No unit tests. This module is the untestable shell — global hotkey registration and console printing — and keeping it that thin is why `cycle.py` exists separately.

- [ ] **Step 1: Write the implementation**

Create `spike/main.py`:

```python
import argparse
import time

from pynput import keyboard

from spike import clipboard, keys
from spike.backends import AgentSDKBackend, ClaudeCLIBackend, OllamaBackend
from spike.cycle import CycleDeps, run_cycle
from spike.timing import StageTimer

HOTKEY = "<ctrl>+<alt>+j"

BACKENDS = {
    "ollama": OllamaBackend,
    "agent-sdk": AgentSDKBackend,
    "claude-cli": ClaudeCLIBackend,
}


def build_deps() -> CycleDeps:
    return CycleDeps(
        read_clipboard=clipboard.read,
        write_clipboard=clipboard.write,
        clear_clipboard=clipboard.clear,
        wait_for_content=lambda: clipboard.wait_for_content(
            timeout_ms=1000, poll_ms=20
        ),
        wait_for_modifier_release=lambda: keys.wait_for_modifier_release(
            timeout_ms=2000, poll_ms=10
        ),
        send_copy=keys.send_copy,
        send_paste=keys.send_paste,
        sleep=time.sleep,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Clipboard AI capture/paste spike")
    parser.add_argument("--backend", choices=sorted(BACKENDS), default="ollama")
    args = parser.parse_args()

    backend = BACKENDS[args.backend]()
    deps = build_deps()

    def on_hotkey() -> None:
        result = run_cycle(backend, StageTimer(), deps)
        status = "PASTED" if result.pasted else f"ABORTED ({result.reason})"
        print(f"\n[{backend.name}] {status}")
        print(result.timer.report())
        if result.pasted:
            print(
                f"  capture latency (t2 to t3): "
                f"{result.timer.elapsed_ms('t2-copy-sent', 't3-clipboard-captured'):.1f} ms"
            )
            print(
                f"  backend latency (t3 to t4): "
                f"{result.timer.elapsed_ms('t3-clipboard-captured', 't4-backend-done'):.1f} ms"
            )

    print(f"Backend: {backend.name}")
    print("Select text anywhere, then press Ctrl+Alt+J. Ctrl+C here to quit.")
    with keyboard.GlobalHotKeys({HOTKEY: on_hotkey}) as listener:
        listener.join()


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Verify the hotkey fires and the abort path is safe**

Run it, then press the hotkey with nothing selected:

```bash
.venv/Scripts/python -m spike.main --backend ollama
```

Click into Notepad without selecting anything, press `Ctrl+Alt+J`.

Expected: the console prints `ABORTED (capture-timeout)`, nothing is pasted into Notepad, and your clipboard still holds whatever it held before. If anything gets pasted here, stop — that is the failure mode the whole spike is guarding against, and it needs fixing before any further testing.

- [ ] **Step 3: Verify the happy path in Notepad**

Type `this sentance have two mistake` in Notepad, select it, press `Ctrl+Alt+J`.

Expected: the selection is replaced with a corrected sentence, the console prints `PASTED` with a timing breakdown, and the clipboard is back to its prior contents.

- [ ] **Step 4: Commit**

```bash
git add spike/main.py
git commit -m "feat: add hotkey entry point and timing report"
```

---

### Task 9: Run the test matrix and write the results note

**Files:**
- Create: `docs/superpowers/notes/2026-08-06-spike-results.md`

**Interfaces:**
- Consumes: the working spike from Tasks 1–8
- Produces: the results note that the production design will be built on

This task produces the spike's actual deliverable. Tasks 1–8 built the measuring instrument; this is the measurement.

- [ ] **Step 1: Run the matrix**

For each application below, run 10 consecutive attempts with `--backend ollama`. Ollama is the right backend for this pass because it is local and needs no auth, which keeps the matrix measuring clipboard mechanics rather than network variance. Record successes, failures, and the failure mode of each failure.

- Notepad (control — if this fails, the mechanism is broken, not the app)
- Outlook
- Teams
- Browser: a plain `<textarea>`
- Browser: a rich `contenteditable` (Gmail compose)
- VS Code
- Word

For each application record the median capture latency (`t2 to t3`) from the console output. That number is the primary result.

- [ ] **Step 2: Record the backend comparison**

Fill in the four latency figures gathered along the way:

- Ollama, model cold and model resident (Task 4, Step 5)
- `claude -p` cold (Task 5, Step 5)
- Agent SDK cold (Task 6, Step 6)
- Agent SDK warm (Task 6, Step 6)

- [ ] **Step 3: Write the results note**

Create `docs/superpowers/notes/2026-08-06-spike-results.md` with these sections:

1. **Application matrix** — a table of app, successes out of 10, median capture latency, and failure modes observed
2. **Backend latency** — the four figures, and whether the Agent SDK authenticated off the Claude Code login
3. **The three questions** — a direct answer to each of the spike's three questions
4. **Recommendation** — is in-place replacement viable, or does the production app need a different interaction model such as a floating result window? If the answer is mixed, say which applications each model would serve.
5. **Carried-forward costs** — things the production app will have to solve that the spike sidestepped. Include at minimum: `modifiers_down()` is Windows-only and needs a per-OS implementation for macOS and Linux; the Anthropic Messages API backend was never built or measured, so that comparison point is still missing; `<think>`-block stripping is currently applied only in the Ollama backend and will need to cover any provider whose models emit reasoning markup.

Write down negative results plainly. Learning that in-place replacement is unreliable in Outlook is worth more than a spike that reports success everywhere.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/notes/2026-08-06-spike-results.md
git commit -m "docs: add capture/paste spike results"
```

---

## After this plan

The spike is throwaway. Once the results note is committed, the `spike/` directory has served its purpose — keep it around only as a reference while the production design is written, then delete it.

The next step is a fresh brainstorm for the production application, informed by the results note. That design decides the stack (Python end-to-end, Electron, or Tauri plus sidecar), the backend/frontend split over localhost, the prompt configuration format, the per-prompt write-back mode, and the macOS and Linux implementations.
