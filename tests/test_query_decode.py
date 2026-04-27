"""Query string: percent-decoding and + handling (see issue #2, src/params.rs)."""

from __future__ import annotations

import asyncio

import httpx

from tests._rsgi_test_transport import asgi_test_app
from oxyroute import App


def test_query_value_percent_decoded() -> None:
    app = App()

    @app.get("/echo")
    def echo(**kwargs) -> str:
        return (kwargs.get("query") or {}).get("m", "")

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/echo?m=hello%20world")
        assert r.status_code == 200
        assert r.text == "hello world"

    asyncio.run(_run())
