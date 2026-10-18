"""
RSGI sync short-circuit: ``openapi`` / 404 / 405 / trivial matched routes return without
``future_into_py`` (see ``try_rsgi_sync_short_circuit``).
"""

from __future__ import annotations

import asyncio
import inspect
from types import SimpleNamespace

from oxyroute import App


class _ProtoText:
    __slots__ = ("headers", "sent")

    def __init__(self) -> None:
        self.sent: list[tuple[int, str, str]] = []
        self.headers: list[list[tuple[str, str]]] = []

    def response_str(self, status: int, headers: list, body: str) -> None:
        self.sent.append((status, body, "str"))
        self.headers.append(headers)

    def response_bytes(self, status: int, headers: list, body: bytes) -> None:
        self.sent.append((status, body.decode("utf-8"), "bytes"))
        self.headers.append(headers)

    def response_empty(self, status: int, headers: list) -> None:
        self.sent.append((status, "", "empty"))
        self.headers.append(headers)


def test_handle_rsgi_openapi_get_returns_non_awaitable() -> None:
    app = App(title="Doc")
    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/openapi.json",
        query_string="",
        headers={},
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent and proto.sent[0][0] == 200
    assert "openapi" in proto.sent[0][1]


def test_handle_rsgi_404_returns_non_awaitable() -> None:
    app = App()
    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/no-such",
        query_string="",
        headers={},
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent[0][0] == 404
    assert "Not Found" in proto.sent[0][1]


def test_handle_rsgi_trivial_matched_route_returns_non_awaitable() -> None:
    app = App()

    @app.get("/hello")
    def hello() -> str:
        return "hello"

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/hello",
        query_string="",
        headers={},
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent and proto.sent[0][0] == 200 and proto.sent[0][1] == "hello"


def test_handle_rsgi_trivial_bytes_response() -> None:
    app = App()

    @app.get("/bin")
    def bin_route() -> bytes:
        return b"\x00\x01"

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/bin",
        query_string="",
        headers={},
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent[0][0] == 200
    assert proto.sent[0][1] == "\x00\x01"


def test_handle_rsgi_route_with_deps_still_awaitable() -> None:
    app = App()

    def dep() -> int:
        return 1

    @app.get("/x", dependencies=[("n", dep)])
    def x(n: int) -> str:
        return str(n)

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/x",
        query_string="",
        headers={},
    )
    proto = _ProtoText()

    async def check() -> None:
        r = app.handle_rsgi(scope, proto)
        assert inspect.isawaitable(r)
        await r

    asyncio.run(check())


def test_handle_rsgi_cors_and_security_headers_on_sync_path() -> None:
    """Issue #218: CORS and SecurityHeaders presets should not disable sync short-circuit."""
    from oxyroute import CORSConfig, SecurityHeadersConfig, apply_cors

    app = App()
    apply_cors(app, CORSConfig(allow_origins=["https://example.com"]))
    app.set_security_headers(SecurityHeadersConfig(x_frame_options="DENY"))

    @app.get("/sync-cors")
    def sync_route() -> str:
        return "fast"

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/sync-cors",
        query_string="",
        headers={"origin": "https://example.com"},
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent and proto.sent[0][0] == 200
    assert proto.sent[0][1] == "fast"
    hdrs = {k.lower(): v for k, v in proto.headers[0]}
    assert hdrs.get("access-control-allow-origin") == "https://example.com"
    assert hdrs.get("x-frame-options") == "DENY"


def test_handle_rsgi_cors_preflight_on_sync_path() -> None:
    """Issue #218: CORS preflight (OPTIONS with ACRM) handled synchronously."""
    from oxyroute import CORSConfig, apply_cors

    app = App()
    apply_cors(
        app, CORSConfig(allow_origins=["https://example.com"], allow_methods=["GET", "POST"])
    )

    scope = SimpleNamespace(
        proto="http",
        method="OPTIONS",
        path="/any",
        query_string="",
        headers={
            "origin": "https://example.com",
            "access-control-request-method": "POST",
        },
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent and proto.sent[0][0] == 204
    hdrs = {k.lower(): v for k, v in proto.headers[0]}
    assert hdrs.get("access-control-allow-origin") == "https://example.com"
    assert "POST" in hdrs.get("access-control-allow-methods", "")
