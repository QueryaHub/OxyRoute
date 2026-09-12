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
