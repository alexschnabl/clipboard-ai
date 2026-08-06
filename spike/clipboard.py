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
