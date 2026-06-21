"""HEAD shares GET routes; OPTIONS has its own (issue #20)."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App, Response
from oxyroute.testing import asgi_test_app


def test_asgi_head_same_path_as_get_empty_body() -> None:
    app = App()

    @app.get("/h")
    def h() -> str:
        return "hello"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            g = await c.get("/h")
            head = await c.request("HEAD", "/h")
        assert g.status_code == 200
        assert g.text == "hello"
        assert head.status_code == 200
        assert head.text == ""
        assert (head.headers.get("content-length") or "").strip() == "5"

    asyncio.run(_run())


def test_asgi_head_openapi_length() -> None:
    app = App(include_openapi=True)

    @app.get("/x")
    def x() -> str:
        return "y"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.request("HEAD", "/openapi.json")
        assert r.status_code == 200
        assert r.text == ""
        doc = app.openapi_json()
        assert (r.headers.get("content-length") or "") == str(len(doc.encode("utf-8")))

    asyncio.run(_run())


def test_asgi_options_route() -> None:
    app = App()

    @app.options("/cors")
    def preflight() -> str:
        return "ok"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.request("OPTIONS", "/cors")
        assert r.status_code == 200
        assert r.text == "ok"

    asyncio.run(_run())


def test_asgi_head_structured_response() -> None:
    app = App()

    @app.get("/r")
    def r() -> Response:
        return Response(
            body={"a": 1},
            status=201,
            headers={"content-type": "application/json", "X-M": "v"},
        )

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            g = await c.get("/r")
            head = await c.request("HEAD", "/r")
        import json

        b = json.dumps({"a": 1}).encode()
        assert g.status_code == 201
        assert g.json() == {"a": 1}
        assert head.status_code == 201
        assert head.text == ""
        assert (head.headers.get("content-length") or "") == str(len(b))
        assert head.headers.get("x-m") == "v"

    asyncio.run(_run())
