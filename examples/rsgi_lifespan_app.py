"""
Per-worker RSGI lifecycle: override ``__rsgi_init__`` / ``__rsgi_del__`` (issue #18).

Run (from the repo root after an editable / wheel install)::

    granian --interface rsgi examples.rsgi_lifespan_app:app

``examples/rsgi_app.py`` is the minimal app. This file shows **subclassing** ``App`` to
open resources when the host starts a worker. In-memory fields are **per OS process**;
with ``granian --workers N`` each worker has its own object graph — use Redis, a DB
pool, or a message bus for **cross-worker** or **cross-machine** state.

See https://github.com/QueryaHub/OxyRoute/blob/main/docs/rsgi.md (lifespan section).
"""

from __future__ import annotations

import time

from oxyroute import App


class LifespanApp(App):
    """Example: set attributes when the RSGI worker calls ``__rsgi_init__``."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.ready_at: float | None = None

    async def __rsgi_init__(self, *args, **kwargs) -> None:
        # Place per-process setup here (DB engine, pool, clients, asyncio primitives).
        self.ready_at = time.time()
        return None

    async def __rsgi_del__(self, *args, **kwargs) -> None:
        self.ready_at = None
        return None


app = LifespanApp(title="Lifespan example")


@app.get("/")
def root() -> str:
    t = app.ready_at
    if t is None:
        return "ok (lifespan not run — use a real RSGI server)"
    return f"ok boot_timestamp={t:.4f}"


@app.get("/meta")
def meta() -> str:
    """In multi-worker mode each worker has its own ``ready_at`` (not a global counter)."""
    t = app.ready_at
    return f"ready_at={t}"
