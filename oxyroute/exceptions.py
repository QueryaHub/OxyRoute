"""HTTP errors mapped by the native dispatcher to non-500 responses (issue #48)."""

from __future__ import annotations

import json
from http import HTTPStatus
from typing import Any


def _phrase(code: int) -> str:
    try:
        return HTTPStatus(code).phrase
    except ValueError:
        return "Error"


class HTTPException(Exception):
    """
    Raise from a route handler or dependency to return a specific status and JSON body.

    The response body is ``application/json; charset=utf-8`` unless you set ``Content-Type`` in
    ``headers``. String ``detail`` becomes ``{"detail": "..."}``; ``dict`` / ``list`` become the
    root JSON value.
    """

    def __init__(
        self,
        status_code: int,
        detail: Any = None,
        *,
        headers: dict[str, str] | None = None,
    ) -> None:
        self.status_code = int(status_code)
        self.detail = detail
        self.headers: dict[str, str] = dict(headers) if headers else {}


def _http_exception_payload(
    exc: BaseException,
) -> tuple[int, bytes, list[tuple[str, str]]] | None:
    """Return ``(status, body_bytes, headers)`` for Rust, or ``None`` if not an HTTPException."""
    if not isinstance(exc, HTTPException):
        return None
    st = exc.status_code
    if not (100 <= st <= 599):
        return None
    if exc.detail is None:
        body = json.dumps({"detail": _phrase(st)}).encode("utf-8")
    elif isinstance(exc.detail, (dict, list)):
        body = json.dumps(exc.detail).encode("utf-8")
    else:
        body = json.dumps({"detail": str(exc.detail)}).encode("utf-8")
    return (st, body, list(exc.headers.items()))
