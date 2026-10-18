from __future__ import annotations

import asyncio

from oxyroute import AdaptiveConcurrencyLimiter, App, Response


class _MockProtocol:
    def __init__(self) -> None:
        self.status = 200
        self.headers: list[tuple[str, str]] = []
        self.body: bytes | str | None = None

    def response_str(self, status: int, headers: list[tuple[str, str]], body: str) -> None:
        self.status = status
        self.headers = headers
        self.body = body

    def response_bytes(self, status: int, headers: list[tuple[str, str]], body: bytes) -> None:
        self.status = status
        self.headers = headers
        self.body = body

    def response_empty(self, status: int, headers: list[tuple[str, str]]) -> None:
        self.status = status
        self.headers = headers


class _MockScope:
    def __init__(self, method: str, path: str) -> None:
        self.proto = "http"
        self.http_version = "1.1"
        self.rsgi_version = "1.0"
        self.scheme = "http"
        self.method = method
        self.path = path
        self.query_string = ""
        self.headers = {}
        self.authority = "localhost"
        self.client = "127.0.0.1:54321"


def test_concurrency_get_set() -> None:
    app = App(max_concurrency=50)
    assert app.get_max_concurrency() == 50
    assert app.get_in_flight() == 0

    app.set_max_concurrency(100)
    assert app.get_max_concurrency() == 100

    app.set_max_concurrency(0)
    assert app.get_max_concurrency() == 0


def test_concurrency_load_shedding_503() -> None:
    app = App(max_concurrency=1)

    event_in_handler = asyncio.Event()
    event_release = asyncio.Event()

    @app.get("/slow")
    async def slow_handler() -> Response:
        event_in_handler.set()
        await event_release.wait()
        return Response(body="ok", status=200)

    async def run_scenario() -> None:
        proto1 = _MockProtocol()
        scope1 = _MockScope("GET", "/slow")

        proto2 = _MockProtocol()
        scope2 = _MockScope("GET", "/slow")

        task1 = asyncio.create_task(app.__rsgi__(scope1, proto1))

        await event_in_handler.wait()
        assert app.get_in_flight() == 1

        # Second request should immediately receive 503 shedding
        await app.__rsgi__(scope2, proto2)
        assert proto2.status == 503
        headers_dict = dict(proto2.headers)
        assert headers_dict.get("retry-after") == "1"

        # Release first request
        event_release.set()
        await task1
        assert proto1.status == 200
        assert app.get_in_flight() == 0

        # Subsequent request succeeds after in_flight drops
        proto3 = _MockProtocol()
        scope3 = _MockScope("GET", "/slow")
        event_release.set()
        await app.__rsgi__(scope3, proto3)
        assert proto3.status == 200

    asyncio.run(run_scenario())


def test_adaptive_concurrency_limiter() -> None:
    app = App()
    limiter = AdaptiveConcurrencyLimiter(
        app,
        min_limit=5,
        max_limit=100,
        initial_limit=20,
        smoothing=0.5,
    )

    assert app.get_max_concurrency() == 20

    # Low latency (10ms) establishes min_rtt
    limiter.on_request_completed(0.010)
    limiter.on_request_completed(0.010)
    assert app.get_max_concurrency() >= 20

    # Elevated latency (100ms) causes concurrency limit to decrease
    limiter.on_request_completed(0.100)
    limiter.on_request_completed(0.100)
    limiter.on_request_completed(0.100)
    assert app.get_max_concurrency() < 20

    # Recovery back to 10ms
    for _ in range(10):
        limiter.on_request_completed(0.010)

    assert app.get_max_concurrency() >= 5


def test_limiter_ignores_non_2xx_responses() -> None:
    app = App()
    limiter = AdaptiveConcurrencyLimiter(app, initial_limit=20, min_limit=5)
    limiter.on_request_completed(0.010, status=200)
    assert limiter.min_rtt == 0.010

    # Non-2xx responses (like fast 404 or 503 load shedding at 0.0001s) must not change min_rtt
    limiter.on_request_completed(0.0001, status=404)
    limiter.on_request_completed(0.0001, status=503)
    limiter.on_request_completed(0.0001, status=500)
    assert limiter.min_rtt == 0.010


def test_limiter_as_access_log_hook_converts_ms_to_seconds() -> None:
    import pytest

    app = App()
    limiter = AdaptiveConcurrencyLimiter(app, initial_limit=20)
    hook = limiter.as_access_log_hook()

    # Pass 15.0 milliseconds from App.__rsgi__
    hook(None, 200, 15.0, "/test")
    assert limiter.min_rtt == pytest.approx(0.015, rel=1e-5)

    # 503 should be ignored by the hook
    hook(None, 503, 0.05, "/test")
    assert limiter.min_rtt == pytest.approx(0.015, rel=1e-5)


def test_limiter_does_not_self_throttle_under_steady_load() -> None:
    app = App()
    limiter = AdaptiveConcurrencyLimiter(app, initial_limit=30, min_limit=5, max_limit=100)
    # Simulate steady load with minor jitter around 10-12ms
    for i in range(100):
        latency = 0.010 + (0.002 if i % 2 == 0 else 0.0)
        limiter.on_request_completed(latency, status=200)

    # Concurrency limit should remain healthy and NOT collapse to min_limit (5)
    assert limiter.current_limit >= 25


def test_limiter_windowed_min_rtt_expires_old_samples() -> None:
    app = App()
    limiter = AdaptiveConcurrencyLimiter(app, initial_limit=20, rtt_window_size=10)
    # 5 fast requests
    for _ in range(5):
        limiter.on_request_completed(0.005, status=200)
    assert limiter.min_rtt == 0.005

    # 10 slower requests at 0.020s fill the window and push out the 0.005s samples
    for _ in range(10):
        limiter.on_request_completed(0.020, status=200)

    assert limiter.min_rtt == 0.020
