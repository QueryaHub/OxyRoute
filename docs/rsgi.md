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

- `async def __rsgi_init__(self, *args, **kwargs) -> None`
- `async def __rsgi_del__(self, *args, **kwargs) -> None`

You can override these in a subclass if you need startup/shutdown hooks; the default does nothing.

## When to use ASGI instead

If your host only speaks **ASGI 3.0** (for example `uvicorn` with stock ASGI), you can use the optional ASGI bridge: `App` is also a callable `async def __call__(scope, receive, send)`. See [asgi.md](asgi.md) for trade-offs and limitations.

## See also

- [Handlers](handlers.md) — what the Rust core passes into your functions
- [Routing](routing.md) — how paths are matched
