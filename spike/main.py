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
