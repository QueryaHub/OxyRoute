"""CSRF double-submit (issue #53)."""

from __future__ import annotations

import asyncio
import json

import httpx
from oxyroute import App
from oxyroute.csrf import CSRFConfig, apply_csrf
from oxyroute.testing import asgi_test_app

_HDR = "X-CSRF-Token"
CK = "oxyroute_csrf"
TOK = "known-test-token"  # not secret strength; pre-route compare only


def test_csrf_mismatch_403() -> None:
    cfg = CSRFConfig(cookie_name=CK, header_name=_HDR)
    app = App()
    apply_csrf(app, cfg)

    @app.post("/p")
    def _p() -> str:
        return "ok"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post(
                "/p",
                headers={_HDR: "wrong", "Cookie": f"{CK}={TOK}"},
            )
        assert r.status_code == 403, r.text
        j = json.loads(r.text)
        assert j.get("error") == "csrf"
        assert j.get("detail") == "mismatch"

    asyncio.run(_run())


def test_csrf_missing_403() -> None:
    cfg = CSRFConfig(cookie_name=CK, header_name=_HDR)
    app = App()
    apply_csrf(app, cfg)

    @app.post("/p")
    def _p2() -> str:
        return "nope"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post("/p")
        assert r.status_code == 403
        j = json.loads(r.text)
        assert j.get("detail") == "missing"

    asyncio.run(_run())


def test_csrf_ok_when_cookie_and_header_match() -> None:
    cfg = CSRFConfig(cookie_name=CK, header_name=_HDR)
    app = App()
    apply_csrf(app, cfg)

    @app.post("/p")
    def _p3() -> str:
        return "yes"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post(
                "/p",
                headers={_HDR: TOK, "Cookie": f"{CK}={TOK}"},
            )
        assert r.status_code == 200
        assert r.text == "yes"

    asyncio.run(_run())
