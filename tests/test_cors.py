"""CORS preflight and cross-origin response headers (issue #49)."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App, CORSConfig, apply_cors
from oxyroute.testing import asgi_test_app


def test_cors_preflight_204_allows_post() -> None:
    n = 0

    app = App()
    apply_cors(app, CORSConfig(allow_origins=["https://app.example"]))

    @app.post("/x")
    def _x() -> str:
        nonlocal n
        n += 1
        return "ok"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.request(
                "OPTIONS",
                "/x",
                headers={
                    "origin": "https://app.example",
                    "access-control-request-method": "POST",
                },
            )
        assert r.status_code == 204, r.text
        assert r.headers.get("access-control-allow-origin") == "https://app.example"
        assert "POST" in (r.headers.get("access-control-allow-methods") or "")
        assert n == 0

    asyncio.run(_run())


def test_cors_get_with_origin_merges_headers() -> None:
    app = App()
    apply_cors(app, CORSConfig(allow_origins=["https://a.example", "https://b.example"]))

    @app.get("/hi")
    def _hi() -> str:
        return "hello"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/hi", headers={"origin": "https://a.example"})
        assert r.status_code == 200
        assert r.text == "hello"
        assert r.headers.get("access-control-allow-origin") == "https://a.example"

    asyncio.run(_run())
