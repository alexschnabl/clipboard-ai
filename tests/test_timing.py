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
