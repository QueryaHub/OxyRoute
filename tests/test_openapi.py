import asyncio
import json

import httpx
import pytest
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


def test_include_openapi_false_reenabled_with_set_openapi_served_get_200() -> None:
    app = App(title="R", include_openapi=False)
    app.set_openapi_served(True)

    @app.get("/b")
    def b() -> str:
        return "b"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=app)
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/openapi.json")
        assert r.status_code == 200, r.text
        doc = r.json()
        assert doc["info"]["title"] == "R"
        assert "/b" in doc["paths"]

    asyncio.run(_run())


def test_openapi_post_request_body_from_pydantic() -> None:
    pydantic = pytest.importorskip("pydantic")

    class ItemIn(pydantic.BaseModel):
        title: str
        count: int = 0

    app = App()

    @app.post("/items", body_model=ItemIn)
    def create_item(json: dict) -> str:
        return "ok"

    doc = json.loads(app.openapi_json())
    rb = doc["paths"]["/items"]["post"]["requestBody"]
    assert rb["required"] is True
    ct = rb["content"]["application/json"]["schema"]
    assert ct["type"] == "object"
    assert "title" in ct.get("properties", {})
    assert "count" in ct.get("properties", {})


def test_openapi_post_request_body_from_body_schema_dict() -> None:
    app = App()
    raw_schema = {
        "type": "object",
        "required": ["a"],
        "properties": {"a": {"type": "string"}, "b": {"type": "integer"}},
    }

    @app.post("/raw", body_schema=raw_schema)
    def create_raw(json: dict) -> str:
        return "ok"

    doc = json.loads(app.openapi_json())
    rb = doc["paths"]["/raw"]["post"]["requestBody"]
    assert rb["required"] is True
    s = rb["content"]["application/json"]["schema"]
    assert s["type"] == "object"
    assert s["required"] == ["a"]
    assert s["properties"]["a"]["type"] == "string"


def test_openapi_body_model_and_body_schema_rejected() -> None:
    pydantic = pytest.importorskip("pydantic")

    class M(pydantic.BaseModel):
        x: int

    app = App()

    with pytest.raises(TypeError, match="body_model and body_schema"):

        @app.post("/x", body_model=M, body_schema={"type": "object"})
        def _bad(json: dict) -> str:
            return "n"
