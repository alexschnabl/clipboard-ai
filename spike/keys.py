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


def get_foreground_window() -> int:
    """Handle of the window currently holding focus. Windows only.

    Used to detect focus loss between capture and paste-back: the value at
    hotkey time is compared against the value right before Ctrl+V is sent.
    """
    return ctypes.windll.user32.GetForegroundWindow()


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
