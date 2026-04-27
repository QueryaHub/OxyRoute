# Routing

[← Documentation index](index.md)

OxyRoute uses the **[matchit](https://crates.io/crates/matchit) 0.7** style of patterns (see upstream docs for the full pattern grammar). Typical **path parameters** use a `:` prefix, for example:

- `/items/:id`
- `/users/:user_id/posts/:post_id`

## HTTP methods

The Python `App` class exposes:

- `get`, `post`, `put`, `patch`, `delete`

Each is a decorator that registers a route and returns the handler unchanged (so you can stack multiple decorators on the same function only if you design for it—usually one method/path per handler is enough).

## Matching and 404

- The request **path** from RSGI is matched against the tree for the request **method**.
- If no route matches, the response is **404** with a plain `Not Found` body (from the Rust dispatch layer).
- Non-`http` RSGI scopes are ignored in the current implementation (no response is sent for unknown `proto` values).

## Compiled routing snapshot

- OxyRoute now **auto-compiles** the route snapshot on the first HTTP request, so hot-path
  matching uses the lock-free compiled tables even if `freeze()` was not called explicitly.
- Calling `freeze()` is still supported and remains the strict "no more route registration" switch.
- If routes are added before `freeze()`, OxyRoute invalidates the previous compiled snapshot and
  lazily rebuilds it on the next request.

## OpenAPI and discovery

If OpenAPI is enabled, route registration also updates a minimal OpenAPI document. See [openapi.md](openapi.md).

## Sub-routers (`APIRouter` + `include_router`)

For shared path prefixes (for example ``/api/v1/...``), build routes on an **`APIRouter`**, then mount them on the main **`App`** with **`include_router(router, prefix=...)`**. Optional keyword arguments to **`include_router`** are merged with each route’s own options; **per-route** options win on key conflicts (same as FastAPI’s `include_router` defaults).

- **`oxyroute.APIRouter`**: the same `get` / `post` / `put` / `patch` / `delete` / `options` decorators as `App`, but they only *record* routes.
- **`App.include_router(router, prefix="")`**: registers all recorded routes on the native `App` with `join_path(prefix, route_path)` (leading slashes normalized).
- **Nesting:** `parent_router.include_router(child_router, prefix="…")` flattens child routes with that prefix, then the parent is mounted on `App` as usual.
- **Duplicate** path + method: still rejected at register time (same as registering twice on `App`).

Example: [examples/routers_include_app.py](../examples/routers_include_app.py)

## See also

- [Handlers](handlers.md) — how matched parameters become keyword arguments
- [Quick example](../examples/rsgi_app.py) in the repository
