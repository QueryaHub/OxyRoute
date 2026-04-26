"""
Sub-routers and ``include_router`` (issue #46).

Run from the project root (after an editable or wheel install)::

    granian --interface rsgi examples.routers_include_app:app

See `docs/routing.md` and https://github.com/QueryaHub/OxyRoute/issues/46
"""

from oxyroute import APIRouter, App

api = APIRouter()


@api.get("/ping")
def api_ping() -> str:
    return "pong"


@api.get("/version")
def api_version() -> str:
    return "1"


app = App(title="Include router example")
app.include_router(api, prefix="/v1")  # => GET /v1/ping, /v1/version


@app.get("/")
def root() -> str:
    return "ok"
