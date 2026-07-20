"""CORS preflight and cross-origin response headers (issue #49)."""

from __future__ import annotations

import asyncio

import httpx
from oxyroute import App, CORSConfig, apply_cors
from oxyroute.testing import asgi_test_app


def test_cors_preflight_204_allows_post() -> None:
    n = 0

    app = App()
    apply_cors(app, CORSConfig(allow_origins=["https://app.example"]))

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
                headers={
                    "origin": "https://app.example",
                    "access-control-request-method": "POST",
                },
            )
        assert r.status_code == 204, r.text
        assert r.headers.get("access-control-allow-origin") == "https://app.example"
        assert "POST" in (r.headers.get("access-control-allow-methods") or "")
        assert n == 0

    asyncio.run(_run())


def test_cors_get_with_origin_merges_headers() -> None:
    app = App()
    apply_cors(app, CORSConfig(allow_origins=["https://a.example", "https://b.example"]))

    @app.get("/hi")
    def _hi() -> str:
        return "hello"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/hi", headers={"origin": "https://a.example"})
        assert r.status_code == 200
        assert r.text == "hello"
        assert r.headers.get("access-control-allow-origin") == "https://a.example"

    asyncio.run(_run())


class _CountingCors(CORSConfig):
    """Tracks ``response_header_pairs`` calls from the native layer (issue #108)."""

    pairs_calls: int

    def __init__(self, **kwargs: object) -> None:
        super().__init__(**kwargs)  # type: ignore[arg-type]
        self.pairs_calls = 0

    def response_header_pairs(self, scope: object) -> list[tuple[str, str]]:
        self.pairs_calls += 1
        return super().response_header_pairs(scope)


def test_cors_without_origin_skips_python_pairs_call() -> None:
    cfg = _CountingCors(allow_origins=["https://app.example"])
    app = App()
    apply_cors(app, cfg)

    @app.get("/n")
    def _n() -> str:
        return "ok"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.get("/n")
        assert r.status_code == 200
        assert r.text == "ok"
        assert r.headers.get("access-control-allow-origin") is None

    asyncio.run(_run())
    assert cfg.pairs_calls == 0


def test_cors_wildcard_without_origin_skips_pairs_with_origin_calls() -> None:
    cfg = _CountingCors(allow_origins=["*"], allow_credentials=False)
    app = App()
    apply_cors(app, cfg)

    @app.get("/w")
    def _w() -> str:
        return "w"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            bare = await c.get("/w")
            starred = await c.get("/w", headers={"origin": "https://any.example"})
        assert bare.status_code == 200
        assert bare.headers.get("access-control-allow-origin") is None
        assert starred.status_code == 200
        assert starred.headers.get("access-control-allow-origin") == "*"

    asyncio.run(_run())
    assert cfg.pairs_calls == 1


def test_cors_credentials_with_origin_still_merges() -> None:
    cfg = _CountingCors(
        allow_origins=["https://app.example"],
        allow_credentials=True,
    )
    app = App()
    apply_cors(app, cfg)

    @app.get("/c")
    def _c() -> str:
        return "c"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            no_o = await c.get("/c")
            with_o = await c.get("/c", headers={"origin": "https://app.example"})
        assert no_o.headers.get("access-control-allow-origin") is None
        assert with_o.headers.get("access-control-allow-origin") == "https://app.example"
        assert with_o.headers.get("access-control-allow-credentials") == "true"

    asyncio.run(_run())
    assert cfg.pairs_calls == 1
