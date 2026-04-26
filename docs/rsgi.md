# RSGI and Granian

[← Documentation index](index.md)

OxyRoute targets **RSGI** (see [Granian RSGI specification](https://github.com/emmett-framework/granian/blob/master/docs/spec/RSGI.md)) rather than only ASGI. With Granian, prefer:

```bash
granian --interface rsgi your_module:app
```

## Application entry

The Python `oxyroute.app.App` class implements the async RSGI entry that Granian looks for when present:

- **`async def __rsgi__(self, scope, protocol):`** — forwards to the native `App.handle_rsgi(scope, protocol)` so the Rust core can read `scope` (path, method, query, headers) and the RSGI `protocol` (body, response helpers).

`scope` and `protocol` are **objects with attributes** (not plain ASGI dicts with `receive` / `send`). The body is read by awaiting a callable on the protocol (as in the linked spec). OxyRoute’s internal implementation matches that RSGI shape.

## Lifespan (optional)

`App` defines no-op coroutines for servers that expect them. Implementations use `*args, **kwargs` so **Granian** (and any server that passes extra parameters to worker lifespan hooks) can call them without a `TypeError`:

- `async def __rsgi_init__(self, *args, **kwargs) -> None` — per-worker (or per-process) **startup** in the RSGI host
- `async def __rsgi_del__(self, *args, **kwargs) -> None` — **teardown** when the worker stops

You can **override** these in a **subclass** of `App` to open DB pools, HTTP clients, `asyncio` primitives, etc. The default base implementation does nothing.

### Workers and shared state (Granian)

- With **`granian --workers N`**, the server runs **N independent worker processes** (typical for CPU-bound HTTP). Each process loads your module, constructs your `app`, and may call `__rsgi_init__` **once per worker** (exact call pattern is defined by the server; see [Granian’s docs](https://github.com/emmett-framework/granian)). **In-memory** attributes you set in `__rsgi_init__` are **not** shared between workers: two requests may hit different processes and see different `self.foo`.
- If you use **a single worker** or run under **in-process** tests, one process is enough for a module-level or `self` cache for development only.
- For **user sessions, counts, or singletons** across the whole deployment, use **external** storage (Postgres, Redis, etc.); a DB **connection pool** created in `__rsgi_init__` is still a good pattern: one pool **per process**, many requests share connections inside that pool.

### Factory pattern

As an alternative to a subclass, you can expose `app` from a factory:

```python
def create_app() -> App:
    a = App(title="x")
    # register routes on `a` …
    return a

app = create_app()
```

`granian` imports `app` once per worker, so the factory runs in each process that loads the module.

### Example in the repository

- [`examples/rsgi_app.py`](../examples/rsgi_app.py) — minimal RSGI app  
- [`examples/rsgi_lifespan_app.py`](../examples/rsgi_lifespan_app.py) — subclass with `__rsgi_init__` / `__rsgi_del__` and `ready_at` used from handlers

## When to use ASGI instead

If your host only speaks **ASGI 3.0** (for example `uvicorn` with stock ASGI), you can use the optional ASGI bridge: `App` is also a callable `async def __call__(scope, receive, send)`. See [asgi.md](asgi.md) for trade-offs and limitations.

## See also

- [Handlers](handlers.md) — what the Rust core passes into your functions
- [Routing](routing.md) — how paths are matched
