"""Unit tests for the native RSGI WebSocket dispatch path (no ASGI involved).

Drives :meth:`oxyroute.app.App.handle_rsgi` directly with a mocked Granian-style
``scope`` / ``protocol`` pair (see :class:`granian.rsgi.RSGIWebsocketProtocol` /
:class:`granian.rsgi.RSGIWebsocketTransport` for the live equivalents). The mocks
return real ``asyncio`` coroutines so ``pyo3-async-runtimes`` can bridge them
to Rust the same way Granian does at runtime.
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Any

import pytest
from oxyroute import App, WebSocket


@dataclass
class _Headers:
    pairs: list[tuple[str, str]] = field(default_factory=list)

    def get(self, key: str, default: Any = None) -> Any:
        for k, v in self.pairs:
            if k.lower() == key.lower():
                return v
        return default


@dataclass
class _WSScope:
    proto: str = "websocket"
    path: str = "/"
    method: str = "GET"
    query_string: str = ""
    http_version: str = "1.1"
    rsgi_version: str = "1.5"
    server: str = "127.0.0.1:8000"
    client: str = "127.0.0.1:1234"
    scheme: str = "http"
    authority: str | None = None
    headers: _Headers = field(default_factory=_Headers)


class _MockMessage:
    """Mirrors :class:`granian.rsgi.WebsocketMessage` (kind / data)."""

    __slots__ = ("data", "kind")

    def __init__(self, kind: int, data: Any) -> None:
        self.kind = kind
        self.data = data


class _MockTransport:
    """Stand-in for :class:`granian.rsgi.RSGIWebsocketTransport`."""

    def __init__(self, incoming: list[_MockMessage]) -> None:
        self._incoming = list(incoming)
        self.sent: list[tuple[str, Any]] = []

    async def receive(self) -> _MockMessage:
        if not self._incoming:
            return _MockMessage(0, b"")
        return self._incoming.pop(0)

    async def send_str(self, data: str) -> None:
        self.sent.append(("text", data))

    async def send_bytes(self, data: bytes) -> None:
        self.sent.append(("bytes", bytes(data)))


class _MockProtocol:
    """Stand-in for :class:`granian.rsgi.RSGIWebsocketProtocol`."""

    def __init__(self, incoming: list[_MockMessage] | None = None) -> None:
        self._incoming = incoming or []
        self.transport: _MockTransport | None = None
        self.closed_with: int | None = None

    async def accept(self) -> _MockTransport:
        self.transport = _MockTransport(self._incoming)
        return self.transport

    def close(self, status: int | None) -> tuple[int, bool]:
        self.closed_with = int(status) if status is not None else 1000
        return (self.closed_with, True)


def _drive(app: App, scope: Any, proto: Any) -> None:
    """Run ``app.handle_rsgi(scope, proto)`` to completion on a fresh event loop.

    ``pyo3-async-runtimes`` needs an active asyncio loop *at the moment* the Rust
    side calls ``future_into_py``, so we always invoke ``handle_rsgi`` from inside
    a coroutine rather than synchronously from the test body.
    """

    async def _inner() -> None:
        await app.handle_rsgi(scope, proto)

    asyncio.run(_inner())


def test_websocket_decorator_registers_route() -> None:
    app = App()

    @app.websocket("/ws")
    async def handler(ws: WebSocket) -> None:
        await ws.accept()
        await ws.close()

    assert handler.__name__ == "handler"


def test_websocket_echo_round_trip() -> None:
    app = App()

    @app.websocket("/ws/:room")
    async def echo(ws: WebSocket) -> None:
        assert ws.path_params["room"] == "lobby"
        await ws.accept()
        msg = await ws.receive_text()
        await ws.send_text(f"echo:{msg}")
        await ws.send_bytes(b"binary")
        await ws.close()

    proto = _MockProtocol(incoming=[_MockMessage(2, "hello")])
    _drive(app, _WSScope(path="/ws/lobby"), proto)

    assert proto.transport is not None
    assert proto.transport.sent == [("text", "echo:hello"), ("bytes", b"binary")]
    assert proto.closed_with == 1000


def test_websocket_send_json() -> None:
    app = App()

    @app.websocket("/json")
    async def push_json(ws: WebSocket) -> None:
        await ws.accept()
        await ws.send_json({"a": 1, "b": [2, 3]})
        await ws.close()

    proto = _MockProtocol()
    _drive(app, _WSScope(path="/json"), proto)

    assert proto.transport is not None
    assert proto.transport.sent[0][0] == "text"
    import json

    assert json.loads(proto.transport.sent[0][1]) == {"a": 1, "b": [2, 3]}


def test_websocket_no_route_closes_polite() -> None:
    app = App()
    proto = _MockProtocol()
    _drive(app, _WSScope(path="/missing"), proto)
    assert proto.closed_with == 1000
    assert proto.transport is None


def test_websocket_handler_error_closes_with_1011() -> None:
    app = App()

    @app.websocket("/boom")
    async def boom(ws: WebSocket) -> None:
        await ws.accept()
        raise RuntimeError("boom")

    proto = _MockProtocol()
    _drive(app, _WSScope(path="/boom"), proto)
    assert proto.closed_with == 1011


def test_websocket_peer_close_marks_closed() -> None:
    """When the peer sends a close frame, ``receive_text`` raises and our side
    must not redundantly call ``protocol.close()`` (Granian handled the close).
    """

    app = App()
    after_state: dict[str, Any] = {}

    @app.websocket("/peer-close")
    async def peer_close(ws: WebSocket) -> None:
        await ws.accept()
        with pytest.raises(RuntimeError, match="closed by peer"):
            await ws.receive_text()
        after_state["is_closed"] = ws.is_closed
        await ws.close()

    proto = _MockProtocol(incoming=[_MockMessage(0, b"")])
    _drive(app, _WSScope(path="/peer-close"), proto)
    assert after_state["is_closed"] is True
    assert proto.closed_with is None


def test_websocket_kind_mismatch_raises_value_error() -> None:
    app = App()

    @app.websocket("/strict-text")
    async def strict_text(ws: WebSocket) -> None:
        await ws.accept()
        with pytest.raises(ValueError):
            await ws.receive_text()
        await ws.close()

    proto = _MockProtocol(incoming=[_MockMessage(1, b"binary-not-text")])
    _drive(app, _WSScope(path="/strict-text"), proto)


def test_websocket_send_before_accept_raises() -> None:
    app = App()

    @app.websocket("/no-accept")
    async def no_accept(ws: WebSocket) -> None:
        with pytest.raises(RuntimeError, match="accept"):
            await ws.send_text("nope")
        await ws.close()

    proto = _MockProtocol()
    _drive(app, _WSScope(path="/no-accept"), proto)


def test_websocket_frozen_app_rejects_route() -> None:
    app = App()
    app.freeze()
    with pytest.raises(ValueError, match="frozen"):

        @app.websocket("/late")
        async def late(ws: WebSocket) -> None: ...
