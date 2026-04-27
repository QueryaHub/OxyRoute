"""
Optional hello-world RPS comparison: OxyRoute (RSGI) vs FastAPI (ASGI) via ``wrk``.

Skipped unless ``OXYROUTE_BENCH=1``. Requires ``pip install -e '.[bench]'``, ``wrk``,
and a built ``oxyroute`` extension (``maturin develop`` / wheel).
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

pytest.importorskip("granian")
pytest.importorskip("fastapi")

skip_bench = pytest.mark.skipif(
    os.environ.get("OXYROUTE_BENCH") != "1",
    reason="set OXYROUTE_BENCH=1 to run wrk comparison (optional; not for CI by default)",
)


@skip_bench
@pytest.mark.bench
def test_bench_hello_script_oxyroute_vs_fastapi() -> None:
    if not shutil.which("wrk"):
        pytest.skip("wrk not on PATH")
    if not shutil.which("bash"):
        pytest.skip("bash not on PATH")
    root = Path(__file__).resolve().parent.parent
    script = root / "perf-test" / "bench_hello.sh"
    if not script.is_file():
        pytest.skip("perf-test/bench_hello.sh not found")
    env = {**os.environ, "OXYROUTE_BENCH_DURATION": "1s", "OXYROUTE_BENCH_CONNECTIONS": "8"}
    proc = subprocess.run(
        ["bash", str(script)],
        cwd=str(root),
        env=env,
        capture_output=True,
        text=True,
        check=False,
        timeout=120,
    )
    out = proc.stdout + proc.stderr
    assert proc.returncode == 0, out
    assert "OxyRoute" in out
    assert "FastAPI" in out
    assert "Delta" in out
    assert "Requests/sec" in out
