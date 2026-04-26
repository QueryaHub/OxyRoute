"""
Subprocess e2e: real Granian server with --interface rsgi (issue #12).

Skip if ``granian`` is not installed (``pip install -e .[dev]`` or add granian in CI).
"""

from __future__ import annotations

import os
import socket
import subprocess
import sys
import tempfile
import textwrap
import time

import httpx
import pytest

pytest.importorskip("granian")


def _free_port() -> int:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def test_granian_rsgi_returns_handler_body() -> None:
    d = tempfile.mkdtemp()
    p = os.path.join(d, "e2e_rsgi.py")
    with open(p, "w", encoding="utf-8") as f:
        f.write(
            textwrap.dedent(
                """\
                from oxyroute import App
                app = App()

                @app.get("/")
                def root() -> str:
                    return "e2e-granian-ok"
                """
            )
        )
    port = _free_port()
    env = os.environ.copy()
    env["PYTHONPATH"] = d
    cmd: list[str] = [
        sys.executable,
        "-m",
        "granian",
        "e2e_rsgi:app",
        "--host",
        "127.0.0.1",
        "--port",
        str(port),
        "--interface",
        "rsgi",
        "--workers",
        "1",
    ]
    proc = subprocess.Popen(
        cmd,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        deadline = time.time() + 20.0
        last_err: str | None = None
        while time.time() < deadline:
            time.sleep(0.05)
            try:
                with httpx.Client(timeout=1.0) as c:
                    r = c.get(f"http://127.0.0.1:{port}/")
                if r.status_code == 200 and r.text == "e2e-granian-ok":
                    return
            except (httpx.HTTPError, OSError) as e:
                last_err = str(e)
            if proc.poll() is not None:
                err = proc.stderr.read() if proc.stderr else ""
                raise AssertionError(
                    f"granian exited early: code={proc.returncode} stderr={err!r} last={last_err!r}"
                )
        err = proc.stderr.read() if proc.stderr else ""
        raise AssertionError(f"server did not become ready. last={last_err!r} stderr={err!r}")
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
