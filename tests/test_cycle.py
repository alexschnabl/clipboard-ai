import pytest

from spike.backends import BackendRefusal
from spike.clipboard import ClipboardTimeout
from spike.cycle import CycleDeps, run_cycle
from spike.timing import StageTimer
from tests.fakes import FakeClock


class FakeOS:
    """Records clipboard, keystroke, and OS-probe effects so tests can assert on them.

    `calls` is an ordered log of every operation performed, in the order
    run_cycle performed them -- this is what lets tests pin the safety-critical
    orderings (clear-before-copy, restore-after-paste, and so on) instead of
    only checking end states.
    """

    def __init__(self, original="ORIGINAL", captured="corected text",
                 capture_raises=False, modifiers_release=True,
                 modifiers_release_before_paste=True,
                 focus_at_copy="WIN-1", focus_before_paste=None):
        self.clipboard = original
        self._captured = captured
        self._capture_raises = capture_raises
        self._modifiers_release = modifiers_release
        self._modifiers_release_before_paste = modifiers_release_before_paste
        self._focus_at_copy = focus_at_copy
        self._focus_before_paste = (
            focus_at_copy if focus_before_paste is None else focus_before_paste
        )
        self.pastes = 0
        self.copies = 0
        self.pasted_content = None
        self.calls = []
        self._modifier_check_count = 0
        self._focus_check_count = 0

    def read_clipboard(self):
        self.calls.append("read")
        return self.clipboard

    def write_clipboard(self, text):
        self.calls.append(f"write:{text}")
        self.clipboard = text

    def clear_clipboard(self):
        self.calls.append("clear")
        self.clipboard = ""

    def wait_for_content(self):
        self.calls.append("wait_for_content")
        if self._capture_raises:
            raise ClipboardTimeout("nothing selected")
        self.clipboard = self._captured
        return self._captured

    def wait_for_modifier_release(self):
        self._modifier_check_count += 1
        self.calls.append("wait_for_modifier_release")
        if self._modifier_check_count == 1:
            return self._modifiers_release
        return self._modifiers_release_before_paste

    def send_copy(self):
        self.calls.append("copy")
        self.copies += 1

    def send_paste(self):
        self.calls.append("paste")
        self.pastes += 1
        self.pasted_content = self.clipboard

    def sleep(self, seconds):
        self.calls.append(f"sleep:{seconds}")

    def get_foreground_window(self):
        self._focus_check_count += 1
        self.calls.append("get_foreground_window")
        if self._focus_check_count == 1:
            return self._focus_at_copy
        return self._focus_before_paste

    def as_deps(self):
        return CycleDeps(
            read_clipboard=self.read_clipboard,
            write_clipboard=self.write_clipboard,
            clear_clipboard=self.clear_clipboard,
            wait_for_content=self.wait_for_content,
            wait_for_modifier_release=self.wait_for_modifier_release,
            send_copy=self.send_copy,
            send_paste=self.send_paste,
            sleep=self.sleep,
            get_foreground_window=self.get_foreground_window,
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
    assert os_.pasted_content == "Corrected text."


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
    assert os_.clipboard == "ORIGINAL"


def test_empty_backend_result_never_pastes():
    os_ = FakeOS()
    backend = StubBackend(result="   ")
    result = run_cycle(backend, new_timer(), os_.as_deps())
    assert result.pasted is False
    assert result.reason == "empty-result"
    assert os_.pastes == 0


def test_empty_backend_result_restores_the_original_clipboard():
    os_ = FakeOS(original="ORIGINAL")
    backend = StubBackend(result="   ")
    run_cycle(backend, new_timer(), os_.as_deps())
    assert os_.clipboard == "ORIGINAL"


def test_unexpected_exception_still_restores_the_clipboard_and_propagates():
    os_ = FakeOS(original="ORIGINAL")
    backend = StubBackend(raises=RuntimeError("boom"))
    with pytest.raises(RuntimeError):
        run_cycle(backend, new_timer(), os_.as_deps())
    assert os_.clipboard == "ORIGINAL"
    assert os_.pastes == 0


def test_records_all_timing_stages_on_the_happy_path():
    os_ = FakeOS()
    result = run_cycle(StubBackend(), new_timer(), os_.as_deps())
    report = result.timer.report()
    for stage in ("t0-hotkey", "t1-modifiers-released", "t2-copy-sent",
                  "t3-clipboard-captured", "t4-backend-done",
                  "t5-paste-sent", "t6-clipboard-restored"):
        assert stage in report


def test_happy_path_call_sequence_matches_the_safety_critical_order():
    """Mutation-tested: this must fail if clear_clipboard() or the post-paste
    sleep() is removed from run_cycle. See the report for the check performed.
    """
    os_ = FakeOS()
    run_cycle(StubBackend("Corrected text."), new_timer(), os_.as_deps())
    tracked = [c for c in os_.calls if c.split(":", 1)[0] in
               {"clear", "copy", "write", "paste", "sleep"}]
    assert tracked == [
        "clear",
        "copy",
        "write:Corrected text.",
        "paste",
        "sleep:0.3",
        "write:ORIGINAL",
    ]


def test_focus_unchanged_pastes_normally():
    os_ = FakeOS(focus_at_copy="WIN-1", focus_before_paste="WIN-1")
    result = run_cycle(StubBackend("Corrected text."), new_timer(), os_.as_deps())
    assert result.pasted is True
    assert os_.pastes == 1


def test_focus_changed_before_paste_aborts_without_pasting():
    os_ = FakeOS(focus_at_copy="WIN-1", focus_before_paste="WIN-2")
    result = run_cycle(StubBackend("Corrected text."), new_timer(), os_.as_deps())
    assert result.pasted is False
    assert result.reason == "focus-changed"
    assert os_.pastes == 0


def test_focus_changed_before_paste_restores_the_original_clipboard():
    os_ = FakeOS(original="ORIGINAL", focus_at_copy="WIN-1", focus_before_paste="WIN-2")
    run_cycle(StubBackend("Corrected text."), new_timer(), os_.as_deps())
    assert os_.clipboard == "ORIGINAL"


def test_modifiers_stuck_before_paste_aborts_without_pasting():
    os_ = FakeOS(modifiers_release_before_paste=False)
    result = run_cycle(StubBackend("Corrected text."), new_timer(), os_.as_deps())
    assert result.pasted is False
    assert result.reason == "modifiers-stuck-before-paste"
    assert os_.pastes == 0


def test_modifiers_stuck_before_paste_restores_the_original_clipboard():
    os_ = FakeOS(original="ORIGINAL", modifiers_release_before_paste=False)
    run_cycle(StubBackend("Corrected text."), new_timer(), os_.as_deps())
    assert os_.clipboard == "ORIGINAL"
