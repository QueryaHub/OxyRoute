"""Pre-route `set_middleware` short-circuits before body read (CORS preflight, issue #11)."""

from __future__ import annotations

import asyncio
import warnings

import httpx
from oxyroute import App, Response
from oxyroute.cors import CORSConfig, apply_cors
from oxyroute.csrf import CSRFConfig, apply_csrf
from oxyroute.testing import asgi_test_app


def test_middleware_cors_preflight_204_no_route_ran() -> None:
    n = 0

    def mw(scope, _protocol) -> Response | None:
        if scope.method == "OPTIONS" and scope.headers.get("access-control-request-method", ""):
            return Response(
                status=204,
                body=None,
                headers={"access-control-allow-origin": "*"},
            )
        return None

    app = App()
    app.set_middleware(mw)

    @app.post("/x")
    def _x() -> str:
        nonlocal n
        n += 1
        return "ok"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.request(
                "OPTIONS",
                "/x",
                headers={"access-control-request-method": "POST"},
            )
        assert r.status_code == 204, r.text
        assert (r.headers.get("access-control-allow-origin") or "") == "*"
        assert n == 0

    asyncio.run(_run())


def test_add_middleware_then_apply_cors_does_not_clobber_auth() -> None:
    """issue #209: add_middleware(auth) followed by apply_cors(...) must not silently
    delete the auth middleware."""
    calls: list[str] = []

    def auth_mw(scope, _protocol):  # type: ignore[no-untyped-def]
        calls.append("auth")
        return None

    app = App()
    app.add_middleware(auth_mw)
    apply_cors(app, CORSConfig())

    @app.get("/x")
    def _x() -> str:
        return "ok"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/x")
        assert r.status_code == 200, r.text

    asyncio.run(_run())
    assert calls == ["auth"]


def test_apply_csrf_then_apply_cors_compose_instead_of_clobbering() -> None:
    """issue #209: calling apply_csrf then apply_cors must not delete the CSRF guard."""
    app = App()
    apply_csrf(app, CSRFConfig())
    apply_cors(app, CORSConfig())

    @app.post("/x")
    def _x() -> str:
        return "ok"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            # unsafe method, no CSRF token -> the CSRF guard (still registered) must block it
            r = await c.post("/x")
        assert r.status_code == 403, r.text

    asyncio.run(_run())


def test_set_middleware_warns_when_clobbering_add_middleware() -> None:
    app = App()
    app.add_middleware(lambda scope, protocol: None)

    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        app.set_middleware(lambda scope, protocol: None)

    assert any("set_middleware" in str(x.message) for x in w)
