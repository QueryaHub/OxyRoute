"""Handler return-value -> HTTP response mapping (issue #211).

``list``/``dict`` handler returns must always be JSON-serialized, never misread as raw
bytes; a dict that merely happens to contain "status" and "body" keys as ordinary data
must round-trip as JSON unless "status" is a plausible HTTP status code.
"""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App
from oxyroute.testing import asgi_test_app


def _client(app: App) -> httpx.AsyncClient:
    transport = httpx.ASGITransport(app=asgi_test_app(app))
    return httpx.AsyncClient(transport=transport, base_url="http://test")


def test_empty_list_is_json_not_octet_stream() -> None:
    app = App()

    @app.get("/empty")
    def empty() -> list:
        return []

    async def _run() -> None:
        async with _client(app) as c:
            r = await c.get("/empty")
        assert r.status_code == 200
        assert r.headers.get("content-type", "").startswith("application/json")
        assert r.json() == []

    asyncio.run(_run())


def test_int_list_is_json_not_raw_bytes() -> None:
    app = App()

    @app.get("/ids")
    def ids() -> list:
        return [1, 2, 3]

    async def _run() -> None:
        async with _client(app) as c:
            r = await c.get("/ids")
        assert r.status_code == 200
        assert r.headers.get("content-type", "").startswith("application/json")
        assert r.json() == [1, 2, 3]

    asyncio.run(_run())


def test_dict_with_out_of_range_status_roundtrips_as_data() -> None:
    """A dict with "status"/"body" keys that isn't a plausible HTTP status (100-599)
    must not be reinterpreted as a structured response."""
    app = App()

    @app.get("/data")
    def data() -> dict:
        return {"status": 1, "body": "x"}

    async def _run() -> None:
        async with _client(app) as c:
            r = await c.get("/data")
        assert r.status_code == 200
        assert r.headers.get("content-type", "").startswith("application/json")
        assert r.json() == {"status": 1, "body": "x"}

    asyncio.run(_run())


def test_dict_with_valid_status_and_body_is_still_a_structured_response() -> None:
    """Existing, documented shorthand: a dict with a real HTTP status code + body sets
    the response status (docs/usage.md)."""
    app = App()

    @app.get("/teapot")
    def teapot() -> dict:
        return {"status": 418, "body": "I'm a teapot"}

    async def _run() -> None:
        async with _client(app) as c:
            r = await c.get("/teapot")
        assert r.status_code == 418
        assert r.text == "I'm a teapot"

    asyncio.run(_run())
