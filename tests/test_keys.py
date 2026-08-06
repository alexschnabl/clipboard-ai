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
