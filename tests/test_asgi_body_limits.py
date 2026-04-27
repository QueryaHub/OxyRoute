"""Extended ASGI body-limit and chunk-read coverage."""

from __future__ import annotations

import asyncio
import os
from collections import deque
from typing import Any

import oxyroute.asgi as asgi_mod
from oxyroute import App


def test_max_body_bytes_default() -> None:
    old = os.environ.pop("OXYROUTE_MAX_BODY_BYTES", None)
    try:
        assert asgi_mod._max_body_bytes() == 8 * 1024 * 1024
    finally:
        if old is not None:
            os.environ["OXYROUTE_MAX_BODY_BYTES"] = old


def test_max_body_bytes_invalid_and_negative_fallback_to_default() -> None:
    old = os.environ.get("OXYROUTE_MAX_BODY_BYTES")
    try:
        os.environ["OXYROUTE_MAX_BODY_BYTES"] = "abc"
        assert asgi_mod._max_body_bytes() == 8 * 1024 * 1024
        os.environ["OXYROUTE_MAX_BODY_BYTES"] = "-1"
        assert asgi_mod._max_body_bytes() == 8 * 1024 * 1024
    finally:
        if old is None:
            os.environ.pop("OXYROUTE_MAX_BODY_BYTES", None)
        else:
            os.environ["OXYROUTE_MAX_BODY_BYTES"] = old


def test_max_body_bytes_zero_means_unlimited() -> None:
    old = os.environ.get("OXYROUTE_MAX_BODY_BYTES")
    try:
        os.environ["OXYROUTE_MAX_BODY_BYTES"] = "0"
        assert asgi_mod._max_body_bytes() > 10**12
    finally:
        if old is None:
            os.environ.pop("OXYROUTE_MAX_BODY_BYTES", None)
        else:
            os.environ["OXYROUTE_MAX_BODY_BYTES"] = old


def test_asgi_body_limit_exact_boundary_passes() -> None:
    app = App()

    @app.post("/raw", read_json_body=False)
    def raw(body: bytes) -> str:
        return body.decode("utf-8")

    scope = {
        "type": "http",
        "http_version": "1.1",
        "scheme": "http",
        "method": "POST",
        "path": "/raw",
        "query_string": b"",
        "headers": [],
    }
    incoming = deque(
        [
            {"type": "http.request", "body": b"ab", "more_body": True},
            {"type": "http.request", "body": b"cd", "more_body": False},
        ]
    )
    sent: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        return incoming.popleft()

    async def send(msg: dict[str, Any]) -> None:
        sent.append(msg)

    old = os.environ.get("OXYROUTE_MAX_BODY_BYTES")
    try:
        os.environ["OXYROUTE_MAX_BODY_BYTES"] = "4"
        asyncio.run(app(scope, receive, send))
    finally:
        if old is None:
            os.environ.pop("OXYROUTE_MAX_BODY_BYTES", None)
        else:
            os.environ["OXYROUTE_MAX_BODY_BYTES"] = old

    assert sent[0]["status"] == 200
    assert sent[1]["body"] == b"abcd"


def test_asgi_body_limit_over_boundary_returns_413_and_json_ct() -> None:
    app = App()

    @app.post("/raw", read_json_body=False)
    def raw(body: bytes) -> str:  # pragma: no cover
        return body.decode("utf-8")

    scope = {
        "type": "http",
        "http_version": "1.1",
        "scheme": "http",
        "method": "POST",
        "path": "/raw",
        "query_string": b"",
        "headers": [],
    }
    incoming = deque(
        [
            {"type": "http.request", "body": b"ab", "more_body": True},
            {"type": "http.request", "body": b"cde", "more_body": False},
        ]
    )
    sent: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        return incoming.popleft()

    async def send(msg: dict[str, Any]) -> None:
        sent.append(msg)

    old = os.environ.get("OXYROUTE_MAX_BODY_BYTES")
    try:
        os.environ["OXYROUTE_MAX_BODY_BYTES"] = "4"
        asyncio.run(app(scope, receive, send))
    finally:
        if old is None:
            os.environ.pop("OXYROUTE_MAX_BODY_BYTES", None)
        else:
            os.environ["OXYROUTE_MAX_BODY_BYTES"] = old

    assert sent[0]["type"] == "http.response.start"
    assert sent[0]["status"] == 413
    assert (b"content-type", b"application/json; charset=utf-8") in sent[0]["headers"]
    assert sent[1]["body"] == b'{"error":"payload too large"}'


def test_asgi_http_disconnect_during_read_sends_nothing() -> None:
    app = App()

    @app.post("/raw", read_json_body=False)
    def raw(body: bytes) -> str:  # pragma: no cover
        return body.decode("utf-8")

    scope = {
        "type": "http",
        "http_version": "1.1",
        "scheme": "http",
        "method": "POST",
        "path": "/raw",
        "query_string": b"",
        "headers": [],
    }
    incoming = deque(
        [
            {"type": "http.request", "body": b"abc", "more_body": True},
            {"type": "http.disconnect"},
        ]
    )
    sent: list[dict[str, Any]] = []

    async def receive() -> dict[str, Any]:
        return incoming.popleft()

    async def send(msg: dict[str, Any]) -> None:
        sent.append(msg)

    asyncio.run(app(scope, receive, send))
    assert sent == []
