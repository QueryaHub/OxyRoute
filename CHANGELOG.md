# Changelog

All notable changes to OxyRoute are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-10-18

### Performance

- **Zero-Tokio WebSocket Transport**: Direct Python coroutine execution for WebSocket `receive`/`send`/`accept` eliminating Tokio task hops and future bridge overhead per frame ([#257](https://github.com/QueryaHub/OxyRoute/issues/257), [#264](https://github.com/QueryaHub/OxyRoute/pull/264)).
- **Lock-Free Concurrency Checks**: Move `in_flight` and `max_concurrency` atomics directly to `App` and snapshot `AppState` once per request, eliminating reader lock contention ([#256](https://github.com/QueryaHub/OxyRoute/issues/256), [#263](https://github.com/QueryaHub/OxyRoute/pull/263)).
- **Direct Pydantic Validation & Zero-AST JSON**: Direct `model_validate_json` execution and direct streaming of JSON bytes into Python dicts bypassing intermediate AST copies ([#255](https://github.com/QueryaHub/OxyRoute/issues/255), [#262](https://github.com/QueryaHub/OxyRoute/pull/262)).
- **Sync Short-Circuit for Path Parameters**: Extended zero-Tokio sync short-circuit path to handle sync routes with path parameters ([#254](https://github.com/QueryaHub/OxyRoute/issues/254), [#261](https://github.com/QueryaHub/OxyRoute/pull/261)).
- **Zero-Allocation Routing Attributes**: Extract string slices `&str` instead of heap-allocated `String` during preliminary dispatch ([#253](https://github.com/QueryaHub/OxyRoute/issues/253), [#260](https://github.com/QueryaHub/OxyRoute/pull/260)).
- **Zero-Allocation Rate Limiting**: Zero-alloc key lookup in `RateLimiter::check` reusing byte buffers ([#252](https://github.com/QueryaHub/OxyRoute/issues/252), [#259](https://github.com/QueryaHub/OxyRoute/pull/259)).
- **Cached Protocol Calls**: Replaced runtime `getattr` calls on protocol objects with direct method calls ([#251](https://github.com/QueryaHub/OxyRoute/issues/251), [#258](https://github.com/QueryaHub/OxyRoute/pull/258)).
- **Native Compiled CORS & Security Headers**: Compiled CORS and security header policies executed directly in Rust without Python interop on short-circuited paths ([#218](https://github.com/QueryaHub/OxyRoute/issues/218), [#250](https://github.com/QueryaHub/OxyRoute/pull/250)).
- **Zero-Copy Request Body**: Zero-copy request body handling via `PyBackedBytes` avoiding double memory copying ([#219](https://github.com/QueryaHub/OxyRoute/issues/219), [#249](https://github.com/QueryaHub/OxyRoute/pull/249)).
- **Optimized Header Scanning**: Single-pass case-insensitive ASCII scan in `header_get_lax` with short-circuit lookup ([#220](https://github.com/QueryaHub/OxyRoute/issues/220), [#248](https://github.com/QueryaHub/OxyRoute/pull/248)).
- **Native Fast JSON Serialization**: Fast-path native JSON serializer for dict and list responses ([#217](https://github.com/QueryaHub/OxyRoute/issues/217), [#247](https://github.com/QueryaHub/OxyRoute/pull/247)).

### Security & Hardening

- **CSWSH Protection**: Validate `Origin` header during WebSocket handshake with `allowed_origins` parameter ([#225](https://github.com/QueryaHub/OxyRoute/issues/225), [#239](https://github.com/QueryaHub/OxyRoute/pull/239)).
- **Content-Length Guard**: Reject oversized request bodies using `Content-Length` before allocating buffer memory ([#206](https://github.com/QueryaHub/OxyRoute/issues/206), [#228](https://github.com/QueryaHub/OxyRoute/pull/228)).
- **Rate Limiting Security**: Disallow trusting `X-Forwarded-For` / `X-Real-IP` by default unless explicitly configured ([#207](https://github.com/QueryaHub/OxyRoute/issues/207), [#229](https://github.com/QueryaHub/OxyRoute/pull/229)).
- **Bounded Rate Limiter Memory**: Bound shard memory with LRU-style eviction preventing memory exhaustion from arbitrary IP/key floods ([#208](https://github.com/QueryaHub/OxyRoute/issues/208), [#230](https://github.com/QueryaHub/OxyRoute/pull/230)).
- **SSE Field Sanitization**: Prevent CRLF injection by rejecting newlines in SSE `event` and `id` fields ([#221](https://github.com/QueryaHub/OxyRoute/issues/221), [#243](https://github.com/QueryaHub/OxyRoute/pull/243)).
- **JSON & Multipart Error Safety**: Build error bodies safely with `serde_json::json!` avoiding format string injection ([#222](https://github.com/QueryaHub/OxyRoute/issues/222), [#242](https://github.com/QueryaHub/OxyRoute/pull/242)).
- **Case-Insensitive Bearer Auth**: Match `Bearer` scheme case-insensitively per RFC 6750 ([#224](https://github.com/QueryaHub/OxyRoute/issues/224), [#240](https://github.com/QueryaHub/OxyRoute/pull/240)).
- **Robust Cookie Parsing**: Ignore malformed cookie segments instead of aborting search ([#223](https://github.com/QueryaHub/OxyRoute/issues/223), [#241](https://github.com/QueryaHub/OxyRoute/pull/241)).
- **Docs UI Security**: Pin CDN asset versions and add SRI integrity hashes for Scalar / Swagger UI ([#226](https://github.com/QueryaHub/OxyRoute/issues/226), [#238](https://github.com/QueryaHub/OxyRoute/pull/238)).

### Bug Fixes

- **Deadlock Prevention**: Acquire Python GIL before taking `AppState` write lock to eliminate AB-BA deadlocks with background worker threads ([#205](https://github.com/QueryaHub/OxyRoute/issues/205), [#227](https://github.com/QueryaHub/OxyRoute/pull/227)).
- **Middleware Stacking**: `apply_cors()` and `apply_csrf()` now compose properly onto existing middleware chains rather than overwriting them ([#209](https://github.com/QueryaHub/OxyRoute/issues/209), [#231](https://github.com/QueryaHub/OxyRoute/pull/231)).
- **Wildcard Path Parameters**: Wildcard `*path` parameters preserve string type and bypass numeric/float schema-lite coercion ([#212](https://github.com/QueryaHub/OxyRoute/issues/212), [#234](https://github.com/QueryaHub/OxyRoute/pull/234)).
- **Handler Return Types**: Correctly handle dict/list returns and prevent status code misinterpretation ([#211](https://github.com/QueryaHub/OxyRoute/issues/211), [#233](https://github.com/QueryaHub/OxyRoute/pull/233)).
- **Database Query Decoding**: Robust decoding for Postgres UUID, Timestamp, and Decimal types without panicking ([#213](https://github.com/QueryaHub/OxyRoute/issues/213), [#237](https://github.com/QueryaHub/OxyRoute/pull/237)).
- **Protocol Wrapper Proxy**: Forward all RSGI protocol methods (`__call__`, async iteration, WebSockets) through `_ProtocolWrapper` ([#216](https://github.com/QueryaHub/OxyRoute/issues/216), [#244](https://github.com/QueryaHub/OxyRoute/pull/244)).
- **Adaptive Concurrency Limiter**: Fix unit mismatches and prevent self-throttling under normal operation ([#215](https://github.com/QueryaHub/OxyRoute/issues/215), [#245](https://github.com/QueryaHub/OxyRoute/pull/245)).
- **Cleanup**: Remove deprecated `/test_db` debug prototype endpoint ([#210](https://github.com/QueryaHub/OxyRoute/issues/210), [#232](https://github.com/QueryaHub/OxyRoute/pull/232)).

## [0.6.0] - 2026-09-12

### Added

- **Native Rate Limiting**: In-memory sharded Token Bucket rate limiter in Rust on route decorators
  (`@app.get(..., rate_limit="100/minute")`) and `APIRouter`. Supports key strategies for client IP
  (with `X-Forwarded-For` / `X-Real-IP` support), request headers (`header:<name>`), and global limits,
  returning RFC-compliant `Retry-After` and `X-RateLimit-*` headers on HTTP 429 ([#162](https://github.com/QueryaHub/OxyRoute/issues/162)).
- **DAG Dependency Resolution**: Directed Acyclic Graph dependency resolution with topological sorting
  and cycle detection at route registration time ([#141](https://github.com/QueryaHub/OxyRoute/issues/141), [#189](https://github.com/QueryaHub/OxyRoute/pull/189)).
- **Adaptive Concurrency Limiting & Load Shedding**: `AdaptiveConcurrencyLimiter` to protect against
  out-of-memory errors and tail latency spikes under sudden load ([#161](https://github.com/QueryaHub/OxyRoute/issues/161), [#185](https://github.com/QueryaHub/OxyRoute/pull/185)).
- **OpenAPI 3.1.0 & Schema Enhancements**: Upgraded default OpenAPI specification version to 3.1.0 for
  Pydantic v2 JSON Schema compatibility ([#154](https://github.com/QueryaHub/OxyRoute/issues/154)),
  auto-documented 401 Unauthorized and 422 Validation Error responses ([#153](https://github.com/QueryaHub/OxyRoute/issues/153)),
  and added query / header parameter schema declarations ([#155](https://github.com/QueryaHub/OxyRoute/issues/155)).
- **Pydantic DX**: Auto-infer Pydantic `body_model` from handler signature annotations and support
  flexible parameter names ([#151](https://github.com/QueryaHub/OxyRoute/issues/151), [#152](https://github.com/QueryaHub/OxyRoute/issues/152)).
- **WebSocket Enhancements**: Concurrent send frame serialization to prevent frame interleaving ([#146](https://github.com/QueryaHub/OxyRoute/issues/146))
  and graceful shutdown broadcast sending WebSocket 1001 (Going Away) to active connections ([#158](https://github.com/QueryaHub/OxyRoute/issues/158)).
- **Type Stubs & Packaging**: Added PEP 561 `py.typed` marker and comprehensive `_oxyroute.pyi` type stubs ([#150](https://github.com/QueryaHub/OxyRoute/issues/150)).
- **Database Streaming**: Streaming and chunked row decoding for `DBQuery` SQLx results ([#143](https://github.com/QueryaHub/OxyRoute/issues/143)).

### Performance

- **PEP 590 Vectorcall Protocol**: Direct Python callable invocation using Vectorcall, eliminating `PyDict`
  allocation for kwargs on the hot path ([#159](https://github.com/QueryaHub/OxyRoute/issues/159)).
- **Packed 64B RouteEntry**: Packed route entry memory layout into a single 64-byte cacheline for optimal
  L1 cacheline utilization during route dispatch ([#147](https://github.com/QueryaHub/OxyRoute/issues/147)).
- **Unified Dependency & Snapshot Layout**: Contiguous `Arc<[DependencyEntry]>` arrays and consolidated
  `Arc<FrozenState>` snapshot reducing atomic reference counts ([#148](https://github.com/QueryaHub/OxyRoute/issues/148), [#149](https://github.com/QueryaHub/OxyRoute/issues/149)).
- **Thread-Local Buffer Pool**: Implemented `PooledBuffer` to reuse request body memory allocations ([#160](https://github.com/QueryaHub/OxyRoute/issues/160)).
- **Zero-Allocation Path Parameters**: Slicing path parameters directly from URL strings without intermediate
  heap allocations ([#140](https://github.com/QueryaHub/OxyRoute/issues/140)).
- **Single-Pass Method Lookup**: Unified radix lookup and method bitmasks for 405 Method Not Allowed handling ([#139](https://github.com/QueryaHub/OxyRoute/issues/139)).
- **Pre-Baked Static Response Headers**: Pre-formatted static response headers eliminating string formatting ([#164](https://github.com/QueryaHub/OxyRoute/issues/164), [#184](https://github.com/QueryaHub/OxyRoute/pull/184)).

### Security & Hardening

- **StaticFiles Hardening**: Prevented path traversal and symlink escape vulnerabilities ([#144](https://github.com/QueryaHub/OxyRoute/issues/144)).
- **DBQuery Parameter Binding**: Verified and documented SQL parameter binding and injection guarantees ([#145](https://github.com/QueryaHub/OxyRoute/issues/145), [#186](https://github.com/QueryaHub/OxyRoute/pull/186)).
- **FFI Safety**: Configured `panic = "abort"` in `Cargo.toml` to prevent undefined behavior from unwinding across FFI ([#156](https://github.com/QueryaHub/OxyRoute/issues/156)).
- **Observability**: Ensured `access_log_hook` execution on unhandled exceptions via try-finally ([#157](https://github.com/QueryaHub/OxyRoute/issues/157)).
- **Dependencies**: Upgraded to `oxyjwt >= 0.7.0` and dropped legacy `pyjwt`/`cryptography` dependencies ([#178](https://github.com/QueryaHub/OxyRoute/pull/178)).

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
