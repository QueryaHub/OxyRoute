from __future__ import annotations

import asyncio
from typing import Any

from oxyroute import App, Response


class _MockProtocol:
    def __init__(self) -> None:
        self.status = 200
        self.headers: list[tuple[str, str]] = []
        self.body: bytes | str | None = None

    def response_str(self, status: int, headers: list[tuple[str, str]], body: str) -> None:
        self.status = status
        self.headers = headers
        self.body = body

    def response_bytes(self, status: int, headers: list[tuple[str, str]], body: bytes) -> None:
        self.status = status
        self.headers = headers
        self.body = body

    def response_empty(self, status: int, headers: list[tuple[str, str]]) -> None:
        self.status = status
        self.headers = headers


class _MockScope:
    def __init__(self, method: str, path: str) -> None:
        self.proto = "http"
        self.http_version = "1.1"
        self.rsgi_version = "1.0"
        self.scheme = "http"
        self.method = method
        self.path = path
        self.query_string = ""
        self.headers = {}
        self.authority = "localhost"
        self.client = "127.0.0.1:54321"


def test_access_log_hook_on_success() -> None:
    logs: list[tuple[Any, int, float, str]] = []

    def log_hook(scope: Any, status: int, duration_ms: float, template: str) -> None:
        logs.append((scope, status, duration_ms, template))

    app = App(access_log_hook=log_hook)

    @app.get("/users/:id")
    def get_user(id: str) -> Response:
        return Response(body=f"user {id}", status=200)

    scope = _MockScope("GET", "/users/42")
    proto = _MockProtocol()

    async def _run() -> None:
        await app.__rsgi__(scope, proto)

    asyncio.run(_run())

    assert len(logs) == 1
    s, status, dur, template = logs[0]
    assert s.path == "/users/42"
    assert status == 200
    assert dur >= 0.0
    assert template == "/users/:id"


def test_access_log_hook_on_unhandled_exception() -> None:
    logs: list[tuple[Any, int, float, str]] = []

    def log_hook(scope: Any, status: int, duration_ms: float, template: str) -> None:
        logs.append((scope, status, duration_ms, template))

    app = App(access_log_hook=log_hook)

    @app.get("/crash")
    def crash() -> None:
        raise RuntimeError("boom")

    scope = _MockScope("GET", "/crash")
    proto = _MockProtocol()

    async def _run() -> None:
        await app.__rsgi__(scope, proto)

    asyncio.run(_run())

    assert len(logs) == 1
    s, status, dur, _ = logs[0]
    assert s.path == "/crash"
    # Status defaults to 500 on unhandled error
    assert status == 500
    assert dur >= 0.0


def test_access_log_hook_with_json_body_route() -> None:
    logs: list[tuple[Any, int, float, str]] = []

    def log_hook(scope: Any, status: int, duration_ms: float, template: str) -> None:
        logs.append((scope, status, duration_ms, template))

    app = App(access_log_hook=log_hook)

    @app.post("/items")
    def create_item(json: dict) -> dict:
        return {"received": json["name"]}

    class _CallableProtocol(_MockProtocol):
        async def __call__(self) -> bytes:
            return b'{"name": "oxy"}'

    scope = _MockScope("POST", "/items")
    proto = _CallableProtocol()

    async def _run() -> None:
        await app.__rsgi__(scope, proto)

    asyncio.run(_run())

    assert len(logs) == 1
    s, status, dur, template = logs[0]
    assert s.path == "/items"
    assert status == 200
    assert dur >= 0.0
    assert template == "/items"
    assert proto.status == 200
    assert proto.body == b'{"received": "oxy"}'


def test_access_log_hook_with_websocket_route() -> None:
    from oxyroute._oxyroute import WebSocket

    logs: list[tuple[Any, int, float, str]] = []

    def log_hook(scope: Any, status: int, duration_ms: float, template: str) -> None:
        logs.append((scope, status, duration_ms, template))

    app = App(access_log_hook=log_hook)

    @app.websocket("/ws/:room")
    async def ws_handler(ws: WebSocket) -> None:
        await ws.accept()
        await ws.close(1000)

    class _MockWsTransport:
        async def receive(self) -> Any:
            class Msg:
                kind = 1
                data = "hello"

            return Msg()

        async def send_str(self, s: str) -> None:
            pass

    class _MockWsProtocol:
        def __init__(self) -> None:
            self.accepted = False
            self.closed_code: int | None = None

        async def accept(self) -> _MockWsTransport:
            self.accepted = True
            return _MockWsTransport()

        def close(self, code: int = 1000) -> None:
            self.closed_code = code

    scope = _MockScope("GET", "/ws/chat")
    scope.proto = "websocket"
    proto = _MockWsProtocol()

    async def _run() -> None:
        await app.__rsgi__(scope, proto)

    asyncio.run(_run())

    assert len(logs) == 1
    _, status, dur, template = logs[0]
    assert status == 101
    assert dur >= 0.0
    assert template == ""
    assert proto.accepted


def test_protocol_wrapper_async_iterator_and_delegation() -> None:
    from oxyroute.app import _ProtocolWrapper

    class _Inner:
        def __init__(self) -> None:
            self.range_called = False
            self.custom_val = 42

        def response_file_range(
            self,
            status: int,
            headers: list[tuple[str, str]],
            file_path: str,
            offset: int,
            count: int,
        ) -> None:
            self.range_called = True

        async def __aiter__(self) -> Any:
            yield b"chunk1"
            yield b"chunk2"

    inner = _Inner()
    wrapper = _ProtocolWrapper(inner)

    async def _test_aiter() -> list[bytes]:
        chunks = []
        async for c in wrapper:
            chunks.append(c)
        return chunks

    result = asyncio.run(_test_aiter())
    assert result == [b"chunk1", b"chunk2"]

    # Test file range
    wrapper.response_file_range(206, [], "file.txt", 0, 10)
    assert wrapper.status == 206
    assert inner.range_called

    # Test getattr delegation
    assert wrapper.custom_val == 42
