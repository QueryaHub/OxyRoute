# OpenAPI and `/openapi.json`

[← Documentation index](index.md)

OxyRoute maintains a small **OpenAPI 3.0**-shaped JSON document in Rust while routes are registered. It is **not** a full OpenAPI model of every type and body schema; it is a **minimal** view suitable for discovery and tooling, and can be extended in future versions.

## Constructor and toggles

Served vs built: one flag, two ways to set it.

- **`include_openapi`** (constructor) and **`set_openapi_served(enabled)`** both update the same native field (`AppState::include_openapi`). Use the constructor to choose the default; use **`set_openapi_served(False)`** to turn off HTTP serving later (or `True` to re-enable) without recreating the app.
- **While serving is off** (`include_openapi=False` at build time, or after `set_openapi_served(False)`):
  - The engine does **not** return the spec for **`GET /openapi.json`** or **`HEAD /openapi.json`**. The request is **not** special-cased, so it goes through normal routing. Unless you add your own handler for that path, the client usually gets **404 Not Found**.
  - Route registration still **merges** operations into the in-memory OpenAPI document. The document is not discarded.
- **Export:** **`openapi_json()`** on the Python `App` still returns the current JSON as a string (handy in tests, admin tools, or a custom response), regardless of the serving toggle.

## Title and export

- **`set_openapi_title`** is applied from `App(..., title="...")` at construction.
- **`openapi_json()`** is described above; it is independent of whether `/openapi.json` is exposed over HTTP.

## What is in the document today

Per route, the code records path, method, a short `summary` / `operationId` derived from the **handler’s** `__name__`, and a simple `200` response placeholder. Bodies, parameters, and components are not yet deep-modeled on the document.

**Future work** might add richer request/response metadata when the schema story grows in Rust.

## See also

- [Routing](routing.md)
- [Handlers](handlers.md)
