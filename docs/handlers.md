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
| `query` | Request has a query string | A Python `dict` of string keys and values, **percent-decoded**; `+` in values is treated as a space, matching `application/x-www-form-urlencoded` / URLSearchParams ([WHATWG](https://url.spec.whatwg.org/#urlencoded-parsing)). **Duplicate keys** are last-wins (a plain `dict`, not a multimap) |
| `json` | `read_json_body` is true and body parses as JSON | `dict`/list/values as converted from `serde_json` to Python |
| `form` | `read_form_body` is true on **POST**, **PUT**, **PATCH**, or **DELETE** | `dict` of string keys to string values (same last-wins semantics as `query`). Parsed from `application/x-www-form-urlencoded` or non-file parts of `multipart/form-data` |
| `files` | `read_form_body` is true and the request is `multipart/form-data` | `list` of `dict`s with `name`, `filename` (`str` or missing), `content_type`, and `data` (`bytes`). In-memory only; there is no streaming spool to disk in the current implementation |
| `body` | Raw body bytes when JSON and form modes are not used | `bytes`. **Not** passed when `read_form_body` is enabled (use `form` / `files` instead) |
| `protocol` | Handler declares a `protocol` parameter (or `**kwargs`) | Underlying RSGI protocol object for advanced response flows (e.g. SSE via `send_sse`) |
| `claims` | `require_jwt` is true and the JWT validates | The decoded JSON claims as a Python object (typically a `dict`) |
| Named dependencies | `dependencies=[("name", factory), ...]` | Return value of each factory, in order (see [dependencies.md](dependencies.md)). Only dependencies whose **names** appear on the route handler’s signature (or `**kwargs`) are passed to the handler—intermediate-only dependencies are not forwarded |

**JWT:** if `require_jwt` is set but validation fails, the **handler is not called**; the response is 401 (or a dedicated “Expired” string for expired signature when applicable). See [jwt.md](jwt.md).

### Form bodies (`read_form_body`)

Register routes with `read_form_body=True` on `post` / `put` / `patch` (and `delete` if you accept a body). This is **mutually exclusive** with `read_json_body` for the same route (the framework forces JSON off when form mode is on).

- **`Content-Type: application/x-www-form-urlencoded`** — the body is parsed like a query string (percent-decoding, `+` as space).
- **`Content-Type: multipart/form-data; boundary=...`** — parts with a **filename** are collected as `files`; other parts go into `form`.
- Wrong or missing `Content-Type` when the body is non-empty → **400** (missing type) or **415** (not a form type). Malformed multipart → **400** with a JSON error.
- **Size limit:** the full body is buffered in memory before parsing. The default maximum is **8 MiB**; override with the environment variable **`OXYROUTE_MAX_BODY_BYTES`** (set to `0` to disable the check—**not recommended** in production). Oversized bodies → **413** with `{"error":"payload too large"}`.

Only parameters that appear in the handler signature (or `**kwargs`) receive `form` / `files`, the same as for `query` and dependencies.

## Sync and async

Both **synchronous** and **asynchronous** callables are supported. If a handler is a coroutine function, the returned awaitable is run on the async runtime integration used by the extension.

## Return values and response mapping

The Rust layer maps the return value to an HTTP response:

- **`str` or `&str` (via `str` on Python object):** **200**, `text/plain; charset=utf-8`
- **`bytes`:** **200**, `application/octet-stream`
- **Any other object (dict, list, custom):** if not a special dict, the value is serialized with **`json.dumps`** and returned as **200** with `application/json; charset=utf-8`
- **`dict` with `status` and `body` keys:** if both are present in a way the native code recognizes, a **custom status code** and body (as string) is returned for plain responses (content type fixed in the current path—see `src/dispatch.rs` for the exact check)
- **`dict` with `status`, `body`, and optional `headers` / `cookies`:** same as structured `Response` below; body is encoded like `json` / `str` / `bytes` (not only `str(body)`). `cookies` is a list of raw `Set-Cookie` header values
- **`Response` (from `oxyroute`):** `status`, `body` (optional; `str`, `bytes`, JSON-serializable, or `None` for empty), optional `headers` (`str` → `str`), optional `cookies` (list of strings for `Set-Cookie` lines). If `headers` does not set `content-type`, it is derived from the body type. The RSGI response is built with the full header list

For precise behavior and edge cases, refer to the implementation in the repository’s `src/dispatch.rs` and `src/response.rs`.

## Errors in handlers and dependencies

### `HTTPException` (non-500 responses)

Raise **`HTTPException`** from `oxyroute` to return a specific **status code** and JSON body without a `try` / `except` at every call site:

```python
from oxyroute import HTTPException

raise HTTPException(404, "not found")
raise HTTPException(422, {"errors": [...]})  # body is this JSON value
raise HTTPException(400, "bad", headers={"X-Reason": "check"})  # optional extra headers
```

- String or other non-`dict` / non-`list` **`detail`** becomes `{"detail": ...}`.
- **`detail=None`** uses a short default message derived from the status (HTTP reason phrase when available).
- **`Content-Type`** is set to `application/json; charset=utf-8` unless you supply a `content-type` in **`headers`**.
- Works from **route handlers**, **dependencies**, **middleware** (when they run in Python), and when **mapping the return value** to a response.

### Other exceptions → 500

If a **dependency factory** or the **route handler** raises any other Python exception (or building the response fails), the server answers with **500** and a small **JSON** body: `{"error":"internal server error"}`. Exception text and tracebacks are **not** included in the response by default (to avoid leaking internals to clients).

Set the environment variable **`OXYROUTE_DEBUG=1`** (or `true`) to include a **`detail`** string in that JSON for the same error and to log more at the `log` crate target **`oxyroute`** (see `RUST_LOG`, e.g. `RUST_LOG=oxyroute=error`).

There is no **`register_exception_handler`** API yet; map custom exception types by catching them in Python or by a small wrapper.

## Pre-route hook (`set_middleware`)

`App.set_middleware(f)` sets an **optional** callable taking `(scope, protocol)` (same RSGI-like objects as the rest of the stack). It runs **after** the path and method are known, **before** the request body is read or routes are matched.

- Return **`None`**: continue with normal routing and body read.
- Return **any other value**: use the same mapping as a route return value (`Response`, dict, `str`, etc.); the response is sent and **the route handler and body are skipped** (e.g. cheap CORS preflight on `OPTIONS` without consuming a `POST` body).

For a configurable **`allow_origins` / `allow_methods` / `allow_headers`** flow plus native merging of CORS headers into normal responses, use **`CORSConfig`** and **`apply_cors`** (see [cors.md](cors.md)). For a **browser security** header preset (HSTS, `X-Content-Type-Options`, etc.), use **`SecurityHeadersConfig`** and **`set_security_headers`** (see [security-headers.md](security-headers.md)). For **CSRF** when you rely on **cookies** and mutating methods, use **`CSRFConfig`** and **`apply_csrf`** (or **`csrf_layer`** with `apply_cors`); see [csrf.md](csrf.md).

## See also

- [Routing](routing.md)
- [JWT](jwt.md)
- [Dependencies](dependencies.md)
- [CORS](cors.md)
- [Security headers](security-headers.md)
- [CSRF](csrf.md)
- [SSE](sse.md)
