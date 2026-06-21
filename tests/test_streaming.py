from oxyroute import App, stream_bytes, stream_jsonl, stream_text
from oxyroute.testing import TestClient


def test_stream_text():
    app = App()

    @app.get("/text")
    async def text_handler(protocol):
        def _iter():
            yield "hello"
            yield " "
            yield "world"

        return await stream_text(protocol, _iter())

    with TestClient(app) as client:
        resp = client.get("/text")
        assert resp.status_code == 200
        assert resp.text == "hello world"
        assert resp.headers["content-type"] == "text/plain; charset=utf-8"


def test_stream_bytes():
    app = App()

    @app.get("/bytes")
    async def bytes_handler(protocol):
        def _iter():
            yield b"123"
            yield b"456"

        return await stream_bytes(protocol, _iter(), status=201, headers=[("x-custom", "foo")])

    with TestClient(app) as client:
        resp = client.get("/bytes")
        assert resp.status_code == 201
        assert resp.content == b"123456"
        assert resp.headers["content-type"] == "application/octet-stream"
        assert resp.headers["x-custom"] == "foo"


def test_stream_jsonl():
    app = App()

    @app.get("/jsonl")
    async def jsonl_handler(protocol):
        def _iter():
            yield {"id": 1, "name": "foo"}
            yield {"id": 2, "name": "bar"}

        return await stream_jsonl(protocol, _iter())

    with TestClient(app) as client:
        resp = client.get("/jsonl")
        assert resp.status_code == 200
        assert resp.text == '{"id": 1, "name": "foo"}\n{"id": 2, "name": "bar"}\n'
        assert resp.headers["content-type"] == "application/x-ndjson; charset=utf-8"


def test_stream_async_iter():
    app = App()

    @app.get("/async")
    async def async_handler(protocol):
        async def _iter():
            yield "async"
            yield " "
            yield "iter"

        return await stream_text(protocol, _iter())

    with TestClient(app) as client:
        resp = client.get("/async")
        assert resp.status_code == 200
        assert resp.text == "async iter"
