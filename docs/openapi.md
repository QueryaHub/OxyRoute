# OpenAPI and `/openapi.json`

[← Documentation index](index.md)

OxyRoute maintains a small **OpenAPI 3.0**-shaped JSON document in Rust while routes are registered. It is **not** a full OpenAPI model of every type and body schema; it is a **minimal** view suitable for discovery and tooling, and can be extended in future versions.

## Constructor and toggles

- **`App(..., include_openapi=True)`** (Python) maps to the native `App` constructor. When `include_openapi` is false, the special **`GET /openapi.json`** response from the engine is not served, and the document may be empty in those modes depending on the implementation.
- **`set_openapi_served(False)`** can disable serving the document at runtime by flipping the same flag in state (naming mirrors the native `set_openapi_served`).

## Title and export

- **`set_openapi_title`** is applied from `App(..., title="...")` at construction.
- **`openapi_json()`** on the Python `App` returns the current document as a **string** (for debugging or an alternate transport).

## What is in the document today

Per route, the code records path, method, a short `summary` / `operationId` derived from the **handler’s** `__name__`, and a simple `200` response placeholder. Bodies, parameters, and components are not yet deep-modeled on the document.

**Future work** might add richer request/response metadata when the schema story grows in Rust.

## See also

- [Routing](routing.md)
- [Handlers](handlers.md)
