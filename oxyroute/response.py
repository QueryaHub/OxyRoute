from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

__all__ = ["Response"]


@dataclass
class Response:
    """
    Structured HTTP response: status, body, and optional extra headers (and ``Set-Cookie`` lines).

    The native dispatcher recognizes this type and calls RSGI with a full header list.
    If ``headers`` does not set ``content-type``, one is chosen from the body type
    (``text/plain`` for ``str``, ``application/octet-stream`` for ``bytes``,
    ``application/json`` for other values after ``json.dumps``).
    """

    body: str | bytes | Any | None = None
    status: int = 200
    headers: Mapping[str, str] | None = None
    cookies: Sequence[str] | None = None
