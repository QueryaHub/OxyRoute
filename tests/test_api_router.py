"""APIRouter + include_router (issue #46)."""

from __future__ import annotations

import asyncio
import json

import httpx
import pytest
from oxyroute import APIRouter, App
from oxyroute.router import join_path


def test_join_path() -> None:
    assert join_path("", "/a") == "/a"
    assert join_path("/v1", "/a") == "/v1/a"
    assert join_path("/v1", "a") == "/v1/a"
    assert join_path("v1", "/a") == "/v1/a"
    assert join_path("/v1", "/") == "/v1/"


def test_include_router_hits_routes() -> None:
    r = APIRouter()

    @r.get("/a")
    def a() -> str:
        return "A"

    @r.get("/b/:x")
    def b(x: str) -> str:
        return f"B{x}"

    app = App()
    app.include_router(r, prefix="/v1")

    async def _go() -> None:
        tr = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=tr, base_url="http://t") as c:
            r1 = await c.get("/v1/a")
            r2 = await c.get("/v1/b/7")
        assert r1.status_code == 200 and r1.text == "A"
        assert r2.status_code == 200 and r2.text == "B7"

    asyncio.run(_go())


def test_include_router_openapi_paths() -> None:
    r = APIRouter()

    @r.get("/items")
    def items() -> str:
        return "ok"

    app = App()
    app.include_router(r, prefix="/api")
    oa = json.loads(app.openapi_json())
    assert "/api/items" in oa.get("paths", {})


def test_include_defaults_passthrough() -> None:
    r = APIRouter()

    @r.get("/x")
    def x() -> str:
        return "x"

    app = App()
    app.include_router(r, "/p")
    oa = json.loads(app.openapi_json())
    assert "/p/x" in oa.get("paths", {})


def test_nested_include_router() -> None:
    inner = APIRouter()

    @inner.get("/c")
    def c() -> str:
        return "c"

    outer = APIRouter()
    outer.include_router(inner, prefix="/inner")
    app = App()
    app.include_router(outer, prefix="/v1")

    async def _go() -> None:
        tr = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=tr, base_url="http://t") as c:
            r0 = await c.get("/v1/inner/c")
        assert r0.status_code == 200 and r0.text == "c"

    asyncio.run(_go())


def test_duplicate_route_errors() -> None:
    r = APIRouter()

    @r.get("/d")
    def d1() -> str:
        return "1"

    @r.get("/d")
    def d2() -> str:
        return "2"

    app = App()
    with pytest.raises(ValueError, match=r"conflict|insertion"):
        app.include_router(r, "/z")
