import time
from typing import Callable


class StageTimer:
    """Records named timestamps and reports the gaps between them."""

    def __init__(self, clock: Callable[[], float] = time.perf_counter):
        self._clock = clock
        self._marks: dict[str, float] = {}
        self._order: list[str] = []

    def mark(self, stage: str) -> None:
        self._marks[stage] = self._clock()
        self._order.append(stage)

    def elapsed_ms(self, from_stage: str, to_stage: str) -> float:
        return (self._marks[to_stage] - self._marks[from_stage]) * 1000.0

    def report(self) -> str:
        if not self._order:
            return "(no stages recorded)"
        first = self._order[0]
        lines = []
        for stage in self._order:
            since_start = (self._marks[stage] - self._marks[first]) * 1000.0
            lines.append(f"  {stage:<28} {since_start:8.1f} ms")
        return "\n".join(lines)
