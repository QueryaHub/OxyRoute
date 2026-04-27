"""RS256 (PEM public key) on the Rust JWT path (issue #8)."""

from __future__ import annotations

import asyncio
import time
from pathlib import Path

import httpx
import jwt
import pytest
from oxyroute import App
from tests._rsgi_test_transport import asgi_test_app

_FIX = Path(__file__).resolve().parent / "fixtures" / "rsa"
_PUB = (_FIX / "public_pkcs8.pem").read_text()
_PRIV = (_FIX / "private_pkcs8.pem").read_text()


def test_asgi_jwt_rs256_bearer() -> None:
    now = int(time.time())
    tok = jwt.encode(
        {"sub": "u-rs", "exp": now + 3600},
        _PRIV,
        algorithm="RS256",
    )
    assert isinstance(tok, str)
    app = App()

    @app.get("/r", require_jwt=True, jwt_secret=_PUB, algorithms=["RS256"])
    def r(claims: object) -> str:
        c = dict(claims)  # type: ignore[arg-type]
        return str(c.get("sub", ""))

    async def _go() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            o = await c.get("/r", headers={"Authorization": f"Bearer {tok}"})
        assert o.status_code == 200
        assert o.text == "u-rs"

    asyncio.run(_go())


def test_add_route_rejects_mixed_key_and_algorithms() -> None:
    app = App()
    with pytest.raises(ValueError, match="incompatible"):

        @app.get(
            "/x",
            require_jwt=True,
            jwt_secret="short-hmac",
            algorithms=["RS256"],
        )
        def _x() -> str:  # pragma: no cover
            return "n"
