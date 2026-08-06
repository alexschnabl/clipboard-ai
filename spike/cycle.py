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
    get_foreground_window: Callable[[], object]


@dataclass
class CycleResult:
    pasted: bool
    reason: str
    timer: StageTimer


def run_cycle(backend, timer: StageTimer, deps: CycleDeps) -> CycleResult:
    """One hotkey press, end to end.

    Every abort path restores the original clipboard and pastes nothing.
    A `finally` guarantees that restore even when an exception other than
    `ClipboardTimeout` or `BackendRefusal` escapes an OS call or the
    backend; the exception is not swallowed, it still propagates.

    Between capture (t2) and paste, the backend call can take anywhere from
    tens of milliseconds to tens of seconds. Two things the user did at
    capture time can silently stop being true by the time paste fires:
    the window they meant to paste into can lose focus, and a modifier key
    can be back down (e.g. mid-shortcut for something else). Both are
    re-checked immediately before send_paste and abort rather than paste
    into the wrong place or trigger the wrong paste variant.
    """
    timer.mark("t0-hotkey")
    original = deps.read_clipboard()
    restored = False

    def abort(reason: str) -> CycleResult:
        nonlocal restored
        deps.write_clipboard(original)
        restored = True
        return CycleResult(pasted=False, reason=reason, timer=timer)

    try:
        if not deps.wait_for_modifier_release():
            return abort("modifiers-stuck")
        timer.mark("t1-modifiers-released")

        deps.clear_clipboard()
        deps.send_copy()
        timer.mark("t2-copy-sent")
        focus_at_copy = deps.get_foreground_window()

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

        if deps.get_foreground_window() != focus_at_copy:
            return abort("focus-changed")

        if not deps.wait_for_modifier_release():
            return abort("modifiers-stuck-before-paste")
        timer.mark("t4a-preflight-passed")

        deps.write_clipboard(transformed)
        deps.send_paste()
        timer.mark("t5-paste-sent")

        deps.sleep(RESTORE_DELAY_S)
        deps.write_clipboard(original)
        restored = True
        timer.mark("t6-clipboard-restored")

        return CycleResult(pasted=True, reason="ok", timer=timer)
    finally:
        if not restored:
            deps.write_clipboard(original)
