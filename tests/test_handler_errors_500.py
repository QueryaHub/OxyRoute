"""Handler / dependency exceptions map to HTTP 500 (issue #3)."""

from __future__ import annotations

import asyncio
import json
import os

import httpx
from oxyroute import App
from oxyroute.testing import asgi_test_app


def _make_app() -> App:
    app = App()

    @app.get("/boom")
    def boom() -> str:
        raise ValueError("super_secret_token")

    def bad_dep() -> str:
        raise RuntimeError("dep fail")

    @app.get("/dep", dependencies=[("x", bad_dep)])
    def with_dep(x: str) -> str:  # pragma: no cover - dep fails first
        return x

    return app


def test_500_no_exception_text_by_default() -> None:
    os.environ.pop("OXYROUTE_DEBUG", None)
    app = _make_app()

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://t") as c:
            r = await c.get("/boom")
        assert r.status_code == 500
        data = json.loads(r.text)
        assert data.get("error") == "internal server error"
        assert "detail" not in data
        assert "super_secret" not in r.text

    asyncio.run(_run())


def test_500_includes_detail_when_debug_env() -> None:
    """Isolated process: ``OXYROUTE_DEBUG`` is cached on first error path."""
    import subprocess
    import sys
    from pathlib import Path

    repo = Path(__file__).resolve().parents[1]
    code = """
import asyncio
import json
import os

import httpx
from oxyroute import App
from oxyroute.testing import asgi_test_app

os.environ["OXYROUTE_DEBUG"] = "1"

app = App()

@app.get("/boom")
def boom() -> str:
    raise ValueError("super_secret_token")

async def main() -> None:
    transport = httpx.ASGITransport(app=asgi_test_app(app))
    async with httpx.AsyncClient(transport=transport, base_url="http://t") as c:
        r = await c.get("/boom")
    assert r.status_code == 500
    data = json.loads(r.text)
    assert "detail" in data
    assert "super_secret_token" in data["detail"] or "ValueError" in data["detail"]

asyncio.run(main())
"""
    env = os.environ.copy()
    env["PYTHONPATH"] = str(repo)
    subprocess.run([sys.executable, "-c", code], env=env, check=True, cwd=repo)


def test_dependency_error_returns_500() -> None:
    os.environ.pop("OXYROUTE_DEBUG", None)
    app = _make_app()

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://t") as c:
            r = await c.get("/dep")
        assert r.status_code == 500
        assert "internal" in r.text.lower()

    asyncio.run(_run())
