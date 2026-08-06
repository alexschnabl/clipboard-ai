import pytest

from spike.clipboard import ClipboardTimeout, wait_for_content
from tests.fakes import FakeClock


def make_reader(values):
    """Returns a reader that yields each value once, then repeats the last."""
    remaining = list(values)

    def reader() -> str:
        if len(remaining) > 1:
            return remaining.pop(0)
        return remaining[0]

    return reader


def test_returns_content_once_it_appears():
    reader = make_reader(["", "", "hello world"])
    result = wait_for_content(
        timeout_ms=1000,
        poll_ms=20,
        reader=reader,
        clock=FakeClock(step=0.02),
        sleep=lambda _: None,
    )
    assert result == "hello world"


def test_returns_immediately_if_content_is_already_present():
    reader = make_reader(["already here"])
    result = wait_for_content(
        timeout_ms=1000,
        poll_ms=20,
        reader=reader,
        clock=FakeClock(step=0.02),
        sleep=lambda _: None,
    )
    assert result == "already here"


def test_raises_timeout_when_clipboard_stays_empty():
    with pytest.raises(ClipboardTimeout):
        wait_for_content(
            timeout_ms=1000,
            poll_ms=20,
            reader=lambda: "",
            clock=FakeClock(step=0.02),
            sleep=lambda _: None,
        )


def test_treats_whitespace_only_clipboard_as_empty():
    with pytest.raises(ClipboardTimeout):
        wait_for_content(
            timeout_ms=1000,
            poll_ms=20,
            reader=lambda: "   \n\t  ",
            clock=FakeClock(step=0.02),
            sleep=lambda _: None,
        )


def test_sleeps_between_polls():
    naps = []
    reader = make_reader(["", "", "text"])
    wait_for_content(
        timeout_ms=1000,
        poll_ms=20,
        reader=reader,
        clock=FakeClock(step=0.02),
        sleep=naps.append,
    )
    assert naps, "expected wait_for_content to sleep between polls"
    assert all(nap == pytest.approx(0.02) for nap in naps)
