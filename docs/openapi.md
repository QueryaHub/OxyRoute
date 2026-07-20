# OpenAPI and `/openapi.json`

[← Documentation index](index.md)

OxyRoute maintains an **OpenAPI 3.0**-shaped JSON document in Rust while routes are registered. It is suitable for discovery and interactive docs (Scalar / Swagger UI), and can be extended further in future versions.

## Constructor and toggles

Served vs built: one flag, two ways to set it.

- **`include_openapi`** (constructor) and **`set_openapi_served(enabled)`** both update the same native field (`AppState::include_openapi`). Use the constructor to choose the default; use **`set_openapi_served(False)`** to turn off HTTP serving later (or `True` to re-enable) without recreating the app.
- **Order does not change the in-memory spec:** routes are merged into the OpenAPI document as you register them, no matter when you flip the serving toggle. You can call **`set_openapi_served` before or after** adding routes; only HTTP exposure of `GET/HEAD /openapi.json` changes.
- **Re-enable:** an app created with **`include_openapi=False`** can still call **`set_openapi_served(True)`** to start serving the spec at `GET/HEAD /openapi.json` (same for turning back on after `set_openapi_served(False)`).
- **When serving is on** (`include_openapi=True` and not since disabled): the dispatcher answers **`GET /openapi.json`** and **`HEAD /openapi.json`** in an early check, **before** the normal router. A user-registered route on exactly `/openapi.json` will **not** run for those methods while the built-in is active.
- **While serving is off** (`include_openapi=False` at build time, or after `set_openapi_served(False)`):
  - The engine does **not** return the spec for **`GET /openapi.json`** or **`HEAD /openapi.json`**. The request is **not** special-cased, so it goes through normal routing. Unless you add your own handler for that path, the client usually gets **404 Not Found**. If you **do** register a handler for `/openapi.json`, that handler can serve a custom response.
  - Route registration still **merges** operations into the in-memory OpenAPI document. The document is not discarded.
- **Export:** **`openapi_json()`** on the Python `App` still returns the current JSON as a string (handy in tests, admin tools, or a custom response), regardless of the serving toggle. The exported document is the **same** enriched spec the UI uses.

## Docs UI (Scalar / Swagger)

Optional interactive explorer (CDN-backed HTML):

```python
from oxyroute import App

app = App(title="My API", docs_ui="scalar")  # or "swagger"
# → GET /docs

# or later / custom path:
app.mount_docs("/api/docs", ui="swagger")
```

| Option | Meaning |
|--------|---------|
| `docs_ui="scalar"` \| `"swagger"` | Mount `GET /docs` at construction |
| `mount_docs(path, ui=...)` | Mount at a custom path |
| Spec URL | Built-in `/openapi.json` |

UI scripts load from **jsDelivr**. If you use `SecurityHeadersConfig` (or a strict CSP), allow `cdn.jsdelivr.net` in `script-src` / `style-src` for the docs route, or disable those headers on `/docs`.

## Title, info, and servers

- **`title=`** / **`set_openapi_title`** — `info.title`.
- **`openapi_description=`**, **`openapi_contact=`**, **`openapi_servers=`** constructor kwargs, or **`app.set_openapi_info(description=..., contact=..., servers=...)`**.

```python
app = App(
    title="Market API",
    openapi_description="Public marketplace HTTP API",
    openapi_contact={"name": "API", "email": "api@example.com"},
    openapi_servers=[{"url": "https://api.example.com"}],
    docs_ui="scalar",
)
```

## What is in the document today

Per route, the code records:

- OpenAPI path templates: matchit **`:id` → `{id}`**, catch-all **`*rest` → `{rest}`**
- Path **`parameters`** (`in: path`, `required: true`, string schema)
- Method, short `summary` / `operationId` from the handler’s `__name__`
- Simple `200` response placeholder
- Optional **`tags`** from the route decorator or `include_router(..., tags=[...])` (per-route `tags=` wins over include defaults)
- If **`require_jwt=True`**: `components.securitySchemes.bearerAuth` (`http` + `bearer` + `JWT`) and `security: [{ bearerAuth: [] }]` on that operation

For **`POST`**, **`PUT`**, and **`PATCH`**, you can document the JSON request body in OpenAPI in two ways (pass **at most one**):

- **`body_model=...`**: a **Pydantic v2** `BaseModel` class. OxyRoute calls **`model_json_schema()`** at registration time and stores the result under **`requestBody` → `content` → `application/json` → `schema`**. Pydantic is optional at runtime: only needed if you use `body_model` (`$defs` / `$ref` appear as Pydantic emits them).
- **`body_schema=...`**: a plain **JSON Schema** object (`dict` / mapping), e.g. from hand-written spec or another library. It is **JSON-serialized** at registration time and used the same way in OpenAPI. No Pydantic import required.

## See also

- [Routing](routing.md)
- [Handlers](handlers.md)
- [RSGI / lifespan](rsgi.md)
