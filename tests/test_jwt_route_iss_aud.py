"""Route-level JWT: issuer, audience, leeway (P1 #6)."""

from __future__ import annotations

import asyncio
import time

import httpx
import pytest
from oxyroute import App
from tests._rsgi_test_transport import asgi_test_app

oxyjwt = pytest.importorskip("oxyjwt")

SECRET = "test-secret-iss-aud"
ISS = "https://issuer.example"
AUD = "expected-aud"


def _token(**claims: object) -> str:
    return oxyjwt.encode(claims, SECRET, algorithm="HS256")  # type: ignore[arg-type]


def test_jwt_route_iss_and_aud_match() -> None:
    now = int(time.time())
    app = App()

    @app.get(
        "/ok",
        require_jwt=True,
        jwt_secret=SECRET,
        algorithms=["HS256"],
        jwt_issuer=ISS,
        jwt_audience=AUD,
    )
    def ok(claims: dict) -> str:
        return "yes"

    tok = _token(sub="u1", iss=ISS, aud=AUD, exp=now + 3600)

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/ok", headers={"Authorization": f"Bearer {tok}"})
        assert r.status_code == 200
        assert r.text == "yes"

    asyncio.run(run())


def test_jwt_route_wrong_iss_401() -> None:
    now = int(time.time())
    app = App()

    @app.get(
        "/p",
        require_jwt=True,
        jwt_secret=SECRET,
        algorithms=["HS256"],
        jwt_issuer=ISS,
        jwt_audience=AUD,
    )
    def p(claims: dict) -> str:
        return "x"

    tok = _token(sub="u1", iss="https://other", aud=AUD, exp=now + 3600)

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/p", headers={"Authorization": f"Bearer {tok}"})
        assert r.status_code == 401

    asyncio.run(run())


def test_jwt_route_wrong_aud_401() -> None:
    now = int(time.time())
    app = App()

    @app.get(
        "/p",
        require_jwt=True,
        jwt_secret=SECRET,
        algorithms=["HS256"],
        jwt_issuer=ISS,
        jwt_audience=AUD,
    )
    def p(claims: dict) -> str:
        return "x"

    tok = _token(sub="u1", iss=ISS, aud="other-aud", exp=now + 3600)

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/p", headers={"Authorization": f"Bearer {tok}"})
        assert r.status_code == 401

    asyncio.run(run())
