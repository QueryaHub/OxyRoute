from __future__ import annotations

import math
from collections import deque
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
        rtt_window_size: int = 100,
    ) -> None:
        self.app = app
        self.min_limit = max(1, min_limit)
        self.max_limit = max(self.min_limit, max_limit)
        self.current_limit: float = float(min(max(initial_limit, self.min_limit), self.max_limit))
        self.alpha = alpha
        self.smoothing = smoothing
        self.rtt_window_size = max(10, rtt_window_size)
        self._rtt_samples: deque[float] = deque(maxlen=self.rtt_window_size)
        self.min_rtt: float | None = None
        self.avg_rtt: float | None = None

        self.app.set_max_concurrency(int(self.current_limit))

    def on_request_completed(self, latency_seconds: float, status: int = 200) -> int:
        """
        Record completed request latency and adjust concurrency limit.
        Only 2xx responses are sampled to prevent fast rejections (e.g. 503 load-shedding
        or 404s) from skewing min_rtt toward zero.
        Returns the updated integer concurrency limit.
        """
        if latency_seconds <= 0.0 or not (200 <= status < 300):
            return int(self.current_limit)

        self._rtt_samples.append(latency_seconds)
        self.min_rtt = min(self._rtt_samples)

        if self.avg_rtt is None:
            self.avg_rtt = latency_seconds
        else:
            self.avg_rtt = (1.0 - self.smoothing) * self.avg_rtt + self.smoothing * latency_seconds

        if self.min_rtt > 0 and self.avg_rtt > 0:
            gradient = min(1.0, self.min_rtt / self.avg_rtt)
            headroom = max(self.alpha, math.sqrt(self.current_limit))
            target = self.current_limit * gradient + headroom
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

        def _hook(scope: Any, status: int, duration_ms: float, path_template: str) -> None:
            self.on_request_completed(duration_ms / 1000.0, status=status)

        return _hook
