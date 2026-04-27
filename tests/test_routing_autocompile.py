"""Issue #75: routing snapshot auto-compiles on first request."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App


def test_routes_added_after_first_request_still_resolve() -> None:
    app = App()

    @app.get("/a")
    def a() -> str:
        return "a"

    async def _run() -> None:
        tr = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=tr, base_url="http://test") as c:
            r1 = await c.get("/a")
            assert r1.status_code == 200
            assert r1.text == "a"

            # This route is registered after first request/auto-compile snapshot.
            @app.get("/b")
            def b() -> str:
                return "b"

            r2 = await c.get("/b")
            assert r2.status_code == 200
            assert r2.text == "b"

    asyncio.run(_run())


def test_autocompile_keeps_405_behavior() -> None:
    app = App()

    @app.post("/x")
    def x() -> str:
        return "ok"

    async def _run() -> None:
        tr = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=tr, base_url="http://test") as c:
            r = await c.get("/x")
        assert r.status_code == 405
        allow = (r.headers.get("allow") or "").upper()
        assert "POST" in allow

    asyncio.run(_run())
