from __future__ import annotations

import json
from collections.abc import AsyncIterable, Iterable
from typing import Any

__all__ = ["stream_bytes", "stream_done", "stream_jsonl", "stream_text"]


class _StreamDone:
    __slots__ = ()
    __oxyroute_stream_done__ = True


def stream_done() -> Any:
    """Return a marker value telling OxyRoute the response was already sent."""
    return _StreamDone()


async def stream_bytes(
    protocol: Any,
    iterable: Iterable[bytes] | AsyncIterable[bytes],
    *,
    status: int = 200,
    headers: list[tuple[str, str]] | None = None,
    content_type: str = "application/octet-stream",
) -> Any:
    """
    Stream raw bytes via RSGI protocol.

    If `response_stream` is available (Granian RSGI), writes chunks incrementally.
    Otherwise (ASGI bridge/test transports), falls back to a single response body.
    """
    base_headers: list[tuple[str, str]] = [("content-type", content_type)]
    if headers:
        base_headers.extend(headers)

    stream_factory = getattr(protocol, "response_stream", None)
    if callable(stream_factory):
        stream = stream_factory(status, base_headers)
        if hasattr(iterable, "__aiter__"):
            async for chunk in iterable:  # type: ignore[union-attr]
                await stream.send_bytes(chunk)
        else:
            for chunk in iterable:  # type: ignore[not-an-iterable]
                await stream.send_bytes(chunk)
        return stream_done()

    # Fallback for test/ASGI transports
    chunks: list[bytes] = []
    if hasattr(iterable, "__aiter__"):
        async for chunk in iterable:  # type: ignore[union-attr]
            chunks.append(chunk)
    else:
        for chunk in iterable:  # type: ignore[not-an-iterable]
            chunks.append(chunk)
    protocol.response_bytes(status, base_headers, b"".join(chunks))
    return stream_done()


async def stream_text(
    protocol: Any,
    iterable: Iterable[str] | AsyncIterable[str],
    *,
    status: int = 200,
    headers: list[tuple[str, str]] | None = None,
    content_type: str = "text/plain; charset=utf-8",
) -> Any:
    """
    Stream text via RSGI protocol.

    If `response_stream` is available (Granian RSGI), writes chunks incrementally.
    Otherwise (ASGI bridge/test transports), falls back to a single response body.
    """
    base_headers: list[tuple[str, str]] = [("content-type", content_type)]
    if headers:
        base_headers.extend(headers)

    stream_factory = getattr(protocol, "response_stream", None)
    if callable(stream_factory):
        stream = stream_factory(status, base_headers)
        if hasattr(iterable, "__aiter__"):
            async for chunk in iterable:  # type: ignore[union-attr]
                await stream.send_str(chunk)
        else:
            for chunk in iterable:  # type: ignore[not-an-iterable]
                await stream.send_str(chunk)
        return stream_done()

    chunks: list[str] = []
    if hasattr(iterable, "__aiter__"):
        async for chunk in iterable:  # type: ignore[union-attr]
            chunks.append(chunk)
    else:
        for chunk in iterable:  # type: ignore[not-an-iterable]
            chunks.append(chunk)
    protocol.response_str(status, base_headers, "".join(chunks))
    return stream_done()


async def stream_jsonl(
    protocol: Any,
    iterable: Iterable[Any] | AsyncIterable[Any],
    *,
    status: int = 200,
    headers: list[tuple[str, str]] | None = None,
) -> Any:
    """
    Stream NDJSON (JSON-Lines) via RSGI protocol.
    """

    async def _jsonl_iter() -> AsyncIterable[str]:
        if hasattr(iterable, "__aiter__"):
            async for item in iterable:  # type: ignore[union-attr]
                yield json.dumps(item) + "\n"
        else:
            for item in iterable:  # type: ignore[not-an-iterable]
                yield json.dumps(item) + "\n"

    return await stream_text(
        protocol,
        _jsonl_iter(),
        status=status,
        headers=headers,
        content_type="application/x-ndjson; charset=utf-8",
    )
