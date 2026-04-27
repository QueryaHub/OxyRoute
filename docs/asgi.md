# Optional ASGI 3.0 bridge

[← Documentation index](index.md)

OxyRoute’s primary design is **RSGI** with Granian. For servers that only expose **ASGI 3.0** (`async def app(scope, receive, send)`), the Python `App` is also a callable: **`async def __call__(scope, receive, send)`**.

The module `oxyroute.asgi` builds a minimal RSGI-shaped `scope` / `protocol` and forwards to the same native **`handle_rsgi`** as `__rsgi__`. That keeps a single request pipeline in the Rust code.

## How responses get back to ASGI

The RSGI response helpers on the protocol object are **synchronous** from the Rust side. The ASGI implementation bridges this through a thread-safe outgoing queue: sync `response_*` calls enqueue ASGI messages from the worker thread, and an async drain task on the main loop performs `send(...)` in order.

**Why the bridge does not `await` on that loop directly:** Awaiting the native `handle_rsgi` coroutine on the same loop that must process outgoing messages can **deadlock**. The bridge therefore runs `handle_rsgi` in a **thread-pool** worker using `asyncio.run()` on a **separate** event loop, while the main ASGI loop drains the queued `http.response.*` events.

**Implications:** This bridge is a **practical adapter**, not a full reimplementation of RSGI under every ASGI host. Prefer **RSGI/Granian** for production with OxyRoute if you can. With **Uvicorn**, use a **single worker** process unless you are sure the combination is safe in your app (the usual `workers=N` + in-process `asyncio` footguns can still apply to third-party `asyncio` use).

## Limitations (current)

- **HTTP** ASGI `scope` only; other scope types are ignored
- RSGI features beyond what the bridge constructs are not modeled
- Error handling matches the RSGI path; unexpected combinations of servers and runtimes should be **tested** in your environment

## Local testing

The **httpx** project’s `ASGITransport` is used in `tests/test_asgi.py` to exercise the bridge without a real TCP server. `tests/test_asgi_stress.py` runs **50** concurrent `GET` requests to guard against cross-thread/loop deadlocks in the bridge.

## See also

- [RSGI and Granian](rsgi.md) — the recommended integration
- [Installation](installation.md) — `httpx` in `oxyroute[dev]`
