from __future__ import annotations

from collections.abc import AsyncIterable, Iterable
from dataclasses import dataclass
from typing import Any

__all__ = ["SSEEvent", "send_sse", "sse_done"]


@dataclass(slots=True)
class SSEEvent:
    data: str
    event: str | None = None
    id: str | None = None
    retry: int | None = None


class _SSEDone:
    __slots__ = ()
    __oxyroute_stream_done__ = True


def sse_done() -> Any:
    """Return a marker value telling OxyRoute the response was already sent."""
    return _SSEDone()


def _format_sse_event(ev: SSEEvent) -> str:
    lines: list[str] = []
    if ev.event:
        lines.append(f"event: {ev.event}")
    if ev.id:
        lines.append(f"id: {ev.id}")
    if ev.retry is not None:
        lines.append(f"retry: {ev.retry}")
    for line in ev.data.splitlines() or [""]:
        lines.append(f"data: {line}")
    return "\n".join(lines) + "\n\n"


def _to_sse_chunk(item: Any) -> str:
    if isinstance(item, SSEEvent):
        return _format_sse_event(item)
    if isinstance(item, str):
        return _format_sse_event(SSEEvent(data=item))
    raise TypeError("SSE items must be str or SSEEvent")


async def send_sse(
    protocol: Any,
    events: Iterable[str | SSEEvent] | AsyncIterable[str | SSEEvent],
    *,
    status: int = 200,
    headers: list[tuple[str, str]] | None = None,
) -> Any:
    """
    Send SSE response via RSGI protocol.

    If `response_stream` is available (Granian RSGI), writes chunks incrementally.
    Otherwise (ASGI bridge/test transports), falls back to a single response body.
    """
    base_headers: list[tuple[str, str]] = [
        ("content-type", "text/event-stream; charset=utf-8"),
        ("cache-control", "no-cache"),
        ("connection", "keep-alive"),
    ]
    if headers:
        base_headers.extend(headers)

    stream_factory = getattr(protocol, "response_stream", None)
    if callable(stream_factory):
        stream = stream_factory(status, base_headers)
        if hasattr(events, "__aiter__"):
            async for item in events:  # type: ignore[union-attr]
                await stream.send_str(_to_sse_chunk(item))
        else:
            for item in events:  # type: ignore[not-an-iterable]
                await stream.send_str(_to_sse_chunk(item))
        return sse_done()

    chunks: list[str] = []
    if hasattr(events, "__aiter__"):
        async for item in events:  # type: ignore[union-attr]
            chunks.append(_to_sse_chunk(item))
    else:
        for item in events:  # type: ignore[not-an-iterable]
            chunks.append(_to_sse_chunk(item))
    protocol.response_str(status, base_headers, "".join(chunks))
    return sse_done()
