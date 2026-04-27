# OxyRoute

High-performance web framework for **Granian RSGI**, tuned for high **single-worker** throughput: routing, JSON/form parsing, JWT checks, response mapping, and native WebSockets run on a **Rust** hot path ([PyO3](https://pyo3.rs/) + [Maturin](https://www.maturin.rs/)), while business logic stays in plain **Python** handlers.

[![CI](https://github.com/QueryaHub/OxyRoute/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/QueryaHub/OxyRoute/actions)

## Features

- **RSGI** entrypoint (`async def __rsgi__(scope, protocol)`) compatible with Granian’s RSGI implementation
- **Routing** via [matchit](https://crates.io/crates/matchit) (path parameters like `/users/:id`)
- **JSON, form, and multipart bodies** parsed on the native path; successful values passed to handlers as kwargs
- **JWT** verification on the Rust path before your handler runs (`require_jwt`, HS*, RSA, EC, EdDSA public-key verification)
- **Optional** `GET /openapi.json` with a minimal OpenAPI-style document
- **Dependencies**: linear list of named factories (`Depends`, sync or async) passed as kwargs
- **Optional middleware layers** for pre-route decisions, CORS, CSRF, and browser security headers
- **Native RSGI WebSockets** via `@app.websocket(path)` and `oxyroute.WebSocket`
- Native extension wheel (abi3) for **Python ≥ 3.10**

Start with the full **[Usage guide](docs/usage.md)**, or use **[docs/index.md](docs/index.md)** for topic-specific pages.

## Requirements

- **Python** 3.10 or newer
- For **running** a pre-built wheel: only `pip` (and a server such as Granian)
- For **building from source**: Rust toolchain + [maturin](https://www.maturin.rs/) (and `patchelf` on some Linux setups is recommended for best wheel layout; see [docs/installation.md](docs/installation.md))

## Install

From PyPI (when published):

```bash
pip install oxyroute
```

Development / optional test dependencies:

```bash
pip install "oxyroute[dev]"
```

From a git checkout (builds the native module):

```bash
pip install maturin
maturin develop
# or: pip install .
```

## Quick start (RSGI + Granian)

`examples/rsgi_app.py`:

```python
from oxyroute import App

app = App(title="Hello OxyRoute")


@app.get("/")
def root() -> str:
    return "OxyRoute RSGI OK"


@app.get("/hello/:name")
def hello_name(name: str) -> dict:
    return {"message": f"Hello, {name}"}
```

Run (from the repo, after `maturin develop` or an editable install):

```bash
granian --interface rsgi examples.rsgi_app:app
```

Per-worker setup (`__rsgi_init__`) is shown in [examples/rsgi_lifespan_app.py](examples/rsgi_lifespan_app.py) and [docs/rsgi.md](docs/rsgi.md#lifespan-optional).

OxyRoute v0.3.0 supports **only** Granian RSGI; the legacy ASGI bridge (`uvicorn` / `granian --interface asgi`) was removed.

## Usage docs

- [Usage guide](docs/usage.md) — install, run, routing, bodies, responses, middleware, CORS, CSRF, JWT, WebSockets, deployment notes, limitations
- [RSGI and Granian](docs/rsgi.md) — app entrypoint, lifespan hooks, worker process model
- [Handlers](docs/handlers.md) — injected parameters and response mapping details
- [Routing](docs/routing.md) — methods, path syntax, `APIRouter`, `freeze()`
- [JWT](docs/jwt.md), [CORS](docs/cors.md), [CSRF](docs/csrf.md), [Security headers](docs/security-headers.md)
- [WebSockets](docs/websocket.md) and [SSE](docs/sse.md)

## Production notes

OxyRoute is designed for Granian RSGI deployments. For public traffic, place it behind a normal production boundary (TLS, request-size limits, timeouts, logging, process supervision) and keep `OXYROUTE_DEBUG` disabled. Request bodies and multipart files are currently buffered in memory before parsing, so enforce limits both at the edge and with `OXYROUTE_MAX_BODY_BYTES`.

## Project layout

- `oxyroute/` — Python package (`App`, `Depends`)
- `src/` — Rust extension (`_oxyroute`, routing, dispatch, JWT helpers)
- `docs/` — detailed English documentation
- `tests/` — pytest suite (run from a temp directory or an installed wheel so the source tree does not shadow the package; see [docs/development.md](docs/development.md))

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) (build, tests, issue backlog, batch `gh` script).

## License

This project is licensed under the [MIT License](LICENSE).

## Links

- [Granian RSGI specification](https://github.com/emmett-framework/granian/blob/master/docs/spec/RSGI.md)
- [Documentation index](docs/index.md)
