"""Concurrent ASGI requests (issue #17) — no deadlock on many parallel clients."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App


def test_asgi_50_concurrent_gets() -> None:
    async def _run() -> None:
        app = App()

        @app.get("/c")
        def c() -> str:
            return "ok"

        async def one(client: httpx.AsyncClient) -> int:
            r = await client.get("/c")
            return r.status_code

        transport = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as client:
            tasks = [one(client) for _ in range(50)]
            codes = await asyncio.gather(*tasks)
        assert all(c == 200 for c in codes)

    asyncio.run(_run())
