import asyncio
import json

import httpx
from oxyroute import App


def test_openapi_shows_route() -> None:
    app = App(title="T")

    @app.get("/items/:i")
    def list_items() -> str:
        return "ok"

    s = app.openapi_json()
    assert "paths" in s
    assert "/items/:i" in s
    assert "T" in s


def test_openapi_includes_patch_lowercase() -> None:
    app = App()

    @app.patch("/m")
    def m() -> str:
        return "ok"

    doc = json.loads(app.openapi_json())
    assert doc["paths"]["/m"]["patch"]["operationId"] == "m"


def test_openapi_serving_off_constructor_get_and_head_404_openapi_json_still_filled() -> None:
    app = App(title="S", include_openapi=False)

    @app.get("/a")
    def a() -> str:
        return "1"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            g = await c.get("/openapi.json")
            h = await c.request("HEAD", "/openapi.json")
        assert g.status_code == 404
        assert h.status_code == 404

    asyncio.run(_run())

    doc = json.loads(app.openapi_json())
    assert doc["info"]["title"] == "S"
    assert "/a" in doc["paths"]


def test_set_openapi_served_false_stops_serving_still_exports_json() -> None:
    app = App()
    app.set_openapi_served(False)

    @app.get("/z")
    def z() -> str:
        return "z"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/openapi.json")
        assert r.status_code == 404

    asyncio.run(_run())
    assert "/z" in json.loads(app.openapi_json())["paths"]
