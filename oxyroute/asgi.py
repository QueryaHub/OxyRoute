"""
Optional ASGI 3.0 entry for ``uvicorn`` / ``granian --interface asgi``.

Builds a minimal RSGI-like ``scope`` / ``protocol`` and delegates to the same Rust
``handle_rsgi`` coroutine as ``__rsgi__``. Only ``type == "http"`` is supported.
"""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from typing import Any


class WebSocket:
    """Small ASGI websocket helper used by the optional ASGI bridge."""

    __slots__ = ("_accepted", "_closed", "_connected", "_receive", "_send")

    def __init__(self, receive: Any, send: Any) -> None:
        self._receive = receive
        self._send = send
        self._accepted = False
        self._closed = False
        self._connected = False

    async def accept(self, subprotocol: str | None = None) -> None:
        if self._accepted:
            return
        if not self._connected:
            first = await self._receive()
            t = first.get("type", "")
            if t == "websocket.disconnect":
                self._closed = True
                raise RuntimeError("websocket disconnected before accept")
            if t != "websocket.connect":
                raise RuntimeError(f"unexpected websocket event before accept: {t}")
            self._connected = True
        msg: dict[str, Any] = {"type": "websocket.accept"}
        if subprotocol is not None:
            msg["subprotocol"] = subprotocol
        await self._send(msg)
        self._accepted = True

    async def receive(self) -> dict[str, Any]:
        return await self._receive()

    async def receive_text(self) -> str:
        msg = await self._receive()
        t = msg.get("type", "")
        if t == "websocket.disconnect":
            raise RuntimeError("websocket disconnected")
        if t != "websocket.receive":
            raise RuntimeError(f"unexpected websocket event: {t}")
        text = msg.get("text")
        if text is None:
            raise RuntimeError("expected text websocket frame")
        return str(text)

    async def send_text(self, text: str) -> None:
        if self._closed:
            return
        await self._send({"type": "websocket.send", "text": text})

    async def send_bytes(self, data: bytes) -> None:
        if self._closed:
            return
        await self._send({"type": "websocket.send", "bytes": data})

    async def close(self, code: int = 1000) -> None:
        if self._closed:
            return
        await self._send({"type": "websocket.close", "code": int(code)})
        self._closed = True


def _run_handle_rsgi_blocking(
    app_rsgi: Callable[[Any, Any], Any],
    rscope: Any,
    proto: Any,
) -> None:
    """Run ``handle_rsgi`` to completion on a **fresh** event loop in the thread pool.

    The ASGI server's main loop must **not** ``await`` the native coroutine directly: the
    Rust/Tokio side calls synchronous ``protocol.response_*`` which use
    ``run_coroutine_threadsafe(..., self._loop).result()`` targeting the **main** loop.
    If the main loop is blocked in ``await handle_rsgi``, it can never run those ``send``
    coroutines — a classic deadlock. Running the await in ``run_in_executor`` + ``asyncio.run``
    keeps the main loop free to drain the threadsafe queue.
    """

    async def _inner() -> None:
        c = app_rsgi(rscope, proto)
        await c

    asyncio.run(_inner())


def _hdr_from_asgi(raw: list[tuple[bytes, bytes]]) -> _HeaderView:
    d: dict[str, str] = {}
    for k, v in raw:
        dk = k.decode("latin-1").lower()
        d[dk] = v.decode("latin-1")
    return _HeaderView(d)


class _HeaderView:
    __slots__ = ("_d",)

    def __init__(self, d: dict[str, str]) -> None:
        self._d = d

    def get(self, k: str, default: str = "") -> str:
        return self._d.get(k.lower(), default)


class _RsgiScope:
    __slots__ = (
        "authority",
        "client",
        "headers",
        "http_version",
        "method",
        "path",
        "proto",
        "query_string",
        "rsgi_version",
        "scheme",
        "server",
    )

    def __init__(
        self,
        scheme: str,
        method: str,
        path: str,
        query_string: str,
        headers: _HeaderView,
    ) -> None:
        self.proto = "http"
        self.http_version = "1.1"
        self.rsgi_version = "1.0"
        self.server = ""
        self.client = ""
        self.scheme = scheme
        self.method = method
        self.path = path
        self.query_string = query_string
        self.headers = headers
        self.authority: str | None = None


def _norm_headers_asgi(
    rsgi_headers: list,
) -> list[tuple[bytes, bytes]]:
    out: list[tuple[bytes, bytes]] = []
    for p in rsgi_headers:
        if not isinstance(p, (list, tuple)) or len(p) != 2:
            continue
        a, b = p[0], p[1]
        if isinstance(a, str):
            a = a.encode("utf-8")
        if isinstance(b, str):
            b = b.encode("utf-8")
        if isinstance(a, (bytes, bytearray)) and isinstance(b, (bytes, bytearray)):
            out.append((bytes(a), bytes(b)))
    return out


class _RsgiProtocol:
    __slots__ = ("_body", "_loop", "_queue", "_status", "status")

    def __init__(
        self,
        body: bytes,
        queue: asyncio.Queue[dict[str, Any] | None],
        main_loop: asyncio.AbstractEventLoop,
    ) -> None:
        self._body = body
        self._queue = queue
        self._loop = main_loop
        self._status: int | None = None
        self.status = 200

    def _enqueue(self, message: dict[str, Any]) -> None:
        self._loop.call_soon_threadsafe(self._queue.put_nowait, message)

    def __call__(self) -> Any:
        async def _body() -> bytes:
            return self._body

        return _body()

    def response_str(self, status: int, headers: list, body: str) -> None:
        self._status = int(status)
        self.status = int(status)
        b = body.encode("utf-8")
        h = _norm_headers_asgi(headers)
        self._enqueue(
            {
                "type": "http.response.start",
                "status": int(status),
                "headers": h,
            }
        )
        self._enqueue(
            {
                "type": "http.response.body",
                "body": b,
            }
        )

    def response_bytes(self, status: int, headers: list, body: bytes) -> None:
        self._status = int(status)
        self.status = int(status)
        h = _norm_headers_asgi(headers)
        self._enqueue(
            {
                "type": "http.response.start",
                "status": int(status),
                "headers": h,
            }
        )
        self._enqueue(
            {
                "type": "http.response.body",
                "body": body,
            }
        )

    def response_empty(self, status: int, headers: list) -> None:
        self._status = int(status)
        self.status = int(status)
        h = _norm_headers_asgi(headers)
        self._enqueue(
            {
                "type": "http.response.start",
                "status": int(status),
                "headers": h,
            }
        )
        self._enqueue(
            {
                "type": "http.response.body",
            }
        )


async def asgi_to_rsgi(
    app_rsgi: Callable[[Any, Any], Any],
    app_ws: Callable[[dict[str, Any], Any, Any], Any] | None,
    scope: dict[str, Any],
    receive: Any,
    send: Any,
) -> None:
    st = scope.get("type")
    if st == "websocket":
        if app_ws is None:
            await send({"type": "websocket.close", "code": 1000})
            return
        await app_ws(scope, receive, send)
        return
    if st != "http":
        return
    body = b""
    while True:
        m = await receive()
        t = m.get("type", "")
        if t == "http.request":
            body += m.get("body", b"")
            if not m.get("more_body", False):
                break
        elif t == "http.disconnect":
            return
    sch = "https" if scope.get("scheme") in ("https",) else "http"
    path = scope.get("path", "/") or "/"
    method = (scope.get("method", "GET") or "GET").upper()
    qs = scope.get("query_string", b"")
    if isinstance(qs, str):
        qs = qs.encode("utf-8")
    hdrs = _hdr_from_asgi(list(scope.get("headers") or []))
    rscope = _RsgiScope(
        sch,
        method,
        path,
        qs.decode("utf-8") if qs else "",
        hdrs,
    )
    loop = asyncio.get_running_loop()
    queue: asyncio.Queue[dict[str, Any] | None] = asyncio.Queue()

    async def _drain_outgoing() -> None:
        while True:
            msg = await queue.get()
            if msg is None:
                return
            await send(msg)

    drain_task = asyncio.create_task(_drain_outgoing())
    proto = _RsgiProtocol(body, queue, loop)
    await loop.run_in_executor(
        None,
        _run_handle_rsgi_blocking,
        app_rsgi,
        rscope,
        proto,
    )
    await queue.put(None)
    await drain_task


def build_asgi_caller(framework_app: Any) -> Callable[..., Any]:
    """
    Return ``async (scope, receive, send)`` using the framework app's
    ``_oxyroute.App.handle_rsgi`` (via ``App`` wrapper if present).
    """

    def _rsgi(s: Any, p: Any) -> Any:
        c = framework_app
        h = getattr(c, "handle_rsgi", None)
        if h is not None and callable(h):
            return h(s, p)
        inner = getattr(c, "_app", c)
        return inner.handle_rsgi(s, p)

    async def _ws(scope: dict[str, Any], receive: Any, send: Any) -> None:
        h = getattr(framework_app, "_handle_asgi_websocket", None)
        if h is None or not callable(h):
            await send({"type": "websocket.close", "code": 1000})
            return
        await h(scope, receive, send)

    async def asgi3(scope: dict[str, Any], receive: Any, send: Any) -> None:
        await asgi_to_rsgi(_rsgi, _ws, scope, receive, send)

    return asgi3
