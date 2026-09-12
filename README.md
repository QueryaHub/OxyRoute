# OxyRoute

<p align="center">
  <strong>High-performance, production-grade RSGI web framework for Python powered by a native Rust hot path.</strong>
</p>

<p align="center">
  <a href="https://github.com/QueryaHub/OxyRoute/actions"><img src="https://github.com/QueryaHub/OxyRoute/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI Status"></a>
  <a href="https://pypi.org/project/oxyroute/"><img src="https://img.shields.io/pypi/v/oxyroute.svg" alt="PyPI version"></a>
  <a href="https://www.python.org/downloads/"><img src="https://img.shields.io/badge/python-3.10+-3776AB.svg?logo=python&logoColor=white" alt="Python 3.10+"></a>
  <a href="https://pyo3.rs/"><img src="https://img.shields.io/badge/rust-PyO3%200.25-DEA584.svg?logo=rust&logoColor=white" alt="Rust PyO3"></a>
  <a href="https://github.com/emmett-framework/granian"><img src="https://img.shields.io/badge/server-Granian%20RSGI-5822B4.svg" alt="Granian RSGI"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-green.svg" alt="License: MIT"></a>
</p>

---

## Overview

**OxyRoute** combines the expressiveness and developer experience of Python with the raw speed, safety, and memory efficiency of Rust. Designed from the ground up for **[Granian RSGI](https://github.com/emmett-framework/granian)**, OxyRoute executes request routing, body parsing, token-bucket rate limiting, JWT authentication, and response encoding directly in compiled **Rust ([PyO3](https://pyo3.rs/))**, executing Python code only for your business logic handlers.

```
Incoming Request
      │
      ▼
┌──────────────────────────────────────────────────────────┐
│  Granian (RSGI Protocol)                                 │
└───────────────┬──────────────────────────────────────────┘
                │ Zero-copy scope & protocol pass
                ▼
┌──────────────────────────────────────────────────────────┐
│  OxyRoute Rust Core (_oxyroute)                          │
│  ├─ Radix Routing (matchit, zero-alloc path slices)      │
│  ├─ Sharded Token-Bucket Rate Limiter (HTTP 429)         │
│  ├─ JWT Verification (RS256/ES256/EdDSA/HS* via oxyjwt)  │
│  ├─ Request Body & Multipart (Thread-local buffer pool)  │
│  └─ PEP 590 Vectorcall Direct Dispatch                   │
└───────────────┬──────────────────────────────────────────┘
                │ Extracted kwargs & dependencies
                ▼
┌──────────────────────────────────────────────────────────┐
│  Python Handlers & Dependencies                          │
│  def get_user(user_id: int, auth: Token, db = Depends()) │
└──────────────────────────────────────────────────────────┘
```

---

## Key Features

### ⚡ Rust-Powered Performance
- **PEP 590 Vectorcall**: Invokes Python handler functions directly via Vectorcall protocol, eliminating `PyDict` allocations for kwargs on the hot path.
- **64-Byte Cacheline Layout**: Internal `RouteEntry` metadata is packed into a single 64-byte struct for optimal CPU L1 cache locality.
- **Zero-Allocation Path Slicing**: URL path parameters are sliced directly from the request URI buffer without heap allocations.
- **Single-Pass Router**: Unified bitmask radix matching for instant HTTP `405 Method Not Allowed` resolution.
- **Thread-Local Buffer Pooling**: Request body readers reuse pooled memory buffers, minimizing GC pressure.
- **Pre-Baked Static Response Headers**: Eliminates dynamic string formatting for standard response headers.

### 🛡️ Built-in Security & Resilience
- **Native Rate Limiting**: In-memory sharded Token Bucket rate limiter with configurable keys (`ip` with proxy support, `header:<name>`, `global`) and automatic RFC-compliant `Retry-After` / `X-RateLimit-*` headers.
- **Adaptive Concurrency Limiting**: Automatic load shedding (`AdaptiveConcurrencyLimiter`) to prevent memory exhaustion (OOM) and tail latency spikes under traffic bursts.
- **JWT Authentication**: Native Rust verification (`require_jwt=True`) supporting HS256/384/510, RSA, EC, and EdDSA via [`oxyjwt`](https://github.com/QueryaHub/oxyjwt).
- **Security Middlewares**: High-performance CORS, CSRF (double-submit cookie), and browser Security Headers (HSTS, CSP, X-Frame-Options).

### 🛠️ Modern Developer Experience
- **DAG Dependency Injection**: Graph-based dependency resolution (`Depends`) with topological sorting and cycle detection at route registration time.
- **OpenAPI 3.1.0 & Interactive Docs**: Native `GET /openapi.json` with Scalar and Swagger UI mounted at `/docs`, automatic 401/422 documentation, and Pydantic v2 JSON Schema compatibility.
- **Auto Pydantic Model Inference**: Automatically infers and validates request bodies from type annotations without boilerplate.
- **Native RSGI WebSockets**: Full-duplex WebSocket support (`@app.websocket`) with concurrent send frame serialization and 1001 Going Away graceful shutdown broadcast.
- **Typed & Tested**: PEP 561 `py.typed` marker package with complete `_oxyroute.pyi` type stubs.

---

## Installation

```bash
pip install oxyroute
```

With development dependencies:
```bash
pip install "oxyroute[dev]"
```

> **Requirements**: Python ≥ 3.10 and [Granian](https://github.com/emmett-framework/granian) (`pip install granian`). Pre-built wheels are available for Linux (x86_64, aarch64), macOS (Apple Silicon & Intel), and Windows (x64).

---

## Quick Start

Create `main.py`:

```python
from pydantic import BaseModel
from oxyroute import App, Depends, HTTPException, Request

app = App(title="OxyRoute Production API", docs_ui="scalar")


class Item(BaseModel):
    name: str
    price: float


def get_db():
    return {"status": "connected"}


@app.get("/")
def read_root() -> dict[str, str]:
    return {"status": "healthy", "engine": "OxyRoute Rust RSGI"}


# Rate limited to 60 requests per minute by client IP
@app.get("/items/:item_id", rate_limit="60/minute")
def read_item(item_id: int, db=Depends(get_db)) -> dict:
    return {"item_id": item_id, "db_status": db["status"]}


# Automatically validates JSON request body via Pydantic
@app.post("/items", status_code=201)
def create_item(item: Item) -> dict:
    return {"created": item.name, "price": item.price}


# Native WebSocket endpoint
@app.websocket("/ws/echo")
async def echo_socket(ws) -> None:
    await ws.accept()
    while True:
        msg = await ws.receive_text()
        await ws.send_text(f"Echo: {msg}")
```

Run with Granian RSGI:

```bash
granian --interface rsgi --workers 4 --threads 2 main:app
```

Interactive documentation is automatically available at **`http://localhost:8000/docs`**.

---

## Examples & Patterns

### 1. Token Bucket Rate Limiting
```python
# Rate limit by custom API Key header
@app.get("/api/v1/data", rate_limit="1000/hour", rate_limit_key="header:X-API-Key")
def secure_data() -> dict:
    return {"data": "sensitive"}
```

### 2. DAG Dependency Injection
```python
from oxyroute import Depends

def config():
    return {"env": "prod"}

def auth_service(cfg = Depends(config)):
    return {"auth_mode": cfg["env"]}

@app.get("/profile")
def user_profile(auth = Depends(auth_service)):
    return {"auth": auth}
```

### 3. JWT Route Protection
```python
@app.get("/admin", require_jwt=True)
def admin_panel(request: Request) -> dict:
    claims = request.state.jwt_claims
    return {"admin_id": claims.get("sub")}
```

---

## Documentation

- **[Usage Guide](docs/usage.md)** — Complete configuration and operational reference.
- **[Rate Limiting](docs/rate-limiting.md)** — Token bucket algorithm, header formats, key strategies.
- **[RSGI & Granian](docs/rsgi.md)** — Architecture, worker lifecycle, and deployment.
- **[Dependencies & DAG](docs/dependencies.md)** — Dependency injection, topological ordering, lifecycle.
- **[OpenAPI & Docs](docs/openapi.md)** — OpenAPI 3.1.0 specifications, Scalar, and Swagger UI.
- **[WebSockets](docs/websocket.md)** — Full-duplex messaging, frame ordering, graceful shutdown.
- **[Database & SQLx](docs/database.md)** — Native database streaming and query helpers.
- **[Resilience](docs/resilience.md)** — Concurrency limiting and load shedding.
- **[Security](docs/jwt.md)** — JWT, CORS, CSRF, and Security Headers.

---

## Contributing

Contributions are warmly welcomed! Please read [CONTRIBUTING.md](CONTRIBUTING.md) and [Development Workflow](docs/development-workflow.md) before opening a pull request.

---

## License

OxyRoute is open-source software licensed under the **[MIT License](LICENSE)**.
