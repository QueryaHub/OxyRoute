import json

from oxyroute import App


def test_openapi_shows_route() -> None:
    app = App(title="T")
    @app.get("/items/:i")
    def list_items() -> str:
        return "ok"
    s = app.openapi_json()
    assert "paths" in s
    assert "/items/:i" in s
    assert "T" in s


def test_openapi_includes_patch_lowercase() -> None:
    app = App()

    @app.patch("/m")
    def m() -> str:
        return "ok"

    doc = json.loads(app.openapi_json())
    assert doc["paths"]["/m"]["patch"]["operationId"] == "m"
