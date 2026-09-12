"""Dependency chains: later factories receive earlier results by name; optional ``request`` context."""

from __future__ import annotations

import asyncio

import httpx
import pytest
from oxyroute import App
from oxyroute.testing import asgi_test_app


def test_dep_second_receives_first_by_name() -> None:
    def make_a() -> int:
        return 10

    def make_b(a: int) -> int:
        return a + 5

    app = App()

    @app.get("/x", dependencies=[("a", make_a), ("b", make_b)])
    def route(b: int) -> str:
        return f"v={b}"

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/x")
        assert r.status_code == 200
        assert r.text == "v=15"

    asyncio.run(run())


def test_dep_request_context_headers() -> None:
    def with_req(request) -> str:
        return request["headers"].get("x-trace", "")

    app = App()

    @app.get("/t", dependencies=[("trace_id", with_req)])
    def route(trace_id: str) -> str:
        return trace_id

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/t", headers={"X-Trace": "z9"})
        assert r.status_code == 200
        assert r.text == "z9"

    asyncio.run(run())


def test_dep_request_context_typed() -> None:
    from oxyroute.request import Request

    def with_req(request: Request) -> str:
        client = request.client or "unknown"
        trace = request.headers.get("x-trace", "none")
        return f"{request.method} {request.path} {client} {trace}"

    app = App()

    @app.get("/t2", dependencies=[("info", with_req)])
    def route2(info: str) -> str:
        return info

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app), client=("1.2.3.4", 1234))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/t2", headers={"X-Trace": "y8"})
        assert r.status_code == 200, r.text
        assert r.text == "GET /t2 1.2.3.4:1234 y8"

    asyncio.run(run())


def test_dep_async_chain() -> None:
    async def make_a() -> str:
        return "aa"

    def make_b(a: str) -> str:
        return a + "b"

    app = App()

    @app.get("/a", dependencies=[("a", make_a), ("b", make_b)])
    def route(b: str) -> str:
        return b

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/a")
        assert r.status_code == 200
        assert r.text == "aab"

    asyncio.run(run())


def test_dep_cycle_detection() -> None:
    def make_a(b: int) -> int:
        return b + 1

    def make_b(a: int) -> int:
        return a + 1

    app = App()

    with pytest.raises(ValueError, match=r"dependency cycle detected"):

        @app.get("/cycle", dependencies=[("a", make_a), ("b", make_b)])
        def route(a: int, b: int) -> str:
            return f"{a},{b}"


def test_dep_topological_reordering() -> None:
    def make_a() -> int:
        return 10

    def make_b(a: int) -> int:
        return a + 5

    app = App()

    # Declared in reverse order (b before a), but b depends on a
    @app.get("/rev", dependencies=[("b", make_b), ("a", make_a)])
    def route(b: int) -> str:
        return f"v={b}"

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/rev")
        assert r.status_code == 200
        assert r.text == "v=15"

    asyncio.run(run())
