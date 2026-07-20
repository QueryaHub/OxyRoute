# Usage guide

[← Documentation index](index.md)

This guide is the recommended end-to-end reference for using OxyRoute as an
application framework. It describes the current **v0.5.0** behavior: OxyRoute is
**RSGI-only** and is intended to run behind **Granian** with `--interface rsgi`.
The removed ASGI bridge is not part of the supported runtime path.

## Minimal application

Create `app.py`:

```python
from oxyroute import App

app = App(title="Example API")


@app.get("/")
def root() -> str:
    return "ok"


@app.get("/users/:user_id")
def get_user(user_id: int, query: dict[str, str] | None = None) -> dict:
    return {"user_id": user_id, "query": query or {}}
```

Run with Granian:

```bash
granian app:app --interface rsgi --host 127.0.0.1 --port 8000
```

For multiple worker processes:

```bash
granian app:app --interface rsgi --host 0.0.0.0 --port 8000 --workers 2
```

Each worker is a separate process. In-memory state is **not shared** across
workers; use external storage for sessions, counters, queues, and durable state.

## Install

From PyPI, once published:

```bash
pip install oxyroute granian
```

From a checkout:

```bash
python -m venv .venv
source .venv/bin/activate
pip install -U pip maturin
maturin develop
pip install granian
```

For tests and local development extras:

```bash
pip install "oxyroute[dev]"
```

For benchmark extras:

```bash
pip install "oxyroute[bench]"
```

See [installation.md](installation.md) for troubleshooting native builds.

## Application object

```python
from oxyroute import App

app = App(title="My API", include_openapi=True, docs_ui="scalar")
```

Constructor options:

| Option | Default | Meaning |
|---|---:|---|
| `title` | `"OxyRoute"` | Stored in the generated OpenAPI document. |
| `include_openapi` | `True` | Serve built-in `GET` / `HEAD /openapi.json`. |
| `docs_ui` | `None` | `"scalar"` or `"swagger"` → mount interactive `GET /docs`. |
| `openapi_description` / `openapi_contact` / `openapi_servers` | `None` | Enrich OpenAPI `info` / `servers`. |

Runtime methods:

| Method | Meaning |
|---|---|
| `app.freeze()` | Reject new route registrations and build a read-only routing snapshot. The app also auto-builds this snapshot on first request if you do not call `freeze()`. |
| `app.set_openapi_served(False)` | Stop serving built-in `/openapi.json`; the in-memory document still exists. |
| `app.openapi_json()` | Return the current OpenAPI JSON string even when serving is disabled. |
| `app.set_openapi_info(...)` | Set `info.description` / `contact` / `servers`. |
| `app.mount_docs(path, ui=...)` | Mount Scalar or Swagger UI at a custom path. |
| `app.set_middleware(fn_or_none)` | Enable or disable one optional pre-route callback. |
| `app.set_cors(config_or_none)` | Enable or disable CORS header merging. |
| `app.set_security_headers(config_or_none)` | Enable or disable browser security header merging. |

## Routing

OxyRoute uses `matchit`-style patterns:

```python
@app.get("/items/:id")
def item(id: int) -> dict:
    return {"id": id}
```

Supported HTTP decorator methods:

- `app.get`
- `app.post`
- `app.put`
- `app.patch`
- `app.delete`
- `app.options`

`HEAD` uses the `GET` router and strips the response body while preserving the
expected `Content-Length` where possible.

Unknown routes return:

- `404 Not Found` when no method matches the path.
- `405 Method Not Allowed` with an `Allow` header when another method matches
  the same path.

Unsupported HTTP methods raise a routing error in the native layer.

## Routers and prefixes

Use `APIRouter` to group routes and mount them under a prefix:

```python
from oxyroute import APIRouter, App

api = APIRouter()


@api.get("/users/:id")
def user(id: int) -> dict:
    return {"id": id}


app = App()
app.include_router(api, prefix="/api/v1")
```

`include_router(..., **defaults)` can pass route defaults such as JWT settings or
dependencies. Per-route options win over defaults.

## Handler parameters

Handlers are called with keyword arguments. OxyRoute only passes values that the
handler accepts by name, unless the handler has `**kwargs`.

```python
@app.get("/search/:kind")
def search(kind: str, query: dict[str, str]) -> dict:
    return {"kind": kind, "query": query}
```

Common injected names:

| Name | When present | Value |
|---|---|---|
| Path params | Route pattern captures them | Coerced to common Python types when possible (`int`, `float`, `bool`, `str`). |
| `query` | Query string exists | `dict[str, str]`, percent-decoded, duplicate keys are last-wins. |
| `json` | `read_json_body=True` and JSON body parses | Python object converted from JSON. |
| `form` | `read_form_body=True` | Form fields as `dict[str, str]`. |
| `files` | Multipart form with file parts | List of dictionaries with `name`, optional `filename`, `content_type`, and `data` bytes. |
| `body` | Handler asks for raw body and JSON/form modes do not consume it | Raw `bytes`. |
| `protocol` | Handler declares `protocol` | Underlying RSGI protocol object for advanced flows such as SSE. |
| `claims` | Route uses `require_jwt=True` and token verifies | Decoded JWT claims object. |
| Dependency names | Route declares `dependencies=[(...)]` | Return values of dependency factories. |

## Request bodies

For `POST`, `PUT`, and `PATCH`, JSON body parsing is enabled by default:

```python
@app.post("/items")
def create_item(json: dict) -> dict:
    return {"created": json}
```

For form bodies:

```python
@app.post("/submit", read_form_body=True)
def submit(form: dict[str, str]) -> dict:
    return {"form": form}
```

For multipart:

```python
@app.post("/upload", read_form_body=True)
def upload(form: dict[str, str], files: list[dict]) -> dict:
    return {"fields": form, "file_count": len(files)}
```

Important production note: request bodies are currently buffered in memory before
parsing. The default limit is **8 MiB**, controlled by `OXYROUTE_MAX_BODY_BYTES`.
Set a stricter limit at the deployment boundary (reverse proxy / server) for
untrusted public traffic.

## Return values

Simple returns:

```python
@app.get("/text")
def text() -> str:
    return "hello"


@app.get("/json")
def json_response() -> dict:
    return {"ok": True}


@app.get("/bytes")
def bytes_response() -> bytes:
    return b"raw"
```

Mapping rules:

| Return value | Response |
|---|---|
| `str` | `200 text/plain; charset=utf-8` |
| `bytes` | `200 application/octet-stream` |
| `dict`, `list`, other JSON-serializable object | `200 application/json; charset=utf-8` |
| `Response` | Custom status/body/headers/cookies |
| Dict with `status`, `body`, optional `headers` / `cookies` | Structured response path |
| `None` | Empty response where supported by the response mapper |

Use `Response` when you need explicit status, content type, or cookies:

```python
from oxyroute import Response


@app.get("/created")
def created() -> Response:
    return Response(
        status=201,
        body={"ok": True},
        headers={"x-example": "yes"},
        cookies=["session=abc; HttpOnly; SameSite=Lax"],
    )
```

## Error responses

Raise `HTTPException` for expected HTTP errors:

```python
from oxyroute import HTTPException


@app.get("/items/:id")
def item(id: int) -> dict:
    if id < 0:
        raise HTTPException(400, "id must be positive")
    return {"id": id}
```

Other uncaught exceptions become `500` with a generic JSON body by default:

```json
{"error":"internal server error"}
```

Set `OXYROUTE_DEBUG=1` only in safe development environments if you need the
error detail in responses.

## Middleware and optional layers

OxyRoute does **not** enable middleware, CORS, or security headers by default.
All of these are opt-in and can be disabled by passing `None`.

### Pre-route middleware

There is one optional pre-route callback:

```python
def block_health_probe(scope, protocol):
    if getattr(scope, "path", "") == "/blocked":
        return {"status": 403, "body": {"error": "blocked"}}
    return None


app.set_middleware(block_health_probe)
```

Behavior:

- Runs before route matching and before body reading.
- Return `None` to continue.
- Return any mapped response value to send that response immediately.
- `app.set_middleware(None)` disables it.

Because middleware must run, enabling it disables some synchronous short-circuit
optimizations for built-in OpenAPI / 404 / 405 paths.

### CORS

Use `apply_cors` for normal browser API usage:

```python
from oxyroute import CORSConfig, apply_cors

apply_cors(
    app,
    CORSConfig(
        allow_origins=["https://frontend.example"],
        allow_methods=["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
        allow_headers=["authorization", "content-type"],
        allow_credentials=True,
    ),
)
```

`apply_cors` configures response header merging and installs a preflight
middleware. If you only want response header merging, use `app.set_cors(config)`.
Disable with `app.set_cors(None)` and, if `apply_cors` installed middleware,
replace or clear middleware with `app.set_middleware(...)`.

### Security headers

```python
from oxyroute import SecurityHeadersConfig

app.set_security_headers(
    SecurityHeadersConfig(
        hsts="max-age=31536000; includeSubDomains",
        content_security_policy="default-src 'none'; frame-ancestors 'none'",
    )
)
```

Disable with:

```python
app.set_security_headers(None)
```

HSTS is emitted only when the request scheme is `https`.

### CSRF

CSRF is opt-in and intended for browser flows that rely on cookies for
authentication. Typical APIs that use only Authorization headers may not need it.

```python
from oxyroute import CSRFConfig, apply_csrf

apply_csrf(app, CSRFConfig(secret="change-me"))
```

See [csrf.md](csrf.md) for token flow and CORS composition.

## JWT-protected routes

```python
@app.get(
    "/private",
    require_jwt=True,
    jwt_secret="dev-secret",
    algorithms=["HS256"],
)
def private(claims: dict) -> dict:
    return {"sub": claims.get("sub")}
```

For asymmetric algorithms, `jwt_secret` is the public key PEM used for
verification:

```python
@app.get(
    "/admin",
    require_jwt=True,
    jwt_secret=PUBLIC_KEY_PEM,
    algorithms=["RS256"],
    jwt_issuer="https://issuer.example",
    jwt_audience="api",
)
def admin(claims: dict) -> dict:
    return claims
```

If `jwt_cookie="name"` is configured, OxyRoute can read a token from that cookie
when there is no usable `Authorization: Bearer ...` header.

## Dependencies

Dependencies are a linear, named list:

```python
from oxyroute import Depends


def settings() -> dict:
    return {"region": "eu"}


async def user(settings: dict) -> dict:
    return {"name": "alice", "region": settings["region"]}


@app.get("/me", dependencies=[("settings", Depends(settings)), ("user", Depends(user))])
def me(user: dict) -> dict:
    return user
```

Later dependency factories receive earlier dependency values by name. Route
handlers receive only dependency names they declare, unless they use `**kwargs`.

## WebSockets

Native WebSockets use the same RSGI app and Granian process:

```python
from oxyroute import WebSocket


@app.websocket("/ws/:room")
async def chat(ws: WebSocket) -> None:
    await ws.accept()
    await ws.send_text(f"room={ws.path_params['room']}")
    while True:
        msg = await ws.receive_text()
        if msg == "bye":
            await ws.close()
            return
        await ws.send_text(f"echo:{msg}")
```

Run:

```bash
granian app:app --interface rsgi
```

Unknown WebSocket paths are closed with code `1000`; handler errors close with
`1011`. Add application-level origin/auth checks as needed for your deployment.

## SSE and streaming-style responses

For Server-Sent Events, ask for the `protocol` parameter and use `send_sse`.
It accepts an iterable or async iterable of `str` / `SSEEvent` and returns the
marker object that tells OxyRoute the response has already been sent:

```python
from oxyroute import SSEEvent, send_sse


@app.get("/events")
async def events(protocol):
    return await send_sse(protocol, [SSEEvent(data="hello")])
```

See [streaming.md](streaming.md) for details and caveats.

## OpenAPI

`GET /openapi.json` and `HEAD /openapi.json` are served by default. Disable:

```python
app = App(include_openapi=False)
# or later:
app.set_openapi_served(False)
```

You can still export the document:

```python
spec_json = app.openapi_json()
```

Request body schemas can be documented with either `body_model=` (Pydantic v2)
or `body_schema=` on `post`, `put`, and `patch` routes.

## Lifespan and per-worker state

Subclass `App` and override **`on_startup` / `on_shutdown`** (Granian calls sync
`__rsgi_init__(loop)` with a non-running loop — do not use `async def __rsgi_init__`):

```python
from oxyroute import App


class MyApp(App):
    async def on_startup(self) -> None:
        self.state.ready = True

    async def on_shutdown(self) -> None:
        self.state.ready = False


app = MyApp()
```

`app.state` is a `types.SimpleNamespace`. It is per process, not shared between
Granian workers. See [rsgi.md](rsgi.md).

## Recommended production shape

Minimal production-facing shape:

```python
from oxyroute import App, CORSConfig, SecurityHeadersConfig, apply_cors

app = App(title="Production API", include_openapi=False)

apply_cors(
    app,
    CORSConfig(
        allow_origins=["https://frontend.example"],
        allow_methods=["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
        allow_headers=["authorization", "content-type"],
        allow_credentials=True,
    ),
)

app.set_security_headers(
    SecurityHeadersConfig(
        hsts="max-age=31536000; includeSubDomains",
        content_security_policy="default-src 'none'; frame-ancestors 'none'",
    )
)


@app.get("/health")
def health() -> str:
    return "ok"


app.freeze()
```

Run behind a reverse proxy or platform that handles TLS, request-size limits,
timeouts, logging, and process supervision:

```bash
granian app:app --interface rsgi --host 0.0.0.0 --port 8000 --workers 2
```

Production checklist:

- Set body-size limits at the edge as well as `OXYROUTE_MAX_BODY_BYTES`.
- Do not expose `/openapi.json` publicly unless intended.
- Use explicit JWT algorithms and issuer/audience checks for protected routes.
- Use `apply_cors` only for trusted origins; avoid `allow_origins=["*"]` with credentials.
- Add origin/auth checks for WebSockets when exposed to browsers.
- Keep `OXYROUTE_DEBUG` unset in production.
- Use external storage for cross-worker state.

## Known limitations in v0.5.0

- Request bodies and multipart files are buffered in memory before parsing.
- WebSocket subprotocol negotiation is not exposed as a high-level API.
- Benchmark scripts are for local comparison and are not CI performance gates.

## See also

- [Routing](routing.md)
- [Handlers](handlers.md)
- [Dependencies](dependencies.md)
- [CORS](cors.md)
- [CSRF](csrf.md)
- [JWT](jwt.md)
- [WebSockets](websocket.md)
- [RSGI and Granian](rsgi.md)
