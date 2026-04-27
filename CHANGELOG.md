# Changelog

All notable changes to OxyRoute are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
