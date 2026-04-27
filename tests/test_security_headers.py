"""Security header preset (issue #54)."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App, SecurityHeadersConfig
from tests._rsgi_test_transport import asgi_test_app


def test_security_headers_merged_on_get() -> None:
    app = App()
    app.set_security_headers(
        SecurityHeadersConfig(
            hsts="max-age=60",
            x_frame_options="SAMEORIGIN",
        )
    )

    @app.get("/h")
    def _h() -> str:
        return "x"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="https://test") as c:
            r = await c.get("/h")
        assert r.status_code == 200
        assert (r.headers.get("x-content-type-options") or "") == "nosniff"
        assert (r.headers.get("x-frame-options") or "") == "SAMEORIGIN"
        assert (r.headers.get("strict-transport-security") or "") == "max-age=60"
        assert (r.headers.get("referrer-policy") or "") == "strict-origin-when-cross-origin"

    asyncio.run(_run())


def test_security_headers_hsts_not_on_http() -> None:
    app = App()
    app.set_security_headers(
        SecurityHeadersConfig(
            hsts="max-age=60",
        )
    )

    @app.get("/h")
    def _h2() -> str:
        return "x"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/h")
        assert r.status_code == 200
        assert "strict-transport-security" not in (k.lower() for k in r.headers)

    asyncio.run(_run())


def test_security_headers_does_not_override_response_header() -> None:
    from oxyroute import Response

    app = App()
    app.set_security_headers(SecurityHeadersConfig(x_frame_options="DENY"))

    @app.get("/h")
    def _h3() -> Response:
        return Response(
            body="a",
            headers={"X-Frame-Options": "ALLOWALL"},
        )

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/h")
        assert (r.headers.get("x-frame-options") or "") == "ALLOWALL"

    asyncio.run(_run())
