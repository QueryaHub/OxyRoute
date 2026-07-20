import asyncio

import httpx
from oxyroute import App
from oxyroute.testing import asgi_test_app
from pydantic import BaseModel


class UserBody(BaseModel):
    name: str
    age: int


def test_body_model_validation_success() -> None:
    app = App()
    seen: dict[str, object] = {}

    @app.post("/user", body_model=UserBody)
    def create_user(json: UserBody) -> str:
        seen["user"] = json
        return json.name

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post("/user", json={"name": "Alice", "age": 30})
        assert r.status_code == 200, r.text
        assert r.text == "Alice"

    asyncio.run(_run())
    user = seen["user"]
    assert isinstance(user, UserBody)
    assert user.name == "Alice"
    assert user.age == 30


def test_body_model_validation_failure_422() -> None:
    app = App()

    @app.post("/user", body_model=UserBody)
    def create_user(json: UserBody) -> str:
        return json.name

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            # Missing 'age', should fail validation
            r = await c.post("/user", json={"name": "Bob"})

        assert r.status_code == 422
        data = r.json()
        assert "detail" in data
        assert isinstance(data["detail"], list)
        assert data["detail"][0]["type"] == "missing"
        assert data["detail"][0]["loc"] == ["age"]

    asyncio.run(_run())
