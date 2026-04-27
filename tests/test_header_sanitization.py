"""Issue #72: reject unsafe CR/LF control chars in response headers/cookies."""

from __future__ import annotations

import asyncio

import httpx

from tests._rsgi_test_transport import asgi_test_app
from oxyroute import App, HTTPException, Response


def test_response_header_crlf_is_rejected_with_500() -> None:
    app = App()

    @app.get("/h")
    def h() -> Response:
        return Response(status=200, body="ok", headers={"X-Bad\r\nInjected": "1"})

    async def _run() -> None:
        tr = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=tr, base_url="http://test") as c:
            r = await c.get("/h")
        assert r.status_code == 500
        assert r.headers.get("x-bad") is None
        assert r.json().get("error") == "internal server error"

    asyncio.run(_run())


def test_response_cookie_control_chars_is_rejected_with_500() -> None:
    app = App()

    @app.get("/c")
    def c() -> Response:
        return Response(status=200, body="ok", cookies=["a=1\r\nX: y"])

    async def _run() -> None:
        tr = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=tr, base_url="http://test") as c:
            r = await c.get("/c")
        assert r.status_code == 500
        assert r.json().get("error") == "internal server error"

    asyncio.run(_run())


def test_http_exception_header_crlf_is_safely_downgraded_to_500() -> None:
    app = App()

    @app.get("/e")
    def e() -> str:
        raise HTTPException(400, "bad", headers={"X-Err\nInject": "1"})

    async def _run() -> None:
        tr = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=tr, base_url="http://test") as c:
            r = await c.get("/e")
        assert r.status_code == 500
        assert r.headers.get("x-err") is None
        assert r.json().get("error") == "internal server error"

    asyncio.run(_run())
