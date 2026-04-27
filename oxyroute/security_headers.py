from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

__all__ = ["SecurityHeadersConfig"]


@dataclass
class SecurityHeadersConfig:
    """
    Preset for common **browser security** response headers. Register with
    :meth:`oxyroute.app.App.set_security_headers`.

    The native layer calls ``response_header_pairs(scope)`` and merges each pair only if a header
    with the same name is **not** already present (so explicit :class:`oxyroute.response.Response`
    headers win). CORS, when configured, is merged after this preset.

    **HSTS** is added only when ``scope.scheme == "https"``; use **only in production** behind TLS
    (or reverse proxy that sets ``X-Forwarded-Proto`` — OxyRoute still uses the RSGI ``scheme``;
    in dev over plain HTTP, leave ``hsts`` unset to avoid long-lived HSTS in browsers).
    """

    hsts: str | None = None
    x_content_type_options: str | None = "nosniff"
    x_frame_options: str | None = "DENY"
    referrer_policy: str | None = "strict-origin-when-cross-origin"
    content_security_policy: str | None = None
    permissions_policy: str | None = None
    extra: dict[str, str] = field(default_factory=dict)

    def response_header_pairs(self, scope: Any) -> list[tuple[str, str]]:
        out: list[tuple[str, str]] = []
        scheme = str(getattr(scope, "scheme", "") or "").lower()
        if self.hsts and scheme == "https":
            out.append(("Strict-Transport-Security", self.hsts))
        if self.x_content_type_options:
            out.append(("X-Content-Type-Options", self.x_content_type_options))
        if self.x_frame_options:
            out.append(("X-Frame-Options", self.x_frame_options))
        if self.referrer_policy:
            out.append(("Referrer-Policy", self.referrer_policy))
        if self.content_security_policy:
            out.append(("Content-Security-Policy", self.content_security_policy))
        if self.permissions_policy:
            out.append(("Permissions-Policy", self.permissions_policy))
        for k, v in self.extra.items():
            out.append((k, v))
        return out
