import asyncio
from typing import Any

import httpx
from oxyroute import App
from oxyroute.testing import asgi_test_app
from pydantic import BaseModel


class Item(BaseModel):
    name: str
    price: float


def test_vectorcall_sync_and_async_handlers() -> None:
    app = App()

    @app.get("/sync/:id")
    def handle_sync(id: int, query: dict[str, Any]) -> dict[str, Any]:
        return {"type": "sync", "id": id, "filter": query.get("filter")}

    @app.post("/async/:id")
    async def handle_async(id: int, query: dict[str, Any], json: dict[str, Any]) -> dict[str, Any]:
        return {"type": "async", "id": id, "filter": query.get("filter"), "data": json}

    def get_dep_a() -> str:
        return "dep_val_a"

    async def get_dep_b(request: dict[str, Any]) -> str:
        return f"dep_b:{request.get('path')}"

    @app.get("/deps", dependencies=[("dep_a", get_dep_a), ("dep_b", get_dep_b)])
    async def handle_deps(dep_a: str, dep_b: str) -> dict[str, str]:
        return {"dep_a": dep_a, "dep_b": dep_b}

    @app.post("/pydantic", body_model=Item)
    def handle_pydantic(json: Item) -> dict[str, Any]:
        return {"name": json.name, "price": json.price}

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            # Sync handler with path param and query
            r1 = await c.get("/sync/42?filter=active")
            assert r1.status_code == 200, r1.text
            assert r1.json() == {"type": "sync", "id": 42, "filter": "active"}

            # Async handler with path param, query, and json body
            r2 = await c.post("/async/99?filter=all", json={"hello": "world"})
            assert r2.status_code == 200, r2.text
            assert r2.json() == {
                "type": "async",
                "id": 99,
                "filter": "all",
                "data": {"hello": "world"},
            }

            # Dependencies
            r3 = await c.get("/deps")
            assert r3.status_code == 200, r3.text
            assert r3.json() == {"dep_a": "dep_val_a", "dep_b": "dep_b:/deps"}

            # Pydantic validation
            r4 = await c.post("/pydantic", json={"name": "Widget", "price": 9.99})
            assert r4.status_code == 200, r4.text
            assert r4.json() == {"name": "Widget", "price": 9.99}

    asyncio.run(_run())
