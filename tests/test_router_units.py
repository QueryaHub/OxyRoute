"""Unit-level coverage for APIRouter registration options."""

from __future__ import annotations

from oxyroute import APIRouter


def _h() -> str:
    return "ok"


def test_router_join_path_variants() -> None:
    from oxyroute.router import join_path

    assert join_path(" /v1/ ", " users ") == "/v1/users"
    assert join_path("", "") == "/"
    assert join_path("v1", "") == "/v1/"


def test_router_collects_all_method_options() -> None:
    r = APIRouter()

    r.get("/g", require_jwt=True, jwt_secret="k", algorithms=["HS256"], jwt_cookie="tok")(_h)
    r.post(
        "/p",
        read_json_body=False,
        read_form_body=True,
        body_schema={"type": "object"},
    )(_h)
    r.put("/u", jwt_issuer="iss", jwt_audience="aud", jwt_leeway=5)(_h)
    r.patch("/pa", dependencies=[("x", lambda: 1)])(_h)
    r.delete("/d", require_jwt=True, jwt_secret="k")(_h)
    r.options("/o")(_h)

    methods = [m for m, _, _, _ in r._routes]
    assert methods == ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]

    by_path = {p: opts for _, p, _, opts in r._routes}
    assert by_path["/g"]["jwt_cookie"] == "tok"
    assert by_path["/p"]["read_form_body"] is True
    assert by_path["/u"]["jwt_issuer"] == "iss"
    assert by_path["/u"]["jwt_audience"] == "aud"
    assert by_path["/u"]["jwt_leeway"] == 5
