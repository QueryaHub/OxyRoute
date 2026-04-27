"""Minimal FastAPI hello-world for `bench_hello.sh` (ASGI under Granian)."""

from __future__ import annotations

from fastapi import FastAPI
from fastapi.responses import PlainTextResponse

app = FastAPI()


@app.get("/", response_class=PlainTextResponse)
def root() -> str:
    return "hello"
