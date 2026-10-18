"""Internal asyncio coroutine helpers for native RSGI WebSocket transport.

These coroutines run directly on Granian's Python asyncio event loop without
crossing the Tokio future bridge, avoiding task scheduling and context switching
overhead on every WebSocket frame (issue #257).
"""

from __future__ import annotations

import json
from typing import Any


async def ws_accept(proto: Any, ws: Any) -> None:
    transport = await proto.accept()
    ws._set_transport(transport)


async def ws_receive(transport: Any, ws: Any) -> Any:
    msg = await transport.receive()
    if msg.kind == 0:
        ws._mark_closed()
        raise RuntimeError("WebSocket closed by peer")
    return msg.data


async def ws_receive_text(transport: Any, ws: Any) -> str:
    msg = await transport.receive()
    if msg.kind == 0:
        ws._mark_closed()
        raise RuntimeError("WebSocket closed by peer")
    if msg.kind != 2:
        raise ValueError("expected text frame")
    if not isinstance(msg.data, str):
        raise ValueError("expected text frame")
    return msg.data


async def ws_receive_bytes(transport: Any, ws: Any) -> bytes:
    msg = await transport.receive()
    if msg.kind == 0:
        ws._mark_closed()
        raise RuntimeError("WebSocket closed by peer")
    if msg.kind != 1:
        raise ValueError("expected binary frame")
    data = msg.data
    if not isinstance(data, (bytes, bytearray, memoryview)):
        raise ValueError("expected binary frame")
    return bytes(data) if not isinstance(data, bytes) else data


async def ws_send_text(transport: Any, lock: Any, data: str) -> None:
    async with lock:
        await transport.send_str(data)


async def ws_send_bytes(transport: Any, lock: Any, data: bytes) -> None:
    async with lock:
        await transport.send_bytes(data)


async def ws_send_json(transport: Any, lock: Any, data: Any) -> None:
    dumped = json.dumps(data)
    async with lock:
        await transport.send_str(dumped)


async def ws_close_noop() -> None:
    return None
