"""Minimal OxyRoute hello-world for `bench_hello.sh` (RSGI)."""

from __future__ import annotations

from oxyroute import App

app = App()


@app.get("/")
def root() -> str:
    return "hello"
