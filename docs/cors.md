# CORS

[← Documentation index](index.md)

Cross-origin resource sharing is supported in two layers:

1. **`CORSConfig` + `apply_cors(app, config)`** — sets the config on the native app (so successful route and middleware responses get CORS headers merged) and installs a **pre-route** middleware that answers **CORS preflight** (`OPTIONS` with `Access-Control-Request-Method`) without reading the body.
2. **`App.set_cors(config)`** — only registers the config for response header merging; you must still handle preflight yourself (e.g. with `set_middleware`) if browsers need it.

## Basic usage

```python
from oxyroute import App, CORSConfig, apply_cors

app = App()
apply_cors(
    app,
    CORSConfig(
        allow_origins=["https://my.frontend.example"],
        allow_methods=["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
        allow_headers=["*"],
    ),
)


@app.get("/api/x")
def x() -> dict:
    return {"ok": True}
```

## Combining with another middleware

OxyRoute exposes a **single** pre-route hook (`set_middleware`). Calling `apply_cors` replaces that hook with an internal function. To run your own logic **after** CORS preflight is ruled out, pass **`chain=`**:

```python
def my_mw(scope, protocol):
    # runs only when apply_cors did not return a preflight response
    return None


apply_cors(app, config, chain=my_mw)
```

If you need the opposite order, call `set_middleware` yourself and use `set_cors` only, or call `set_middleware` with a function that calls your code first, then delegates preflight to `config.preflight_response(scope)`.

## Configuration fields

| Field | Role |
|--------|------|
| `allow_origins` | List of allowed `Origin` values, or `["*"]` when not using credentials. |
| `allow_methods` | HTTP methods allowed in preflight and echoed in `Access-Control-Allow-Methods`. |
| `allow_headers` | `["*"]` or a list of permitted request header names for preflight. |
| `expose_headers` | Optional list; sent as `Access-Control-Expose-Headers` on real responses. |
| `allow_credentials` | If true, `Access-Control-Allow-Credentials: true` and `*` cannot be used as the origin. |
| `max_age` | Seconds for `Access-Control-Max-Age` on preflight, or `None` to omit. |

## See also

- [Handlers](handlers.md) — `set_middleware` and return mapping
- [RSGI and Granian](rsgi.md)
