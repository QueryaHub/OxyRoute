"""`application/x-www-form-urlencoded` and `multipart/form-data` (issues #22 / #47)."""

from __future__ import annotations

import asyncio
import os

import httpx
from oxyroute import App
from tests._rsgi_test_transport import asgi_test_app


def test_urlencoded_form_fields() -> None:
    app = App()

    @app.post("/form", read_form_body=True)
    def form_route(form: dict) -> str:
        return f"{form.get('a')},{form.get('b')}"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post(
                "/form",
                content="a=1&b=two%20here",
                headers={"content-type": "application/x-www-form-urlencoded; charset=utf-8"},
            )
        assert r.status_code == 200, r.text
        assert r.text == "1,two here"

    asyncio.run(_run())


def test_multipart_file_and_field() -> None:
    app = App()

    @app.post("/up", read_form_body=True)
    def up(form: dict, files: list) -> str:
        f0 = files[0]
        data = f0["data"]
        return f"note={form.get('note')};name={f0['name']};len={len(data)}"

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            r = await c.post(
                "/up",
                data={"note": "n1"},
                files={"f": ("a.txt", b"hello", "text/plain")},
            )
        assert r.status_code == 200, r.text
        assert r.text == "note=n1;name=f;len=5"

    asyncio.run(_run())


def test_payload_too_large_413() -> None:
    """Isolated process: ``OXYROUTE_MAX_BODY_BYTES`` is cached on first body read."""
    import subprocess
    import sys
    from pathlib import Path

    repo = Path(__file__).resolve().parents[1]
    code = """
import asyncio
import os

import httpx
from oxyroute import App
from tests._rsgi_test_transport import asgi_test_app

os.environ["OXYROUTE_MAX_BODY_BYTES"] = "20"

app = App()

@app.post("/b", read_form_body=True)
def b(form: dict) -> str:
    return "ok" if not form else "bad"

async def main() -> None:
    transport = httpx.ASGITransport(app=asgi_test_app(app))
    async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
        r = await c.post(
            "/b",
            content="x" * 100,
            headers={"content-type": "application/x-www-form-urlencoded"},
        )
    assert r.status_code == 413
    err = r.json() if r.headers.get("content-type", "").startswith("application/json") else {}
    assert err.get("error") == "payload too large" or "payload" in (r.text or "")

asyncio.run(main())
"""
    env = os.environ.copy()
    env["PYTHONPATH"] = str(repo)
    subprocess.run([sys.executable, "-c", code], env=env, check=True, cwd=repo)
