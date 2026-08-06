import argparse
import threading
import time
import traceback

from pynput import keyboard

from spike import clipboard, keys
from spike.backends import (
    AgentSDKBackend,
    AgentSDKSessionBackend,
    ClaudeCLIBackend,
    OllamaBackend,
)
from spike.cycle import CycleDeps, run_cycle
from spike.timing import StageTimer

HOTKEY = "<ctrl>+<alt>+j"

BACKENDS = {
    "ollama": OllamaBackend,
    "agent-sdk": AgentSDKBackend,
    "agent-sdk-session": AgentSDKSessionBackend,
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
        get_foreground_window=keys.get_foreground_window,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Clipboard AI capture/paste spike")
    parser.add_argument("--backend", choices=sorted(BACKENDS), default="ollama")
    args = parser.parse_args()

    backend = BACKENDS[args.backend]()
    deps = build_deps()

    # A WH_KEYBOARD_LL hook is only serviceable while its installing thread
    # (this one, pynput's listener thread) is pumping messages. run_cycle can
    # take tens of seconds (cold model load), so it must not run inline here
    # -- that would stall the message pump for the whole cycle, and Windows
    # silently proceeds without the hook (or removes it) after
    # LowLevelHooksTimeout (default 300 ms), corrupting the very latency
    # numbers this spike exists to produce. Each press instead hands off to a
    # daemon thread immediately; `busy` is a non-blocking re-entrancy guard so
    # a second press while a cycle is in flight is ignored (with a message)
    # instead of starting a second concurrent cycle.
    busy = threading.Lock()

    def run_and_report() -> None:
        try:
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
        except Exception:
            # Containment, not a new safety invariant: run_cycle's own
            # finally already restores the clipboard and nothing is pasted
            # before this point on any aborted path. Without this, an
            # exception here (e.g. pyperclip's PyperclipWindowsException from
            # a transient clipboard lock -- routine with Outlook/Word/Teams
            # or a clipboard manager running) would propagate out of this
            # background thread and simply be swallowed by Python's default
            # thread excepthook, so it would look like the hotkey silently
            # stopped working partway through a run. Printing the traceback
            # here instead surfaces it to the operator and keeps the
            # listener (and any later hotkey press) alive.
            print("\n[unexpected error during cycle]")
            print(traceback.format_exc())
        finally:
            busy.release()

    def on_hotkey() -> None:
        if not busy.acquire(blocking=False):
            print("\nbusy; hotkey ignored (a cycle is already in flight)")
            return
        try:
            threading.Thread(target=run_and_report, daemon=True).start()
        except Exception:
            busy.release()
            print("\n[unexpected error starting cycle thread]")
            print(traceback.format_exc())

    print(f"Backend: {backend.name}")
    print("Select text anywhere, then press Ctrl+Alt+J. Ctrl+C here to quit.")
    with keyboard.GlobalHotKeys({HOTKEY: on_hotkey}) as listener:
        listener.join()


if __name__ == "__main__":
    main()
