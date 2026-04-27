from __future__ import annotations

import hmac
import re
import secrets
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any, TypeAlias

from oxyroute.app import App
from oxyroute.response import Response

__all__ = ["CSRFConfig", "apply_csrf", "csrf_layer"]

_Middleware: TypeAlias = Callable[[Any, Any], Response | None]
_DEFAULT_UNSAFE = frozenset({"POST", "PUT", "PATCH", "DELETE"})

_COOKIE_PAIR = re.compile(r"([^=]+)=(.*)")


@dataclass
class CSRFConfig:
    """
    **Double-submit** CSRF: the client must send the same secret in a **cookie** and in
    a **header** (or header only vs cookie — both present and equal). Stateless; no
    server-side session table.

    This matters when the browser **automatically** sends a session cookie; pure APIs that
    use only ``Authorization: Bearer ...`` and no auth cookies can usually **omit** CSRF.

    :meth:`guard` runs in pre-route middleware (before the body) — safe for large POSTs:
    the check only reads ``Cookie`` and the header.

    For **CORS + CSRF**, register CORS first and pass :func:`csrf_layer` as ``chain`` to
    :func:`oxyroute.cors.apply_cors` (see the CSRF doc page in ``docs/``).

    *SameSite* on the cookie (``Lax``/``Strict``) helps against cross-site form posts; the
    double-submit is an extra check when you must support cookies across flows that
    ``SameSite`` does not cover.
    """

    cookie_name: str = "oxyroute_csrf"
    header_name: str = "X-CSRF-Token"
    unsafe_methods: frozenset[str] = field(default_factory=lambda: _DEFAULT_UNSAFE)
    # Set-Cookie flags when you build the cookie (see :meth:`set_cookie_value`)
    secure: bool = False

    def issue_token(self) -> str:
        return secrets.token_urlsafe(32)

    def set_cookie_value(self, token: str) -> str:
        """
        One ``Set-Cookie`` line (append via :attr:`oxyroute.Response.cookies`).
        **HttpOnly** is omitted so browser JS *can* mirror the value into a header; if you
        only use traditional forms, you can set ``HttpOnly`` and pass the token in a hidden
        field instead of this header.
        """
        p = f"{self.cookie_name}={token}; Path=/; SameSite=Lax"
        if self.secure:
            p += "; Secure"
        return p

    def guard(self, scope: Any) -> Response | None:
        """
        If the request must be protected and validation fails, return **403**; else
        return ``None`` to continue.
        """
        m = str(getattr(scope, "method", "") or "").upper()
        if m not in self.unsafe_methods:
            return None
        a = _read_cookie(scope, self.cookie_name)
        b = _read_header(scope, self.header_name)
        if not a or not b:
            return _csrf_403("missing")
        if len(a) != len(b) or not hmac.compare_digest(a.encode("utf-8"), b.encode("utf-8")):
            return _csrf_403("mismatch")
        return None


def _csrf_403(detail: str) -> Response:
    return Response(
        status=403,
        body={"error": "csrf", "detail": detail},
        headers={"content-type": "application/json; charset=utf-8"},
    )


def _read_header(scope: Any, name: str) -> str:
    h = getattr(scope, "headers", None)
    if h is None:
        return ""
    v = h.get(name, "") or h.get(name.lower(), "")
    return str(v or "").strip()


def _read_cookie(scope: Any, name: str) -> str:
    h = getattr(scope, "headers", None)
    if h is None:
        return ""
    raw = str(h.get("cookie", "") or h.get("Cookie", "") or "")
    for part in raw.split(";"):
        part = part.strip()
        mo = _COOKIE_PAIR.match(part)
        if not mo:
            continue
        if mo.group(1).strip() == name:
            return mo.group(2).strip().strip('"')
    return ""


def csrf_layer(config: CSRFConfig) -> _Middleware:
    """
    A ``(scope, protocol)`` callback suitable for ``chain=`` in :func:`apply_cors` or
    for composing other pre-route hooks: runs CSRF :meth:`CSRFConfig.guard` and returns
    a response or ``None``.
    """

    def _mw(scope: Any, _protocol: Any) -> Response | None:
        return config.guard(scope)

    return _mw


def apply_csrf(
    app: App,
    config: CSRFConfig,
    *,
    chain: _Middleware | None = None,
) -> None:
    """
    Installs **one** :meth:`oxyroute.app.App.set_middleware` that runs :meth:`CSRFConfig.guard`
    first, then ``chain`` (if any), then continues routing. Replaces any previous
    pre-route callback — combine manually or use :func:`csrf_layer` inside
    :func:`oxyroute.cors.apply_cors`.
    """

    def _mw(scope: Any, protocol: Any) -> Response | None:
        d = config.guard(scope)
        if d is not None:
            return d
        if chain is not None:
            return chain(scope, protocol)
        return None

    app.set_middleware(_mw)
