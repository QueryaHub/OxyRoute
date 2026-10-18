"""
RSGI sync short-circuit: ``openapi`` / 404 / 405 / trivial matched routes return without
``future_into_py`` (see ``try_rsgi_sync_short_circuit``).
"""

from __future__ import annotations

import asyncio
import inspect
from types import SimpleNamespace
from typing import Any

from oxyroute import App


class _ProtoText:
    __slots__ = ("headers", "sent")

    def __init__(self) -> None:
        self.sent: list[tuple[int, str, str]] = []
        self.headers: list[list[tuple[str, str]]] = []

    async def __call__(self) -> bytes:
        return b""

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


def test_handle_rsgi_sync_route_with_path_param_short_circuits() -> None:
    """Issue #254: sync routes with path parameters should short-circuit synchronously."""
    app = App()

    @app.get("/items/:id")
    def get_item(id: int) -> str:
        return f"item:{id}:{type(id).__name__}"

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/items/123",
        query_string="",
        headers={},
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent and proto.sent[0][0] == 200
    assert proto.sent[0][1] == "item:123:int"


def test_handle_rsgi_sync_route_with_multiple_path_params() -> None:
    """Issue #254: sync routes with multiple path parameters."""
    app = App()

    @app.get("/users/:user_id/posts/:slug")
    def get_post(user_id: int, slug: str) -> str:
        return f"{user_id}/{slug}"

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/users/99/posts/hello-world",
        query_string="",
        headers={},
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent and proto.sent[0][0] == 200
    assert proto.sent[0][1] == "99/hello-world"


def test_handle_rsgi_sync_route_with_wildcard_param() -> None:
    """Issue #254: sync routes with wildcard catch-all parameter (*path)."""
    app = App()

    @app.get("/static/*filepath")
    def static_file(filepath: str) -> str:
        return f"file:{filepath}"

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/static/css/main.css",
        query_string="",
        headers={},
    )
    proto = _ProtoText()
    r = app.handle_rsgi(scope, proto)
    assert r is None
    assert not inspect.isawaitable(r)
    assert proto.sent and proto.sent[0][0] == 200
    assert proto.sent[0][1] == "file:css/main.css"


def test_handle_rsgi_sync_route_with_varkw_defers_to_async() -> None:
    """Issue #254: sync route with **kwargs defers to async to allow query/scope injection."""
    app = App()

    @app.get("/varkw/:id")
    def varkw_handler(**kwargs: Any) -> str:
        return f"id={kwargs['id']}"

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/varkw/abc",
        query_string="",
        headers={},
    )
    proto = _ProtoText()

    async def check() -> None:
        r = app.handle_rsgi(scope, proto)
        assert inspect.isawaitable(r)
        await r
        assert proto.sent and proto.sent[0][0] == 200
        assert proto.sent[0][1] == "id=abc"

    asyncio.run(check())


def test_handle_rsgi_sync_route_with_extra_query_param_defers_to_async() -> None:
    """Issue #254: sync route with non-path parameter defers to async."""
    app = App()

    @app.get("/search/:category")
    def search(category: str, q: str = "") -> str:
        return f"{category}:{q}"

    scope = SimpleNamespace(
        proto="http",
        method="GET",
        path="/search/books",
        query_string="",
        headers={},
    )
    proto = _ProtoText()

    async def check() -> None:
        r = app.handle_rsgi(scope, proto)
        assert inspect.isawaitable(r)
        await r
        assert proto.sent and proto.sent[0][0] == 200

    asyncio.run(check())
