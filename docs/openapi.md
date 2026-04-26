# OpenAPI and `/openapi.json`

[← Documentation index](index.md)

OxyRoute maintains a small **OpenAPI 3.0**-shaped JSON document in Rust while routes are registered. It is **not** a full OpenAPI model of every type and body schema; it is a **minimal** view suitable for discovery and tooling, and can be extended in future versions.

## Constructor and toggles

Served vs built: one flag, two ways to set it.

- **`include_openapi`** (constructor) and **`set_openapi_served(enabled)`** both update the same native field (`AppState::include_openapi`). Use the constructor to choose the default; use **`set_openapi_served(False)`** to turn off HTTP serving later (or `True` to re-enable) without recreating the app.
- **Order does not change the in-memory spec:** routes are merged into the OpenAPI document as you register them, no matter when you flip the serving toggle. You can call **`set_openapi_served` before or after** adding routes; only HTTP exposure of `GET/HEAD /openapi.json` changes.
- **Re-enable:** an app created with **`include_openapi=False`** can still call **`set_openapi_served(True)`** to start serving the spec at `GET/HEAD /openapi.json` (same for turning back on after `set_openapi_served(False)`).
- **When serving is on** (`include_openapi=True` and not since disabled): the dispatcher answers **`GET /openapi.json`** and **`HEAD /openapi.json`** in an early check, **before** the normal router. A user-registered route on exactly `/openapi.json` will **not** run for those methods while the built-in is active.
- **While serving is off** (`include_openapi=False` at build time, or after `set_openapi_served(False)`):
  - The engine does **not** return the spec for **`GET /openapi.json`** or **`HEAD /openapi.json`**. The request is **not** special-cased, so it goes through normal routing. Unless you add your own handler for that path, the client usually gets **404 Not Found**. If you **do** register a handler for `/openapi.json`, that handler can serve a custom response.
  - Route registration still **merges** operations into the in-memory OpenAPI document. The document is not discarded.
- **Export:** **`openapi_json()`** on the Python `App` still returns the current JSON as a string (handy in tests, admin tools, or a custom response), regardless of the serving toggle.

## Title and export

- **`set_openapi_title`** is applied from `App(..., title="...")` at construction.
- **`openapi_json()`** is described above; it is independent of whether `/openapi.json` is exposed over HTTP.

## What is in the document today

Per route, the code records path, method, a short `summary` / `operationId` derived from the **handler’s** `__name__`, and a simple `200` response placeholder.

For **`POST`**, **`PUT`**, and **`PATCH`**, you can pass **`body_model=...`** where the value is a **Pydantic v2** `BaseModel` class. OxyRoute calls **`model_json_schema()`** at registration time and stores that JSON Schema under the operation’s **`requestBody` → `content` → `application/json` → `schema`**. Pydantic is optional at runtime: only needed if you use `body_model`. The schema is a plain JSON value in the OpenAPI file (`$defs` and refs may appear as Pydantic emits them).

## See also

- [Routing](routing.md)
- [Handlers](handlers.md)
