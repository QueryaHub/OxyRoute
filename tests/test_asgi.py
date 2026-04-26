"""ASGI 3.0 bridge to the same RSGI core as ``__rsgi__`` (optional server compatibility)."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App, Response


def test_asgi_get_plain_text() -> None:
    app = App()

    @app.get("/p")
    def p() -> str:
        return "x"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r2 = await c.get("/p")
        assert r2.status_code == 200
        assert r2.text == "x"

    asyncio.run(_run())


def test_asgi_patch_json_body() -> None:
    app = App()

    @app.patch("/x")
    def patch_x(json: dict) -> str:
        return f"p:{json.get('a')}"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.patch("/x", json={"a": 7})
        assert r.status_code == 200
        assert r.text == "p:7"

    asyncio.run(_run())


def test_asgi_response_custom_headers_and_json_ct() -> None:
    app = App()

    @app.get("/j")
    def j() -> Response:
        return Response(
            body={"x": 1},
            status=201,
            headers={"content-type": "application/json", "X-Trick": "z"},
        )

    async def _run() -> None:
        transport = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/j")
        assert r.status_code == 201, r.text
        assert r.headers["x-trick"] == "z"
        assert "application/json" in (r.headers.get("content-type") or "")
        assert r.json() == {"x": 1}

    asyncio.run(_run())
