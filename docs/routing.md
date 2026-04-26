# Routing

[← Documentation index](index.md)

OxyRoute uses the **[matchit](https://crates.io/crates/matchit) 0.7** style of patterns (see upstream docs for the full pattern grammar). Typical **path parameters** use a `:` prefix, for example:

- `/items/:id`
- `/users/:user_id/posts/:post_id`

## HTTP methods

The Python `App` class exposes:

- `get`, `post`, `put`, `delete`

Each is a decorator that registers a route and returns the handler unchanged (so you can stack multiple decorators on the same function only if you design for it—usually one method/path per handler is enough).

## Matching and 404

- The request **path** from RSGI is matched against the tree for the request **method**.
- If no route matches, the response is **404** with a plain `Not Found` body (from the Rust dispatch layer).
- Non-`http` RSGI scopes are ignored in the current implementation (no response is sent for unknown `proto` values).

## OpenAPI and discovery

If OpenAPI is enabled, route registration also updates a minimal OpenAPI document. See [openapi.md](openapi.md).

## See also

- [Handlers](handlers.md) — how matched parameters become keyword arguments
- [Quick example](../examples/rsgi_app.py) in the repository
