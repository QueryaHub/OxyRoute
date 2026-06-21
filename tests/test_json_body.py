"""JSON request body injection via native ``json_to_py`` (issue #95)."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App
from oxyroute.testing import asgi_test_app


def test_json_body_nested_types() -> None:
    app = App()
    seen: dict[str, object] = {}

    @app.post("/echo")
    def echo(json: dict) -> str:
        seen["json"] = json
        return "ok"

    payload = {"a": 1, "b": [None, False, "x"], "c": {"d": 2.5}}

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post("/echo", json=payload)
        assert r.status_code == 200, r.text
        assert r.text == "ok"

    asyncio.run(_run())
    assert seen["json"] == payload


def test_json_body_scalar_and_empty_object() -> None:
    app = App()
    seen: dict[str, object] = {}

    @app.post("/nums")
    def nums(json: dict) -> str:
        seen["json"] = json
        return "ok"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post("/nums", json={"n": 42, "f": 1.25, "s": "z", "t": True, "u": {}})
        assert r.status_code == 200, r.text

    asyncio.run(_run())
    assert seen["json"] == {"n": 42, "f": 1.25, "s": "z", "t": True, "u": {}}
