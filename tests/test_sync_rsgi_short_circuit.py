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
    __slots__ = ("sent",)

    def __init__(self) -> None:
        self.sent: list[tuple[int, str, str]] = []

    def response_str(self, status: int, _headers: list, body: str) -> None:
        self.sent.append((status, body, "str"))

    def response_bytes(self, status: int, _headers: list, body: bytes) -> None:
        self.sent.append((status, body.decode("utf-8"), "bytes"))


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
