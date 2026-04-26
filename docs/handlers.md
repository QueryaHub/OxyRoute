# Handlers

[← Documentation index](index.md)

Handlers are normal Python callables. The Rust dispatcher builds **keyword arguments** and invokes:

```python
handler(**kwargs)
```

(Implementation detail: a dict is used with only the keys described below, not arbitrary `**kwargs` from the user unless you use `**kwargs` in the signature to collect extras.)

## Injected parameters

| Name | When present | Meaning |
|------|----------------|--------|
| Path parameters | Matched from the route, e.g. `id` for `/items/:id` | String-like values, with coercion for `int` / `float` / `bool` / `str` in the Rust layer where applicable |
| `query` | Request has a query string | A Python `dict` of string keys/values (see implementation for parsing rules) |
| `json` | `read_json_body` is true and body parses as JSON | `dict`/list/values as converted from `serde_json` to Python |
| `body` | Raw body bytes, when JSON is not used or empty | `bytes` |
| `claims` | `require_jwt` is true and the JWT validates | The decoded JSON claims as a Python object (typically a `dict`) |
| Named dependencies | `dependencies=[("name", factory), ...]` | Return value of each factory, in order (see [dependencies.md](dependencies.md)) |

**JWT:** if `require_jwt` is set but validation fails, the **handler is not called**; the response is 401 (or a dedicated “Expired” string for expired signature when applicable). See [jwt.md](jwt.md).

## Sync and async

Both **synchronous** and **asynchronous** callables are supported. If a handler is a coroutine function, the returned awaitable is run on the async runtime integration used by the extension.

## Return values and response mapping

The Rust layer maps the return value to an HTTP response:

- **`str` or `&str` (via `str` on Python object):** **200**, `text/plain; charset=utf-8`
- **`bytes`:** **200**, `application/octet-stream`
- **Any other object (dict, list, custom):** if not a special dict, the value is serialized with **`json.dumps`** and returned as **200** with `application/json; charset=utf-8`
- **`dict` with `status` and `body` keys:** if both are present in a way the native code recognizes, a **custom status code** and body (as string) is returned for plain responses (content type fixed in the current path—see `src/dispatch.rs` for the exact check)

For precise behavior and edge cases, refer to the implementation in the repository’s `src/dispatch.rs` and `src/response.rs`.

## See also

- [Routing](routing.md)
- [JWT](jwt.md)
- [Dependencies](dependencies.md)
