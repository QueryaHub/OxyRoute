"""Issue #71: 404/405 should fail before request body read on RSGI path."""

from __future__ import annotations

import asyncio
from types import SimpleNamespace
from typing import Any

from oxyroute import App


class _ProbeProtocol:
    def __init__(self) -> None:
        self.body_calls = 0
        self.sent: list[tuple[str, int, Any]] = []

    async def __call__(self) -> bytes:
        self.body_calls += 1
        return b'{"large":"payload"}'

    def response_str(self, status: int, _headers: list, body: str) -> None:
        self.sent.append(("str", int(status), body))

    def response_bytes(self, status: int, _headers: list, body: bytes) -> None:
        self.sent.append(("bytes", int(status), body))

    def response_empty(self, status: int, _headers: list) -> None:
        self.sent.append(("empty", int(status), b""))


def _scope(method: str, path: str) -> Any:
    return SimpleNamespace(
        proto="http",
        method=method,
        path=path,
        query_string="",
        headers={},
    )


def test_rsgi_404_does_not_consume_body() -> None:
    app = App()

    @app.get("/exists")
    def exists() -> str:
        return "ok"

    async def _run() -> None:
        proto = _ProbeProtocol()
        await app.__rsgi__(_scope("POST", "/missing"), proto)
        assert proto.body_calls == 0
        assert proto.sent
        assert proto.sent[-1][1] == 404

    asyncio.run(_run())


def test_rsgi_405_does_not_consume_body() -> None:
    app = App()

    @app.post("/x")
    def x() -> str:
        return "ok"

    async def _run() -> None:
        proto = _ProbeProtocol()
        await app.__rsgi__(_scope("GET", "/x"), proto)
        assert proto.body_calls == 0
        assert proto.sent
        assert proto.sent[-1][1] == 405

    asyncio.run(_run())


def test_rsgi_matched_get_without_body_param_skips_body_read() -> None:
    app = App()

    @app.get("/ok")
    def ok() -> str:
        return "ok"

    async def _run() -> None:
        proto = _ProbeProtocol()
        await app.__rsgi__(_scope("GET", "/ok"), proto)
        assert proto.body_calls == 0
        assert proto.sent
        assert proto.sent[-1][1] == 200

    asyncio.run(_run())


def test_rsgi_reads_body_when_handler_accepts_body_param() -> None:
    app = App()

    @app.post("/echo", read_json_body=False)
    def echo(body: bytes) -> str:
        return str(len(body))

    async def _run() -> None:
        proto = _ProbeProtocol()
        await app.__rsgi__(_scope("POST", "/echo"), proto)
        assert proto.body_calls == 1
        assert proto.sent
        assert proto.sent[-1][1] == 200

    asyncio.run(_run())


def test_rsgi_body_bytes_zero_copy_identity() -> None:
    """Issue #219: PyBytes body should be passed directly without copying."""
    app = App()
    received_body: bytes | None = None
    expected_payload = b"distinct_payload_for_zero_copy_test"

    class _CustomProtocol(_ProbeProtocol):
        async def __call__(self) -> bytes:
            self.body_calls += 1
            return expected_payload

    @app.post("/raw", read_json_body=False)
    def raw_handler(body: bytes) -> str:
        nonlocal received_body
        received_body = body
        return "ok"

    async def _run() -> None:
        proto = _CustomProtocol()
        await app.__rsgi__(_scope("POST", "/raw"), proto)
        assert proto.body_calls == 1
        assert received_body == expected_payload
        assert received_body is expected_payload

    asyncio.run(_run())
