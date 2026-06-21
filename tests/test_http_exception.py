"""``HTTPException`` mapped to HTTP responses (issue #48)."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App, HTTPException
from oxyroute.testing import asgi_test_app


def test_http_exception_404_string_detail() -> None:
    app = App()

    @app.get("/x")
    def x() -> str:
        raise HTTPException(404, "nope")

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/x")
        assert r.status_code == 404
        assert r.json() == {"detail": "nope"}

    asyncio.run(_run())


def test_http_exception_dict_body() -> None:
    app = App()

    @app.get("/e")
    def e() -> str:
        raise HTTPException(422, {"errors": [{"loc": "x", "msg": "bad"}]})

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/e")
        assert r.status_code == 422
        assert r.json() == {"errors": [{"loc": "x", "msg": "bad"}]}

    asyncio.run(_run())


def test_http_exception_custom_header() -> None:
    app = App()

    @app.get("/h")
    def h() -> str:
        raise HTTPException(400, "bad", headers={"X-App": "1"})

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/h")
        assert r.status_code == 400
        assert r.headers.get("x-app") == "1"
        assert "application/json" in (r.headers.get("content-type") or "")

    asyncio.run(_run())


def test_other_exception_still_500() -> None:
    app = App()

    @app.get("/b")
    def b() -> str:
        raise ValueError("oops")

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/b")
        assert r.status_code == 500
        j = r.json()
        assert j.get("error") == "internal server error"

    asyncio.run(_run())
