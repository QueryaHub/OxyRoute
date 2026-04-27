"""405 Method Not Allowed with Allow when the path exists for other verbs (issue #19)."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App
from tests._rsgi_test_transport import asgi_test_app


def test_405_get_on_post_only_path() -> None:
    app = App()

    @app.post("/p")
    def p() -> str:
        return "x"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/p")
        assert r.status_code == 405, r.text
        assert (r.headers.get("allow") or "").upper().find("POST") >= 0
        assert r.text == "Method Not Allowed"

    asyncio.run(_run())


def test_405_post_on_get_only_includes_get_and_head() -> None:
    app = App()

    @app.get("/g")
    def g() -> str:
        return "1"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post("/g")
        assert r.status_code == 405
        allow = (r.headers.get("allow") or "").upper()
        assert "GET" in allow
        assert "HEAD" in allow

    asyncio.run(_run())


def test_404_still_404_when_nothing_matches() -> None:
    app = App()

    @app.get("/a")
    def a() -> str:
        return "a"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post("/nope")
        assert r.status_code == 404

    asyncio.run(_run())
