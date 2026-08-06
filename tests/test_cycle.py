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
