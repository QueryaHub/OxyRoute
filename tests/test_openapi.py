import asyncio
import json

import httpx
import pytest
from oxyroute import APIRouter, App
from oxyroute.testing import asgi_test_app


def test_openapi_shows_route() -> None:
    app = App(title="T")

    @app.get("/items/:i")
    def list_items() -> str:
        return "ok"

    s = app.openapi_json()
    assert "paths" in s
    assert "/items/{i}" in s
    assert "/items/:i" not in s
    assert "T" in s
    doc = json.loads(s)
    params = doc["paths"]["/items/{i}"]["get"]["parameters"]
    assert params == [
        {
            "name": "i",
            "in": "path",
            "required": True,
            "schema": {"type": "string"},
        }
    ]


def test_openapi_includes_patch_lowercase() -> None:
    app = App()

    @app.patch("/m")
    def m() -> str:
        return "ok"

    doc = json.loads(app.openapi_json())
    assert doc["paths"]["/m"]["patch"]["operationId"] == "m"


def test_openapi_jwt_security_scheme() -> None:
    app = App()

    @app.get("/public")
    def public() -> str:
        return "ok"

    @app.get("/secret", require_jwt=True, jwt_secret="test-secret-key")
    def secret(claims: dict) -> str:
        return "ok"

    doc = json.loads(app.openapi_json())
    schemes = doc["components"]["securitySchemes"]
    assert schemes["bearerAuth"]["type"] == "http"
    assert schemes["bearerAuth"]["scheme"] == "bearer"
    assert schemes["bearerAuth"]["bearerFormat"] == "JWT"
    assert doc["paths"]["/secret"]["get"]["security"] == [{"bearerAuth": []}]
    assert "security" not in doc["paths"]["/public"]["get"]


def test_openapi_tags_from_route_and_include_router() -> None:
    r = APIRouter()

    @r.get("/a", tags=["alpha"])
    def a() -> str:
        return "a"

    app = App()
    app.include_router(r, prefix="/api", tags=["shared"])

    @app.get("/b", tags=["beta"])
    def b() -> str:
        return "b"

    doc = json.loads(app.openapi_json())
    # include_router merges defaults then per-route opts — route tags win over defaults
    assert doc["paths"]["/api/a"]["get"]["tags"] == ["alpha"]
    assert doc["paths"]["/b"]["get"]["tags"] == ["beta"]


def test_openapi_include_router_default_tags() -> None:
    r = APIRouter()

    @r.get("/x")
    def x() -> str:
        return "x"

    app = App()
    app.include_router(r, prefix="/v1", tags=["v1"])
    doc = json.loads(app.openapi_json())
    assert doc["paths"]["/v1/x"]["get"]["tags"] == ["v1"]


def test_openapi_set_info_and_servers() -> None:
    app = App(
        title="API",
        openapi_description="Demo",
        openapi_contact={"name": "Ops", "email": "ops@example.com"},
        openapi_servers=[{"url": "https://api.example.com", "description": "prod"}],
    )
    doc = json.loads(app.openapi_json())
    assert doc["info"]["description"] == "Demo"
    assert doc["info"]["contact"]["email"] == "ops@example.com"
    assert doc["servers"][0]["url"] == "https://api.example.com"


def test_docs_ui_scalar_returns_html() -> None:
    app = App(title="DocsApp", docs_ui="scalar")

    @app.get("/ping")
    def ping() -> str:
        return "pong"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/docs")
        assert r.status_code == 200
        assert "text/html" in r.headers.get("content-type", "")
        assert "/openapi.json" in r.text
        assert "scalar" in r.text.lower() or "api-reference" in r.text

    asyncio.run(_run())


def test_docs_ui_swagger_via_mount_docs() -> None:
    app = App(title="Swag")
    app.mount_docs("/swagger", ui="swagger")

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/swagger")
        assert r.status_code == 200
        assert "text/html" in r.headers.get("content-type", "")
        assert "swagger-ui" in r.text.lower()

    asyncio.run(_run())


def test_openapi_serving_off_constructor_get_and_head_404_openapi_json_still_filled() -> None:
    app = App(title="S", include_openapi=False)

    @app.get("/a")
    def a() -> str:
        return "1"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
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
        transport = httpx.ASGITransport(app=asgi_test_app(app))
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
        transport = httpx.ASGITransport(app=asgi_test_app(app))
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
