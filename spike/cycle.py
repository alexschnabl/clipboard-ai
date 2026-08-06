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
