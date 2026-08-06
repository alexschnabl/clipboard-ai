class FakeClock:
    """Monotonic clock that advances a fixed step on every call.

    First call returns 0.0, so `elapsed` arithmetic in tests is predictable.
    """

    def __init__(self, step: float = 0.02):
        self.now = 0.0
        self.step = step

    def __call__(self) -> float:
        value = self.now
        self.now += self.step
        return value
