"""Unit-level coverage for SSE helper fallback paths."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass

from oxyroute.sse import SSEEvent, send_sse


@dataclass
class _Stream:
    out: list[str]

    async def send_str(self, value: str) -> None:
        self.out.append(value)


class _ProtoWithStream:
    def __init__(self) -> None:
        self.status_headers: tuple[int, list[tuple[str, str]]] | None = None
        self.out: list[str] = []

    def response_stream(self, status: int, headers: list[tuple[str, str]]) -> _Stream:
        self.status_headers = (status, headers)
        return _Stream(self.out)


class _ProtoFallback:
    def __init__(self) -> None:
        self.calls: list[tuple[int, list[tuple[str, str]], str]] = []

    def response_str(self, status: int, headers: list[tuple[str, str]], body: str) -> None:
        self.calls.append((status, headers, body))


def test_send_sse_stream_path_with_async_iterable() -> None:
    p = _ProtoWithStream()

    async def events() -> object:
        for item in ["a", SSEEvent(data="b", event="tick")]:
            yield item

    async def _run() -> None:
        done = await send_sse(p, events(), status=201, headers=[("x-test", "1")])
        assert getattr(done, "__oxyroute_stream_done__", False)

    asyncio.run(_run())
    assert p.status_headers is not None
    st, headers = p.status_headers
    assert st == 201
    assert ("x-test", "1") in headers
    assert any("data: a" in chunk for chunk in p.out)
    assert any("event: tick" in chunk for chunk in p.out)


def test_send_sse_fallback_path_with_list_events() -> None:
    p = _ProtoFallback()

    async def _run() -> None:
        done = await send_sse(p, ["hello", SSEEvent(data="w", id="1")])
        assert getattr(done, "__oxyroute_stream_done__", False)

    asyncio.run(_run())
    assert len(p.calls) == 1
    status, headers, body = p.calls[0]
    assert status == 200
    assert ("content-type", "text/event-stream; charset=utf-8") in headers
    assert "data: hello" in body
    assert "id: 1" in body
