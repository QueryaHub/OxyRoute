# OxyRoute

**RSGI-first** web toolkit: HTTP routing, JSON handling, and HS\* JWT validation on a **Rust** hot path ([PyO3](https://pyo3.rs/) + [Maturin](https://www.maturin.rs/)), with your business logic in ordinary **Python** handlers. Pair it with **[Granian](https://github.com/emmett-framework/granian)** using `--interface rsgi` for the intended stack.

[![CI](https://github.com/QueryaHub/OxyRoute/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/QueryaHub/OxyRoute/actions)

## Features

- **RSGI** entrypoint (`async def __rsgi__(scope, protocol)`) compatible with Granian’s RSGI implementation
- **Routing** via [matchit](https://crates.io/crates/matchit) (path parameters like `/users/:id`)
- **JSON bodies** parsed in Rust; successful values passed to handlers as kwargs
- **JWT (HMAC)** verification on the Rust path before your handler runs (`require_jwt`, HS256/384/512)
- **Optional** `GET /openapi.json` with a minimal OpenAPI-style document
- **Dependencies**: linear list of named factories (`Depends`, sync or async) passed as kwargs
- **Optional ASGI 3** bridge: `async def __call__(scope, receive, send)` for servers that speak ASGI (see [docs/asgi.md](docs/asgi.md))
- Native extension wheel (abi3) for **Python ≥ 3.10**

Full documentation: **[docs/index.md](docs/index.md)**

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
def hello_name(**kwargs) -> str:
    return f"Hello, {kwargs.get('name', '')}"
```

Run (from the repo, after `maturin develop` or an editable install):

```bash
granian --interface rsgi examples.rsgi_app:app
```

ASGI and other servers are covered in [docs/asgi.md](docs/asgi.md).

## Project layout

- `oxyroute/` — Python package (`App`, `Depends`, optional ASGI bridge)
- `src/` — Rust extension (`_oxyroute`, routing, dispatch, JWT helpers)
- `docs/` — detailed English documentation
- `tests/` — pytest suite (run from a temp directory or an installed wheel so the source tree does not shadow the package; see [docs/development.md](docs/development.md))

## License

This project is licensed under the [MIT License](LICENSE).

## Links

- [Granian RSGI specification](https://github.com/emmett-framework/granian/blob/master/docs/spec/RSGI.md)
- [Documentation index](docs/index.md)
