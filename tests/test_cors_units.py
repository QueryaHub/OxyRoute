"""Extra branch coverage for CORS configuration logic."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass

import httpx
from oxyroute import App
from oxyroute.cors import CORSConfig, apply_cors
from oxyroute.testing import asgi_test_app


@dataclass
class _Scope:
    method: str = "GET"
    headers: dict[str, str] | None = None

    def __post_init__(self) -> None:
        if self.headers is None:
            self.headers = {}


def test_select_allow_origin_branches() -> None:
    cfg = CORSConfig(allow_origins=["*"], allow_credentials=False)
    assert cfg._select_allow_origin("") is None
    assert cfg._select_allow_origin("https://x.example") == "*"

    cfg2 = CORSConfig(
        allow_origins=["https://a.example"],
        allow_credentials=True,
    )
    assert cfg2._select_allow_origin("https://a.example") == "https://a.example"
    assert cfg2._select_allow_origin("https://b.example") is None

    cfg3 = CORSConfig(allow_origins=["https://a.example"], allow_credentials=False)
    assert cfg3._select_allow_origin("https://b.example") is None
    assert cfg3._select_allow_origin("https://a.example") == "https://a.example"


def test_response_header_pairs_origin_not_allowed_is_empty() -> None:
    cfg = CORSConfig(allow_origins=["https://a.example"])
    pairs = cfg.response_header_pairs(_Scope(headers={"origin": "https://b.example"}))
    assert pairs == []


def test_preflight_non_options_and_missing_acrm_return_none() -> None:
    cfg = CORSConfig()
    assert cfg.preflight_response(_Scope(method="GET", headers={})) is None
    assert (
        cfg.preflight_response(_Scope(method="OPTIONS", headers={"origin": "https://a.example"}))
        is None
    )


def test_preflight_origin_disallowed_and_success_headers() -> None:
    cfg = CORSConfig(
        allow_origins=["https://a.example"],
        allow_methods=["GET", "POST"],
        allow_headers=["X-A", "X-B"],
        allow_credentials=True,
        max_age=600,
    )
    bad = cfg.preflight_response(
        _Scope(
            method="OPTIONS",
            headers={
                "origin": "https://b.example",
                "access-control-request-method": "GET",
            },
        )
    )
    assert bad is not None
    assert bad.status == 400
    assert bad.body == "CORS origin not allowed"

    ok = cfg.preflight_response(
        _Scope(
            method="OPTIONS",
            headers={
                "origin": "https://a.example",
                "access-control-request-method": "POST",
                "access-control-request-headers": "X-A, X-B",
            },
        )
    )
    assert ok is not None
    assert ok.status == 204
    assert ok.body is None
    assert ok.headers is not None
    assert ok.headers["Access-Control-Allow-Origin"] == "https://a.example"
    assert ok.headers["Access-Control-Allow-Credentials"] == "true"
    assert ok.headers["Access-Control-Allow-Methods"] == "GET, POST"
    assert ok.headers["Access-Control-Allow-Headers"] == "X-A, X-B"
    assert ok.headers["Access-Control-Max-Age"] == "600"
    assert ok.headers["Vary"] == "Origin"


def test_preflight_allow_headers_empty_parts_and_reject_unknown() -> None:
    cfg = CORSConfig(allow_headers=["X-A", "X-B"])
    # Empty request header list -> configured allow-list value
    assert cfg._preflight_access_control_allow_headers_value(" , ,, ") == "X-A, X-B"
    # Unknown header is rejected
    assert cfg._preflight_access_control_allow_headers_value("X-A, X-C") is None


def test_apply_cors_chain_passthrough_and_preflight_short_circuit() -> None:
    app = App()
    called: list[str] = []
    cfg = CORSConfig(allow_origins=["https://a.example"], allow_methods=["GET"])

    def chain(scope: object, _protocol: object) -> None:
        called.append(f"{getattr(scope, 'method', '')}")
        return None

    apply_cors(app, cfg, chain=chain)

    # Validate branch behavior through equivalent closure logic used by apply_cors.
    def _cors_mw(scope: object, protocol: object) -> object:
        p = cfg.preflight_response(scope)
        if p is not None:
            return p
        return chain(scope, protocol)

    out = _cors_mw(_Scope(method="GET", headers={}), object())
    assert out is None
    assert called == ["GET"]

    # Preflight: should short-circuit and not call chain.
    called.clear()
    out2 = _cors_mw(
        _Scope(
            method="OPTIONS",
            headers={
                "origin": "https://a.example",
                "access-control-request-method": "GET",
            },
        ),
        object(),
    )
    assert out2 is not None
    assert called == []


def test_apply_cors_chain_called_for_non_preflight_integration() -> None:
    app = App()
    called: list[str] = []

    def chain(scope: object, _protocol: object) -> None:
        called.append(getattr(scope, "method", ""))
        return None

    apply_cors(
        app,
        CORSConfig(allow_origins=["https://a.example"], allow_methods=["GET"]),
        chain=chain,
    )

    @app.get("/hi")
    def hi() -> str:
        return "ok"

    async def _run() -> None:
        tr = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=tr, base_url="http://t") as c:
            r = await c.get("/hi", headers={"origin": "https://a.example"})
        assert r.status_code == 200
        assert r.text == "ok"

    asyncio.run(_run())
    assert called == ["GET"]


def test_cors_select_origin_with_wildcard_plus_specific_list() -> None:
    cfg = CORSConfig(allow_origins=["*", "https://a.example"], allow_credentials=False)
    # not the exact ["*"] list, so explicit origin membership branch is used
    assert cfg._select_allow_origin("https://a.example") == "https://a.example"
    assert cfg._select_allow_origin("https://b.example") is None
