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

Granian’s RSGI worker calls **sync** lifespan hooks with a **non-running** event loop:

```python
def __rsgi_init__(self, loop):
    loop.run_until_complete(...)
```

OxyRoute’s base `App` implements that contract. Prefer overriding the async helpers:

- **`async def on_startup(self) -> None`** — per-worker startup (DB pools, clients, …)
- **`async def on_shutdown(self) -> None`** — teardown (base closes the SQLx pool if any)

The framework’s sync **`__rsgi_init__(loop)`** / **`__rsgi_del__(loop)`** call `loop.run_until_complete` on those coroutines. When called **without** a loop (tests / `TestClient`), they **return** the coroutine so callers can `await` it.

**Warning:** Do not override `__rsgi_init__` as `async def`. Under Granian the coroutine is never awaited (`coroutine was never awaited`), so pools never open. Override **`on_startup`** / **`on_shutdown`** instead.

Every `App` exposes **`app.state`**, a `types.SimpleNamespace` for attaching **per-process** objects. Use it in `on_startup` (or a factory) instead of ad hoc attributes on `self` if you want a single obvious place for shared services; it is the same not-shared-across-processes story as any other in-memory `App` data.

### Workers and shared state (Granian)

- With **`granian --workers N`**, the server runs **N independent worker processes** (typical for CPU-bound HTTP). Each process loads your module, constructs your `app`, and may call `__rsgi_init__` **once per worker** (exact call pattern is defined by the server; see [Granian’s docs](https://github.com/emmett-framework/granian)). **In-memory** attributes you set in `on_startup` are **not** shared between workers: two requests may hit different processes and see different `self.foo`.
- If you use **a single worker** or run under **in-process** tests, one process is enough for a module-level or `self` cache for development only.
- For **user sessions, counts, or singletons** across the whole deployment, use **external** storage (Postgres, Redis, etc.); a DB **connection pool** created in `on_startup` is still a good pattern: one pool **per process**, many requests share connections inside that pool.

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
- [`examples/rsgi_lifespan_app.py`](../examples/rsgi_lifespan_app.py) — subclass with `on_startup` / `on_shutdown` and `ready_at` used from handlers

## See also

- [Handlers](handlers.md) — what the Rust core passes into your functions
- [Routing](routing.md) — how paths are matched
- [OpenAPI](openapi.md) — docs UI and enriched `/openapi.json`
