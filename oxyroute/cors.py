from __future__ import annotations

import re
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any, TypeAlias

from oxyroute.app import App
from oxyroute.response import Response

__all__ = ["CORSConfig", "apply_cors"]

_Middleware: TypeAlias = Callable[[Any, Any], Response | None]
_HDR_SPLIT = re.compile(r"[\s,]+")


@dataclass
class CORSConfig:
    """
    Declarative CORS settings. Register with :func:`apply_cors` (sets native ``set_cors`` and
    preflight middleware) or assign via :meth:`oxyroute.app.App.set_cors` if you only need
    response header merging without the built-in ``OPTIONS`` handler.

    ``response_header_pairs`` is called from Rust to merge CORS headers into normal responses
    (after a route or middleware that returns a body).
    """

    allow_origins: list[str] = field(default_factory=lambda: ["*"])
    allow_methods: list[str] = field(
        default_factory=lambda: ["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH", "OPTIONS"]
    )
    allow_headers: list[str] = field(default_factory=lambda: ["*"])
    expose_headers: list[str] = field(default_factory=list)
    allow_credentials: bool = False
    max_age: int | None = 600

    def __post_init__(self) -> None:
        self._allow_methods_upper = {m.upper() for m in self.allow_methods}

    def _select_allow_origin(self, origin: str) -> str | None:
        if not origin:
            return None
        if self.allow_credentials or "*" not in self.allow_origins:
            if origin in self.allow_origins:
                return origin
            return None
        if self.allow_origins == ["*"]:
            return "*"
        if origin in self.allow_origins:
            return origin
        return None

    def response_header_pairs(self, scope: Any) -> list[tuple[str, str]]:
        """
        Pairs merged into the outgoing response (called from the native layer). For requests
        without a permitted ``Origin``, returns an empty list.
        """
        origin = str(scope.headers.get("origin", "") or "")
        selected = self._select_allow_origin(origin)
        if selected is None:
            return []
        out: list[tuple[str, str]] = [("Access-Control-Allow-Origin", selected)]
        if self.allow_credentials:
            out.append(("Access-Control-Allow-Credentials", "true"))
        if self.expose_headers:
            out.append(
                (
                    "Access-Control-Expose-Headers",
                    ", ".join(self.expose_headers),
                )
            )
        if selected != "*":
            out.append(("Vary", "Origin"))
        return out

    def preflight_response(self, scope: Any) -> Response | None:
        """
        If this is a CORS preflight and the request is allowed, return ``204`` with preflight
        headers; if it is a preflight but the origin is not allowed, return ``400``; otherwise
        return ``None`` (not a preflight request).
        """
        method = str(getattr(scope, "method", "") or "").upper()
        if method != "OPTIONS":
            return None
        raw_acrm = str(scope.headers.get("access-control-request-method", "") or "")
        if not raw_acrm:
            return None
        request_method = raw_acrm.strip().upper()
        if request_method not in self._allow_methods_upper:
            return Response(status=400, body="Disallowed CORS method", headers={})

        origin = str(scope.headers.get("origin", "") or "")
        selected = self._select_allow_origin(origin)
        if selected is None:
            return Response(status=400, body="CORS origin not allowed", headers={})

        raw_ach = str(scope.headers.get("access-control-request-headers", "") or "")
        acah = self._preflight_access_control_allow_headers_value(raw_ach)
        if acah is None:
            return Response(status=400, body="Disallowed CORS request headers", headers={})

        h: dict[str, str] = {
            "Access-Control-Allow-Origin": selected,
        }
        if self.allow_credentials:
            h["Access-Control-Allow-Credentials"] = "true"
        h["Access-Control-Allow-Methods"] = ", ".join(self.allow_methods)
        h["Access-Control-Allow-Headers"] = acah
        if self.max_age is not None:
            h["Access-Control-Max-Age"] = str(self.max_age)
        if selected != "*":
            h["Vary"] = "Origin"
        return Response(status=204, body=None, headers=h)

    def _preflight_access_control_allow_headers_value(self, raw_ach: str) -> str | None:
        if self.allow_headers == ["*"] or (len(self.allow_headers) == 1 and self.allow_headers[0] == "*"):
            return "*"
        allow_lower = {x.lower() for x in self.allow_headers if x != "*"}
        parts = [p for p in _HDR_SPLIT.split(raw_ach.strip()) if p]
        if not parts:
            return ", ".join(self.allow_headers)
        for p in parts:
            if p.lower() not in allow_lower:
                return None
        return ", ".join(parts)


def apply_cors(
    app: App,
    config: CORSConfig,
    *,
    chain: _Middleware | None = None,
) -> None:
    """
    Register CORS: stores ``config`` for native response merging and installs preflight
    handling via :meth:`oxyroute.app.App.set_middleware`. If you already use middleware for
    other work, pass it as ``chain`` so it runs when the request is not a CORS preflight
    (your handler runs after the CORS layer returns ``None`` for continuation).

    **Order:** this replaces ``set_middleware`` with an internal function. To combine with
    another pre-route callback, use ``apply_cors(..., chain=your_middleware)``.
    """
    app.set_cors(config)

    def _cors_middleware(scope: Any, protocol: Any) -> Response | None:
        p = config.preflight_response(scope)
        if p is not None:
            return p
        if chain is not None:
            return chain(scope, protocol)
        return None

    app.set_middleware(_cors_middleware)
