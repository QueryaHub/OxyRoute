"""
Per-worker RSGI lifecycle: override ``__rsgi_init__`` / ``__rsgi_del__`` (issue #18).

Run (from the repo root after an editable / wheel install)::

    granian --interface rsgi examples.rsgi_lifespan_app:app

``examples/rsgi_app.py`` is the minimal app. This file shows **subclassing** ``App`` to
open resources when the host starts a worker, using :attr:`oxyroute.app.App.state` and a
:func:`concurrent.futures.ThreadPoolExecutor` (typical for blocking I/O in sync handlers;
use ``asyncio`` primitives in ``__rsgi_init__`` when your stack is natively async).

In-memory data is **per OS process**; with ``granian --workers N`` each worker has its
own object graph — use Redis, a DB pool, or a message bus for **cross-worker** or
**cross-machine** state.

See https://github.com/QueryaHub/OxyRoute/blob/main/docs/rsgi.md (lifespan section).
"""

from __future__ import annotations

import asyncio
import time
from concurrent.futures import ThreadPoolExecutor

from oxyroute import App

_WORKERS = 4  # per-process pool size; not shared across ``granian --workers N``


class LifespanApp(App):
    """Example: attach per-process state when the RSGI worker calls ``__rsgi_init__``."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # ``state`` is a :class:`types.SimpleNamespace` on the base :class:`App`.
        self.state.ready_at = None
        self.state.thread_pool = None

    async def __rsgi_init__(self, *args, **kwargs) -> None:
        # ``asyncio`` primitive example (use from async callables you control).
        self.state.bg_limit = asyncio.Semaphore(8)
        # Thread pool: run blocking work via ``run_in_executor(self.state.thread_pool, ...)``
        # from async glue, or keep sync handlers cheap and use the pool in dependencies.
        self.state.thread_pool = ThreadPoolExecutor(
            max_workers=_WORKERS,
            thread_name_prefix="rsgi",
        )
        self.state.ready_at = time.time()
        return None

    async def __rsgi_del__(self, *args, **kwargs) -> None:
        pool = self.state.thread_pool
        if pool is not None:
            pool.shutdown(wait=True)
        self.state.ready_at = None
        self.state.thread_pool = None
        if hasattr(self.state, "bg_limit"):
            del self.state.bg_limit
        return None


app = LifespanApp(title="Lifespan example")


@app.get("/")
def root() -> str:
    t = app.state.ready_at
    if t is None:
        return "ok (lifespan not run — use a real RSGI server)"
    return f"ok boot_timestamp={t:.4f}"


@app.get("/meta")
def meta() -> str:
    """In multi-worker mode each worker has its own ``ready_at`` (not a global counter)."""
    t = app.state.ready_at
    return f"ready_at={t}"


@app.get("/pool")
def pool_info() -> str:
    p = app.state.thread_pool
    if p is None:
        return "thread_pool=off"
    return f"thread_pool max_workers={_WORKERS} (per process, not across workers)"

