from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .app import App


class AdaptiveConcurrencyLimiter:
    """
    Adaptive concurrency limiter based on TCP Vegas / Little's Law gradient algorithm.

    Monitors request latency (RTT) and dynamically scales `app.set_max_concurrency(...)`
    to prevent queue build-up, memory spikes, and tail-latency degradation under load.
    """

    def __init__(
        self,
        app: App,
        *,
        min_limit: int = 4,
        max_limit: int = 1024,
        initial_limit: int = 32,
        alpha: float = 0.1,
        smoothing: float = 0.2,
    ) -> None:
        self.app = app
        self.min_limit = max(1, min_limit)
        self.max_limit = max(self.min_limit, max_limit)
        self.current_limit: float = float(min(max(initial_limit, self.min_limit), self.max_limit))
        self.alpha = alpha
        self.smoothing = smoothing
        self.min_rtt: float | None = None
        self.avg_rtt: float | None = None

        self.app.set_max_concurrency(int(self.current_limit))

    def on_request_completed(self, latency_seconds: float) -> int:
        """
        Record completed request latency and adjust concurrency limit.
        Returns the updated integer concurrency limit.
        """
        if latency_seconds <= 0.0:
            return int(self.current_limit)

        if self.min_rtt is None or latency_seconds < self.min_rtt:
            self.min_rtt = latency_seconds

        if self.avg_rtt is None:
            self.avg_rtt = latency_seconds
        else:
            self.avg_rtt = (1.0 - self.smoothing) * self.avg_rtt + self.smoothing * latency_seconds

        if self.min_rtt > 0 and self.avg_rtt > 0:
            gradient = self.min_rtt / self.avg_rtt
            # Vegas calculation: target = limit * gradient + alpha
            target = self.current_limit * gradient + self.alpha
            # Smooth adjustment towards target
            self.current_limit = (
                1.0 - self.smoothing
            ) * self.current_limit + self.smoothing * target
            self.current_limit = max(
                float(self.min_limit), min(float(self.max_limit), self.current_limit)
            )
            new_limit = int(self.current_limit)
            self.app.set_max_concurrency(new_limit)
            return new_limit

        return int(self.current_limit)

    def as_access_log_hook(self) -> Any:
        """
        Return a callable suitable for `App(access_log_hook=limiter.as_access_log_hook())`.
        """

        def _hook(scope: Any, status: int, duration_seconds: float, path_template: str) -> None:
            self.on_request_completed(duration_seconds)

        return _hook
