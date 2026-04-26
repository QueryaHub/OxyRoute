"""
Optional ASGI 3.0 entry for ``uvicorn`` / ``granian --interface asgi``.

Builds a minimal RSGI-like ``scope`` / ``protocol`` and delegates to the same Rust
``handle_rsgi`` coroutine as ``__rsgi__``. Only ``type == "http"`` is supported.
"""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from typing import Any


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
    __slots__ = ("_body", "_loop", "_send", "_status", "status")

    def __init__(self, body: bytes, send: Any, main_loop: asyncio.AbstractEventLoop) -> None:
        self._body = body
        self._send = send
        self._loop = main_loop
        self._status: int | None = None
        self.status = 200

    def _run_send(self, coro: Any) -> None:
        f = asyncio.run_coroutine_threadsafe(coro, self._loop)
        f.result()

    def __call__(self) -> Any:
        async def _body() -> bytes:
            return self._body

        return _body()

    def response_str(self, status: int, headers: list, body: str) -> None:
        self._status = int(status)
        self.status = int(status)
        b = body.encode("utf-8")
        h = _norm_headers_asgi(headers)

        async def _go() -> None:
            await self._send(
                {
                    "type": "http.response.start",
                    "status": int(status),
                    "headers": h,
                }
            )
            await self._send(
                {
                    "type": "http.response.body",
                    "body": b,
                }
            )

        self._run_send(_go())

    def response_bytes(self, status: int, headers: list, body: bytes) -> None:
        self._status = int(status)
        self.status = int(status)
        h = _norm_headers_asgi(headers)

        async def _go() -> None:
            await self._send(
                {
                    "type": "http.response.start",
                    "status": int(status),
                    "headers": h,
                }
            )
            await self._send(
                {
                    "type": "http.response.body",
                    "body": body,
                }
            )

        self._run_send(_go())

    def response_empty(self, status: int, headers: list) -> None:
        self._status = int(status)
        self.status = int(status)
        h = _norm_headers_asgi(headers)

        async def _go() -> None:
            await self._send(
                {
                    "type": "http.response.start",
                    "status": int(status),
                    "headers": h,
                }
            )
            await self._send(
                {
                    "type": "http.response.body",
                }
            )

        self._run_send(_go())


async def asgi_to_rsgi(
    app_rsgi: Callable[[Any, Any], Any],
    scope: dict[str, Any],
    receive: Any,
    send: Any,
) -> None:
    if scope.get("type") != "http":
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
    proto = _RsgiProtocol(body, send, loop)
    await app_rsgi(rscope, proto)


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

    async def asgi3(scope: dict[str, Any], receive: Any, send: Any) -> None:
        await asgi_to_rsgi(_rsgi, scope, receive, send)

    return asgi3
