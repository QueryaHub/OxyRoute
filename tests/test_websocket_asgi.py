"""ASGI websocket spike for issue #52."""

from __future__ import annotations

import asyncio
from collections import deque
from typing import Any

from oxyroute import App


def test_asgi_websocket_echo_flow() -> None:
    app = App()

    @app.websocket("/ws")
    async def ws(sock: Any) -> None:
        await sock.accept()
        text = await sock.receive_text()
        await sock.send_text(f"echo:{text}")
        await sock.close(code=1000)

    incoming: deque[dict[str, Any]] = deque(
        [
            {"type": "websocket.connect"},
            {"type": "websocket.receive", "text": "ping"},
        ]
    )
    sent: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        if incoming:
            return incoming.popleft()
        return {"type": "websocket.disconnect", "code": 1000}

    async def send(message: dict[str, Any]) -> None:
        sent.append(message)

    scope = {
        "type": "websocket",
        "path": "/ws",
        "headers": [],
        "query_string": b"",
        "scheme": "ws",
    }

    async def _run() -> None:
        await app(scope, receive, send)

    asyncio.run(_run())

    assert sent[0]["type"] == "websocket.accept"
    assert sent[1]["type"] == "websocket.send"
    assert sent[1]["text"] == "echo:ping"
    assert sent[2]["type"] == "websocket.close"
