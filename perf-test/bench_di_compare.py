#!/usr/bin/env python3
"""
DI/DX performance comparison benchmark.

Starts app_di_compare via Granian (subprocess), then hammers each route
with concurrent httpx requests and reports req/s + overhead vs baseline.

Usage:
    python perf-test/bench_di_compare.py [--duration 5] [--concurrency 32] [--workers 1]

Requirements: granian, httpx (already in .venv)
"""
from __future__ import annotations

import argparse
import asyncio
import importlib.util
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).parent.parent
VENV_PY = ROOT / ".venv" / "bin" / "python"
PYTHON = str(VENV_PY) if VENV_PY.exists() else sys.executable


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


async def wait_http(port: int, path: str = "/nodep", timeout: float = 30.0) -> None:
    import httpx

    deadline = time.monotonic() + timeout
    async with httpx.AsyncClient() as client:
        while time.monotonic() < deadline:
            try:
                r = await client.get(f"http://127.0.0.1:{port}{path}", timeout=1.0)
                if r.status_code < 500:
                    return
            except Exception:
                pass
            await asyncio.sleep(0.05)
    raise RuntimeError(f"server on port {port} did not become ready in {timeout}s")


async def bench_route(
    port: int,
    path: str,
    method: str = "GET",
    body: bytes | None = None,
    headers: dict | None = None,
    duration: float = 5.0,
    concurrency: int = 32,
) -> float:
    """Return requests/second."""
    import httpx

    url = f"http://127.0.0.1:{port}{path}"
    hdrs = dict(headers or {})
    count = 0
    stop = False

    async def worker() -> None:
        nonlocal count
        async with httpx.AsyncClient(http2=False) as client:
            while not stop:
                try:
                    if method == "POST":
                        await client.post(url, content=body, headers=hdrs, timeout=5.0)
                    else:
                        await client.get(url, headers=hdrs, timeout=5.0)
                    count += 1
                except Exception:
                    pass

    tasks = [asyncio.create_task(worker()) for _ in range(concurrency)]
    start = time.monotonic()
    await asyncio.sleep(duration)
    stop = True  # type: ignore[assignment]
    for t in tasks:
        t.cancel()
    await asyncio.gather(*tasks, return_exceptions=True)
    elapsed = time.monotonic() - start
    return count / elapsed


SCENARIOS: list[tuple[str, str, str, bytes | None, dict | None]] = [
    # (label, method, path, body, headers)
    ("nodep",    "GET",  "/nodep",    None, None),
    ("dep1",     "GET",  "/dep1",     None, None),
    ("dep3",     "GET",  "/dep3",     None, None),
    ("depchain", "GET",  "/depchain", None, None),
    ("depasync", "GET",  "/depasync", None, None),
    ("dep1json", "POST", "/dep1json", b'{"x":1,"y":"hello"}',
     {"Content-Type": "application/json"}),
]


def print_table(results: dict[str, float]) -> None:
    baseline = results.get("nodep", 0.0)
    print(f"\n{'scenario':<14} {'req/s':>10} {'vs baseline':>12} {'overhead':>10}")
    print("-" * 52)
    for label, _, _, _, _ in SCENARIOS:
        rps = results.get(label, 0.0)
        if baseline > 0 and rps > 0:
            ratio = rps / baseline
            pct = (ratio - 1.0) * 100.0
            print(f"{label:<14} {rps:>10.0f} {ratio:>11.3f}x {pct:>+9.1f}%")
        else:
            print(f"{label:<14} {'n/a':>10} {'n/a':>12} {'n/a':>10}")
    print()


async def main(duration: float, concurrency: int, workers: int) -> None:
    import httpx  # noqa: F401 – just ensure it's present

    port = free_port()
    env_path = str(ROOT)
    cmd = [
        PYTHON, "-m", "granian", "app_di_compare:app",
        "--host", "127.0.0.1",
        "--port", str(port),
        "--interface", "rsgi",
        "--workers", str(workers),
    ]
    print(f"bench_di_compare: OxyRoute DI/DX overhead (RSGI via Granian)")
    print(f"  duration={duration}s  concurrency={concurrency}  workers={workers}")
    print()

    proc = subprocess.Popen(
        cmd,
        cwd=str(ROOT / "perf-test"),
        env={**__import__("os").environ, "PYTHONPATH": env_path},
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        await wait_http(port)
        results: dict[str, float] = {}
        total = len(SCENARIOS)
        for idx, (label, method, path, body, headers) in enumerate(SCENARIOS, 1):
            desc = f"{method} {path}"
            print(f"  [{idx}/{total}] {label:<12} {desc}")
            rps = await bench_route(
                port, path,
                method=method, body=body, headers=headers,
                duration=duration, concurrency=concurrency,
            )
            results[label] = rps
        print_table(results)
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="DI/DX overhead benchmark")
    parser.add_argument("--duration",    type=float, default=5.0,  help="seconds per route")
    parser.add_argument("--concurrency", type=int,   default=32,   help="concurrent requests")
    parser.add_argument("--workers",     type=int,   default=1,    help="Granian worker count")
    args = parser.parse_args()
    asyncio.run(main(args.duration, args.concurrency, args.workers))
