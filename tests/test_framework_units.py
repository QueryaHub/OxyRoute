"""Unit-level coverage for config/helper modules."""

from __future__ import annotations

from dataclasses import dataclass

import pytest
from oxyroute.cors import CORSConfig
from oxyroute.csrf import CSRFConfig, _read_cookie, _read_header, csrf_layer
from oxyroute.exceptions import HTTPException, _http_exception_payload
from oxyroute.security_headers import SecurityHeadersConfig
from oxyroute.sse import SSEEvent, _to_sse_chunk, sse_done


@dataclass
class _Scope:
    method: str = "GET"
    scheme: str = "http"
    headers: dict[str, str] | None = None

    def __post_init__(self) -> None:
        if self.headers is None:
            self.headers = {}


def test_cors_select_allow_origin_and_response_pairs() -> None:
    cfg = CORSConfig(
        allow_origins=["https://a.example", "https://b.example"],
        expose_headers=["X-Id"],
        allow_credentials=True,
    )
    s = _Scope(headers={"origin": "https://a.example"})
    pairs = cfg.response_header_pairs(s)
    assert ("Access-Control-Allow-Origin", "https://a.example") in pairs
    assert ("Access-Control-Allow-Credentials", "true") in pairs
    assert ("Access-Control-Expose-Headers", "X-Id") in pairs
    assert ("Vary", "Origin") in pairs


def test_cors_preflight_disallowed_method_and_headers() -> None:
    cfg = CORSConfig(
        allow_origins=["https://a.example"],
        allow_methods=["GET"],
        allow_headers=["X-Ok"],
    )
    bad_method = _Scope(
        method="OPTIONS",
        headers={
            "origin": "https://a.example",
            "access-control-request-method": "POST",
        },
    )
    r1 = cfg.preflight_response(bad_method)
    assert r1 is not None and r1.status == 400 and r1.body == "Disallowed CORS method"

    bad_headers = _Scope(
        method="OPTIONS",
        headers={
            "origin": "https://a.example",
            "access-control-request-method": "GET",
            "access-control-request-headers": "X-Bad",
        },
    )
    r2 = cfg.preflight_response(bad_headers)
    assert r2 is not None and r2.status == 400 and r2.body == "Disallowed CORS request headers"


def test_cors_preflight_wildcard_headers_value() -> None:
    cfg = CORSConfig(allow_headers=["*"])
    assert cfg._preflight_access_control_allow_headers_value("X-A, X-B") == "*"


def test_csrf_issue_cookie_and_guard_safe_method() -> None:
    cfg = CSRFConfig(cookie_name="ck", header_name="X-CSRF", secure=True)
    token = cfg.issue_token()
    cookie = cfg.set_cookie_value(token)
    assert cookie.startswith("ck=") and "; Secure" in cookie and "SameSite=Lax" in cookie
    assert cfg.guard(_Scope(method="GET", headers={})) is None


def test_csrf_guard_missing_and_mismatch_and_layer() -> None:
    cfg = CSRFConfig(cookie_name="ck", header_name="X-CSRF")
    missing = cfg.guard(_Scope(method="POST", headers={}))
    assert missing is not None and missing.status == 403 and missing.body["detail"] == "missing"

    mismatch = cfg.guard(_Scope(method="POST", headers={"cookie": "ck=a", "X-CSRF": "b"}))
    assert mismatch is not None and mismatch.status == 403 and mismatch.body["detail"] == "mismatch"

    layer = csrf_layer(cfg)
    assert layer(_Scope(method="GET", headers={}), object()) is None


def test_csrf_header_cookie_read_helpers() -> None:
    s = _Scope(headers={"Cookie": 'a=1; ck="v.t"; z=9', "x-csrf-token": " tok "})
    assert _read_cookie(s, "ck") == "v.t"
    assert _read_header(s, "X-CSRF-Token") == "tok"


def test_http_exception_payload_variants() -> None:
    assert _http_exception_payload(ValueError("x")) is None
    assert _http_exception_payload(HTTPException(99, "x")) is None

    st, body, headers = _http_exception_payload(HTTPException(404, None)) or (0, b"", [])
    assert st == 404 and b"Not Found" in body and headers == []

    st2, body2, _ = _http_exception_payload(HTTPException(422, {"x": 1})) or (0, b"", [])
    assert st2 == 422 and body2 == b'{"x": 1}'

    st3, body3, headers3 = _http_exception_payload(
        HTTPException(400, "bad", headers={"X-A": "1"})
    ) or (0, b"", [])
    assert st3 == 400 and b'"detail": "bad"' in body3 and ("X-A", "1") in headers3


def test_security_headers_pairs_and_extra() -> None:
    cfg = SecurityHeadersConfig(
        hsts="max-age=60",
        x_content_type_options="nosniff",
        x_frame_options="DENY",
        referrer_policy="strict-origin-when-cross-origin",
        content_security_policy="default-src 'none'",
        permissions_policy="geolocation=()",
        extra={"X-Custom": "1"},
    )
    https_pairs = cfg.response_header_pairs(_Scope(scheme="https"))
    assert ("Strict-Transport-Security", "max-age=60") in https_pairs
    assert ("Content-Security-Policy", "default-src 'none'") in https_pairs
    assert ("Permissions-Policy", "geolocation=()") in https_pairs
    assert ("X-Custom", "1") in https_pairs

    http_pairs = cfg.response_header_pairs(_Scope(scheme="http"))
    assert not any(k == "Strict-Transport-Security" for k, _ in http_pairs)


def test_sse_helpers_and_done_marker() -> None:
    chunk = _to_sse_chunk(SSEEvent(data="a\nb", event="tick", id="7", retry=1000))
    assert "event: tick" in chunk and "id: 7" in chunk and "retry: 1000" in chunk
    assert "data: a" in chunk and "data: b" in chunk and chunk.endswith("\n\n")

    assert "data: hello" in _to_sse_chunk("hello")
    with pytest.raises(TypeError):
        _to_sse_chunk(123)  # type: ignore[arg-type]

    done = sse_done()
    assert getattr(done, "__oxyroute_stream_done__", False) is True
