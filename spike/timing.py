import time
from typing import Callable


class StageTimer:
    """Records named timestamps and reports each one's time since the first mark.

    report() prints time cumulative since t0 (the first stage marked), not the
    gap since the previous stage. Use elapsed_ms(from_stage, to_stage) for a
    gap between two specific stages.
    """

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
        """Render each stage with its time since t0 (cumulative, not a per-stage delta)."""
        if not self._order:
            return "(no stages recorded)"
        first = self._order[0]
        lines = [f"  {'stage':<28} {'since t0 (ms)':>13}"]
        for stage in self._order:
            since_start = (self._marks[stage] - self._marks[first]) * 1000.0
            lines.append(f"  {stage:<28} {since_start:8.1f} ms")
        return "\n".join(lines)
