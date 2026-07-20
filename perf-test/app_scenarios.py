"""Multi-scenario OxyRoute app for ``bench_scenarios.sh`` (issue #110)."""

from __future__ import annotations

from oxyroute import App, Depends
from oxyroute.cors import CORSConfig, apply_cors

SECRET = "bench-secret-key-do-not-use-in-prod"

app = App(title="perf scenarios", include_openapi=False)
apply_cors(app, CORSConfig(allow_origins=["*"], allow_credentials=False))


@app.get("/")
def plain_text() -> str:
    return "hello"


@app.post("/json")
def json_echo(json: dict) -> dict:
    return {"ok": True, "echo": json}


@app.get("/jwt", require_jwt=True, jwt_secret=SECRET, algorithms=["HS256"])
def jwt_ok(claims: dict) -> str:
    return f"sub={claims.get('sub', '')}"


def _dep_value() -> int:
    return 42


@app.get("/dep", dependencies=[("n", Depends(_dep_value))])
def with_dep(n: int) -> str:
    return f"n={n}"
