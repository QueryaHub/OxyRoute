"""ASGI 3.0 bridge to the same RSGI core as ``__rsgi__`` (optional server compatibility)."""

from __future__ import annotations

import asyncio

import httpx

from oxyroute import App


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
