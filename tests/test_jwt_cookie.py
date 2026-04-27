"""JWT from Cookie when Bearer is missing (issue #7)."""

from __future__ import annotations

import asyncio
import time

import httpx

from tests._rsgi_test_transport import asgi_test_app
import pytest
from oxyroute import App

oxyjwt = pytest.importorskip("oxyjwt")

SECRET = "ck-secret"
COOKIE_NAME = "access_token"


def _token() -> str:
    now = int(time.time())
    return oxyjwt.encode(  # type: ignore[no-untyped-call]
        {"sub": "c1", "exp": now + 3600},
        SECRET,
        algorithm="HS256",
    )


def test_jwt_from_cookie() -> None:
    app = App()

    @app.get(
        "/c",
        require_jwt=True,
        jwt_secret=SECRET,
        algorithms=["HS256"],
        jwt_cookie=COOKIE_NAME,
    )
    def c(claims: dict) -> str:
        return claims.get("sub", "")

    tok = _token()

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as client:
            r = await client.get(
                "/c",
                headers={"Cookie": f"other=1; {COOKIE_NAME}={tok}"},
            )
        assert r.status_code == 200, r.text
        assert r.text == "c1"

    asyncio.run(run())


def test_bearer_wins_over_cookie() -> None:
    app = App()

    @app.get(
        "/b",
        require_jwt=True,
        jwt_secret=SECRET,
        algorithms=["HS256"],
        jwt_cookie=COOKIE_NAME,
    )
    def b(claims: dict) -> str:
        return str(claims.get("sub", ""))

    now = int(time.time())
    bearer_tok = oxyjwt.encode(  # type: ignore[no-untyped-call]
        {"sub": "from-bearer", "exp": now + 3600},
        SECRET,
        algorithm="HS256",
    )
    cook_tok = oxyjwt.encode(  # type: ignore[no-untyped-call]
        {"sub": "from-cookie", "exp": now + 3600},
        SECRET,
        algorithm="HS256",
    )

    async def run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as client:
            r = await client.get(
                "/b",
                headers={
                    "Authorization": f"Bearer {bearer_tok}",
                    "Cookie": f"{COOKIE_NAME}={cook_tok}",
                },
            )
        assert r.status_code == 200
        assert r.text == "from-bearer"

    asyncio.run(run())
