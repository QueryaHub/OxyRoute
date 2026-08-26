# Changelog

All notable changes to OxyRoute are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-07-20

### Added

- First-class OpenAPI docs UI: `App(..., docs_ui="scalar"|"swagger")` and
  `app.mount_docs(...)` serve CDN-backed Scalar / Swagger UI at `/docs` against
  `/openapi.json` ([#130](https://github.com/QueryaHub/OxyRoute/issues/130)).
- OpenAPI enrichment for interactive explorers: matchit `:param` / `*rest` → `{param}` /
  `{rest}` with path `parameters`; JWT `bearerAuth` when `require_jwt=True`; operation
  `tags=` / `include_router(..., tags=[...])`; `set_openapi_info` and constructor
  `openapi_description` / `openapi_contact` / `openapi_servers`.
- Granian-compatible lifespan: sync `__rsgi_init__(loop)` / `__rsgi_del__(loop)` run
  `on_startup` / `on_shutdown` via `loop.run_until_complete`. Prefer overriding
  `on_startup` / `on_shutdown` instead of `async def __rsgi_init__`.
- `oxyroute.testing.TestClient` for in-process HTTP tests ([#102](https://github.com/QueryaHub/OxyRoute/issues/102)).
- Typed `Request` with lazy headers ([#101](https://github.com/QueryaHub/OxyRoute/issues/101)).
- Runtime `body_model` validation with HTTP 422 ([#100](https://github.com/QueryaHub/OxyRoute/issues/100)).
- Global exception handlers for sync and async routes
  ([#99](https://github.com/QueryaHub/OxyRoute/issues/99)).
- Optional request / response middleware chain
  ([#98](https://github.com/QueryaHub/OxyRoute/issues/98)).
- `StaticFiles` and `App.mount` ([#104](https://github.com/QueryaHub/OxyRoute/issues/104)).
- Generic streaming responses (non-SSE chunked generators)
  ([#103](https://github.com/QueryaHub/OxyRoute/issues/103)).
- Observability hooks: request id, access log, metrics
  ([#127](https://github.com/QueryaHub/OxyRoute/pull/127)).
- SQLx / Postgres pool helpers on `AppState` and dynamic query execution from Python
  dependencies ([#116](https://github.com/QueryaHub/OxyRoute/issues/116),
  [#118](https://github.com/QueryaHub/OxyRoute/issues/118)).
- Criterion microbenchmarks (`cargo bench --bench hot_path`) and expanded wrk scenarios
  (`perf-test/bench_scenarios.sh`); optional `perf-smoke` workflow
  ([#110](https://github.com/QueryaHub/OxyRoute/issues/110)).

### Changed

- OpenAPI path keys use `{param}` form (breaking for consumers that asserted matchit
  `:param` strings in `openapi_json()`).
- JWT hot path reuses prebuilt `DecodingKey` and `Validation` per route
  ([#109](https://github.com/QueryaHub/OxyRoute/issues/109)).
- CORS response merge skips the Python `response_header_pairs` call when the request has
  no `Origin` header ([#108](https://github.com/QueryaHub/OxyRoute/issues/108)).
- OpenAPI document string is cached until the next registration change
  ([#129](https://github.com/QueryaHub/OxyRoute/pull/129)).
- Router / dispatch hot-path improvements: fewer path-param allocations, sync short-circuit
  for trivial routes, direct `json_to_py`, cheaper str/bytes responses, env-flag caching
  ([#94](https://github.com/QueryaHub/OxyRoute/issues/94)–[#97](https://github.com/QueryaHub/OxyRoute/issues/97),
  [#128](https://github.com/QueryaHub/OxyRoute/pull/128)).

### Migration

- Prefer `async def on_startup` / `on_shutdown` for worker lifecycle under Granian. Do not
  override `__rsgi_init__` as `async def` (the coroutine is never awaited).
- Update any tooling that expected OpenAPI paths with matchit `:param` syntax to `{param}`.
- Interactive docs: `App(..., docs_ui="scalar")` (or `mount_docs`) instead of app-local HTML.

## [0.4.0] - 2026-05

RSGI-only line with native WebSockets, forms, CORS/CSRF/security headers, and related
hardening after the v0.3.0 ASGI removal. See `git log v0.3.0..v0.4.0` for the full list.

## [0.3.0] - 2026-04-27

### Added

- Native RSGI WebSocket support: `@app.websocket(path)` and `oxyroute.WebSocket` are
  exported from the package and dispatched directly inside the Rust extension. No ASGI
  shim is involved; the helper class wraps Granian's `RSGIWebsocketProtocol` and exposes
  `accept`, `receive` / `receive_text` / `receive_bytes`, `send_text` / `send_bytes` /
  `send_json`, and `close`. Path matching uses the same `matchit` syntax as HTTP routes
  and lives in its own router. Unknown paths produce a polite `close(1000)`; handler
  errors trigger `close(1011)`.

### Removed (breaking)

- The optional ASGI 3.0 compatibility bridge (`oxyroute.asgi`) was removed. `App` is no
  longer an ASGI callable; `App.__call__`, `App._asgi3`, the `asgi_to_rsgi` helper and the
  `WebSocket` helper class are gone. Run OxyRoute exclusively under
  ``granian --interface rsgi``.
- The ASGI-based `@app.websocket(path)` decorator and the
  `App._handle_asgi_websocket` plumbing were removed alongside the bridge. Native RSGI
  WebSocket support is reintroduced in this release (see *Added*).

### Migration

- If you ran `uvicorn` or `granian --interface asgi` against an OxyRoute app, switch to
  `granian --interface rsgi`. RSGI is now the only supported transport.
- For unit tests that drove the app via `httpx.ASGITransport(app=app)`, import the
  test-only shim from the test tree:

  ```python
  from tests._rsgi_test_transport import asgi_test_app

  transport = httpx.ASGITransport(app=asgi_test_app(app))
  ```

  The shim is **only** built for the test suite; production code must not import it.

## [0.2.0] - 2026-04

Initial public release. See `git log v0.2.0` for details.
