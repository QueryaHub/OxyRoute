# Optional ASGI 3.0 bridge

[← Documentation index](index.md)

OxyRoute’s primary design is **RSGI** with Granian. For servers that only expose **ASGI 3.0** (`async def app(scope, receive, send)`), the Python `App` is also a callable: **`async def __call__(scope, receive, send)`**.

The module `oxyroute.asgi` builds a minimal RSGI-shaped `scope` / `protocol` and forwards to the same native **`handle_rsgi`** as `__rsgi__`. That keeps a single request pipeline in the Rust code.

## How responses get back to ASGI

The RSGI response helpers on the protocol object are **synchronous** from the Rust side. The ASGI implementation schedules `send` coroutines on the **current asyncio event loop** that was running when the request was accepted (`asyncio.run_coroutine_threadsafe` + `result()`), so the event loop in the process must be free to make progress when native code issues responses.

**Implications:** This bridge is a **practical adapter**, not a full reimplementation of RSGI under every ASGI host. Prefer **RSGI/Granian** for production with OxyRoute if you can.

## Limitations (current)

- **HTTP** ASGI `scope` only; other scope types are ignored
- RSGI features beyond what the bridge constructs are not modeled
- Error handling matches the RSGI path; unexpected combinations of servers and runtimes should be **tested** in your environment

## Local testing

The **httpx** project’s `ASGITransport` is used in `tests/test_asgi.py` to exercise the bridge without a real TCP server. That test is a good template for a smoke check.

## See also

- [RSGI and Granian](rsgi.md) — the recommended integration
- [Installation](installation.md) — `httpx` in `oxyroute[dev]`
