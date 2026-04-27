"""Pre-route `set_middleware` short-circuits before body read (CORS preflight, issue #11)."""

from __future__ import annotations

import asyncio

import httpx

from tests._rsgi_test_transport import asgi_test_app
from oxyroute import App, Response


def test_middleware_cors_preflight_204_no_route_ran() -> None:
    n = 0

    def mw(scope, _protocol) -> Response | None:
        if scope.method == "OPTIONS" and scope.headers.get("access-control-request-method", ""):
            return Response(
                status=204,
                body=None,
                headers={"access-control-allow-origin": "*"},
            )
        return None

    app = App()
    app.set_middleware(mw)

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
                headers={"access-control-request-method": "POST"},
            )
        assert r.status_code == 204, r.text
        assert (r.headers.get("access-control-allow-origin") or "") == "*"
        assert n == 0

    asyncio.run(_run())
