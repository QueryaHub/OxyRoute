"""Unit coverage for ASGI bridge helper internals."""

from __future__ import annotations

import asyncio

import oxyroute.asgi as asgi_mod
import pytest


def test_norm_headers_asgi_filters_invalid_pairs() -> None:
    out = asgi_mod._norm_headers_asgi(
        [
            ("A", "1"),
            (b"B", b"2"),
            ("C", b"3"),
            ("D", 4),
            ("bad",),
            "x",
        ]
    )
    assert out == [(b"A", b"1"), (b"B", b"2"), (b"C", b"3")]


def test_hdr_from_asgi_lowercases_and_get_lax() -> None:
    hv = asgi_mod._hdr_from_asgi([(b"Origin", b"https://a.example"), (b"X", b"1")])
    assert hv.get("origin") == "https://a.example"
    assert hv.get("ORIGIN") == "https://a.example"
    assert hv.get("missing", "d") == "d"


def test_rsgi_scope_shape() -> None:
    hv = asgi_mod._hdr_from_asgi([])
    s = asgi_mod._RsgiScope("http", "GET", "/x", "a=1", hv)
    assert s.proto == "http"
    assert s.http_version == "1.1"
    assert s.method == "GET"
    assert s.path == "/x"
    assert s.query_string == "a=1"
    assert s.headers.get("none", "") == ""


def test_websocket_send_and_close_idempotent() -> None:
    sent: list[dict] = []

    async def receive() -> dict:
        return {"type": "websocket.connect"}

    async def send(msg: dict) -> None:
        sent.append(msg)

    ws = asgi_mod.WebSocket(receive, send)

    async def _run() -> None:
        await ws.send_text("a")
        await ws.send_bytes(b"x")
        await ws.close(code=1001)
        await ws.send_text("ignored")
        await ws.close(code=1000)

    asyncio.run(_run())
    assert sent[0] == {"type": "websocket.send", "text": "a"}
    assert sent[1] == {"type": "websocket.send", "bytes": b"x"}
    assert sent[2] == {"type": "websocket.close", "code": 1001}
    assert len(sent) == 3


def test_websocket_receive_returns_raw_message() -> None:
    incoming = iter([{"type": "websocket.receive", "text": "ok"}])

    async def receive() -> dict:
        return next(incoming)

    async def send(_msg: dict) -> None:
        return None

    ws = asgi_mod.WebSocket(receive, send)
    out = asyncio.run(ws.receive())
    assert out["type"] == "websocket.receive"
    assert out["text"] == "ok"


def test_websocket_accept_subprotocol_and_idempotent_second_accept() -> None:
    sent: list[dict] = []
    incoming = iter(
        [
            {"type": "websocket.connect"},
            {"type": "websocket.receive", "text": "msg"},
        ]
    )

    async def receive() -> dict:
        return next(incoming)

    async def send(msg: dict) -> None:
        sent.append(msg)

    async def _run() -> None:
        ws = asgi_mod.WebSocket(receive, send)
        await ws.accept(subprotocol="chat")
        await ws.accept(subprotocol="chat")  # no-op
        assert await ws.receive_text() == "msg"

    asyncio.run(_run())
    assert sent == [{"type": "websocket.accept", "subprotocol": "chat"}]


def test_websocket_accept_raises_on_unexpected_pre_accept_event() -> None:
    async def receive() -> dict:
        return {"type": "websocket.receive", "text": "x"}

    async def send(_msg: dict) -> None:
        return None

    async def _run() -> None:
        ws = asgi_mod.WebSocket(receive, send)
        with pytest.raises(RuntimeError, match="unexpected websocket event before accept"):
            await ws.accept()

    asyncio.run(_run())


def test_websocket_receive_text_disconnect_raises() -> None:
    async def receive() -> dict:
        return {"type": "websocket.disconnect"}

    async def send(_msg: dict) -> None:
        return None

    async def _run() -> None:
        ws = asgi_mod.WebSocket(receive, send)
        with pytest.raises(RuntimeError, match="disconnected"):
            await ws.receive_text()

    asyncio.run(_run())


def test_rsgi_protocol_response_bytes_and_empty_enqueue() -> None:
    async def _run() -> None:
        loop = asyncio.get_running_loop()
        q: asyncio.Queue[dict | None] = asyncio.Queue()
        p = asgi_mod._RsgiProtocol(b"", q, loop)
        p.response_bytes(201, [("x", "1")], b"abc")
        p.response_empty(204, [])
        msg1 = await q.get()
        msg2 = await q.get()
        msg3 = await q.get()
        msg4 = await q.get()
        assert msg1 == {
            "type": "http.response.start",
            "status": 201,
            "headers": [(b"x", b"1")],
        }
        assert msg2 == {"type": "http.response.body", "body": b"abc"}
        assert msg3 == {"type": "http.response.start", "status": 204, "headers": []}
        assert msg4 == {"type": "http.response.body"}

    asyncio.run(_run())


def test_build_asgi_caller_non_callable_ws_handler_closes() -> None:
    class _Framework:
        async def handle_rsgi(self, _scope: object, _proto: object) -> None:
            return None

        _handle_asgi_websocket = "not-callable"

    app = asgi_mod.build_asgi_caller(_Framework())
    sent: list[dict] = []

    async def receive() -> dict:
        return {"type": "websocket.connect"}

    async def send(msg: dict) -> None:
        sent.append(msg)

    scope = {"type": "websocket", "path": "/x", "headers": [], "query_string": b""}
    asyncio.run(app(scope, receive, send))
    assert sent == [{"type": "websocket.close", "code": 1000}]


def test_build_asgi_caller_uses_inner_app_when_handle_rsgi_missing() -> None:
    called: list[str] = []

    class _Inner:
        def handle_rsgi(self, _scope: object, _proto: object) -> object:
            called.append("inner")

            async def _noop() -> None:
                return None

            return _noop()

    class _Framework:
        def __init__(self) -> None:
            self._app = _Inner()

    app = asgi_mod.build_asgi_caller(_Framework())
    sent: list[dict] = []
    incoming = iter([{"type": "http.request", "body": b"", "more_body": False}])

    async def receive() -> dict:
        return next(incoming)

    async def send(msg: dict) -> None:
        sent.append(msg)

    scope = {
        "type": "http",
        "http_version": "1.1",
        "scheme": "http",
        "method": "GET",
        "path": "/x",
        "query_string": b"",
        "headers": [],
    }
    asyncio.run(app(scope, receive, send))
    assert called == ["inner"]
