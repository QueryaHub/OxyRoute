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
