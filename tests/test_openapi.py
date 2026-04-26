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
