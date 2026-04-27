"""SSE helper over RSGI protocol (issue #51)."""

from __future__ import annotations

import asyncio

import httpx

from tests._rsgi_test_transport import asgi_test_app
from oxyroute import App, send_sse


def test_sse_response_body_and_content_type() -> None:
    app = App()

    @app.get("/events")
    async def events(protocol: object) -> object:
        data = ["ready", "tick"]
        return await send_sse(protocol, data)  # type: ignore[arg-type]

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/events")
        assert r.status_code == 200
        assert "text/event-stream" in r.headers.get("content-type", "")
        assert "data: ready\n\n" in r.text
        assert "data: tick\n\n" in r.text

    asyncio.run(_run())
