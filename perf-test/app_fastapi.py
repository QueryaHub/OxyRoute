"""Minimal FastAPI hello-world for `bench_hello.sh` (ASGI under Granian)."""

from __future__ import annotations

from fastapi import FastAPI

app = FastAPI()


@app.get("/")
def root() -> str:
    return "hello"
