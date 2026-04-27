"""Extra CSRF helper branch coverage."""

from __future__ import annotations

from dataclasses import dataclass

from oxyroute.csrf import CSRFConfig, _read_cookie, _read_header, apply_csrf


@dataclass
class _Scope:
    method: str = "GET"
    headers: dict[str, str] | None = None


def test_read_helpers_with_missing_headers_object() -> None:
    s = _Scope(method="POST", headers=None)
    assert _read_cookie(s, "x") == ""
    assert _read_header(s, "x") == ""


def test_apply_csrf_chain_runs_after_guard_pass() -> None:
    cfg = CSRFConfig(cookie_name="ck", header_name="X-CSRF")
    called: list[str] = []

    def chain(scope: object, _protocol: object) -> None:
        called.append(getattr(scope, "method", ""))
        return None

    class _AppStub:
        def __init__(self) -> None:
            self.middleware = None

        def set_middleware(self, mw):  # type: ignore[no-untyped-def]
            self.middleware = mw

    app = _AppStub()
    apply_csrf(app, cfg, chain=chain)  # type: ignore[arg-type]
    assert app.middleware is not None

    # safe method: guard passes, chain executes
    out = app.middleware(_Scope(method="GET", headers={}), object())
    assert out is None
    assert called == ["GET"]

    # unsafe method with mismatch: guard blocks before chain
    called.clear()
    out2 = app.middleware(
        _Scope(method="POST", headers={"cookie": "ck=a", "X-CSRF": "b"}),
        object(),
    )
    assert out2 is not None
    assert called == []
