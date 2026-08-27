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


def test_auto_inferred_body_model_and_custom_param_name() -> None:
    app = App()
    seen: dict[str, object] = {}

    # Auto-infer UserBody from annotation, bound to param name 'user'
    @app.post("/users")
    def create_user(user: UserBody) -> dict[str, object]:
        seen["user"] = user
        return {"name": user.name, "age": user.age}

    # Explicit body_model with custom parameter name 'payload'
    @app.put("/users", body_model=UserBody)
    def update_user(payload: UserBody) -> dict[str, object]:
        seen["updated"] = payload
        return {"updated": payload.name}

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            # Test auto-inferred model
            r1 = await c.post("/users", json={"name": "Charlie", "age": 25})
            assert r1.status_code == 200, r1.text
            assert r1.json() == {"name": "Charlie", "age": 25}

            # Test 422 on auto-inferred model
            r2 = await c.post("/users", json={"name": "Invalid"})
            assert r2.status_code == 422

            # Test explicit model with custom parameter name
            r3 = await c.put("/users", json={"name": "David", "age": 40})
            assert r3.status_code == 200, r3.text
            assert r3.json() == {"updated": "David"}

            # Test OpenAPI schema auto-generated for auto-inferred route
            r_oa = await c.get("/openapi.json")
            assert r_oa.status_code == 200
            oa = r_oa.json()
            assert "/users" in oa["paths"]
            assert "post" in oa["paths"]["/users"]
            assert "requestBody" in oa["paths"]["/users"]["post"]

    asyncio.run(_run())
    u = seen["user"]
    assert isinstance(u, UserBody)
    assert u.name == "Charlie"
    assert u.age == 25

    up = seen["updated"]
    assert isinstance(up, UserBody)
    assert up.name == "David"
