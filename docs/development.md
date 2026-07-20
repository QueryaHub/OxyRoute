# Development

[← Documentation index](index.md)

## Local Development & Tests

The project uses `uv` for dependency management and a `Makefile` to handle building the Rust extension and running tests safely. You do not need to manually run `maturin develop` or create virtual environments.

```bash
# Install dependencies, build the extension, and run all linters & tests
make test
```

**Shadowing the installed package:** If you run `pytest` directly from the repository root, Python might import the raw source tree `oxyroute/` without the compiled `._oxyroute` binary, causing failures. 
The `make test` and `make pytest` commands automatically run tests from a temporary directory to avoid this issue.

### Other useful commands:
- `make lint` — Run `ruff` and `cargo clippy/fmt` checks.
- `make fix` — Auto-format code with `ruff format` and `cargo fmt`.
- `make develop` — Build the Rust extension into `.venv` without running tests.

## Writing Application Tests

OxyRoute ships with an integrated `TestClient` for writing synchronous HTTP tests against your application without needing to start a real server.

```python
from oxyroute import App
from oxyroute.testing import TestClient

app = App()

@app.get("/")
def home():
    return {"status": "ok"}

def test_home():
    with TestClient(app) as client:
        resp = client.get("/")
        assert resp.status_code == 200
        assert resp.json() == {"status": "ok"}
```

Using `with TestClient(app)` ensures that the application's `__rsgi_init__` and `__rsgi_del__` lifespan hooks are run synchronously.

## Granian RSGI (end-to-end)

`tests/test_granian_e2e.py` starts a real **Granian** subprocess with `--interface rsgi`, sends HTTP requests with **httpx**, then stops the server. It is part of the normal **pytest** run when `granian` is installed (`oxyroute[dev]` includes it). The same file runs in **CI** on every matrix combination (Linux, macOS, Windows), so the native RSGI path is exercised against a real server, not only the in-process httpx test transport.

## Continuous integration

The workflow at `.github/workflows/ci.yml` (job name: **ci**):

- Matrix across **OS** (Ubuntu, macOS, Windows) and **Python** 3.10 through 3.14 (see `.github/workflows/ci.yml`)
- Installs dev dependencies (including **granian**), builds a release wheel, installs it, and runs the full pytest suite as above

## Releasing to PyPI

Tag a release with a **`v`-prefixed** semver tag (example: **`v0.5.0`**). That triggers `.github/workflows/release-pypi.yml`, which builds an **sdist**, **manylinux** x86_64 wheels, **Windows** x64, and **macOS** arm64 + x86_64 wheels, then uploads to **PyPI** using a **project-scoped API token** stored in GitHub as **`PYPI_API_TOKEN`** (Secret or Environment variable) on the **`pypi`** environment. The publish step uses `secrets` first, then `vars` (so you can start with a variable and move the value to a **Secret** later).

**Before the first upload:**

1. Keep **`pyproject.toml`**, **`Cargo.toml`**, and **`oxyroute/__init__.py`** `__version__` in sync with the version you are releasing, and with the tag (e.g. `0.5.0` → tag `v0.5.0`).
2. On [PyPI](https://pypi.org), create a **scoped API token** for this project, then in GitHub → **Settings → Environments** create the **`pypi`** environment and add **`PYPI_API_TOKEN`** (strongly prefer an **Environment secret** over a **Variable**; tokens in Variables are visible to people with access to the environment).
3. Optional alternative to API tokens: [trusted publishing](https://docs.pypi.org/trusted-publishers/) (OIDC) — no long-lived token; then the workflow’s publish job should omit `with.password` and set `id-token: write` (see the PyPA action README).

Build jobs set **`CARGO_INCREMENTAL=0`** for more reproducible release artifacts; wheel builds use **`--locked`** with the committed **`Cargo.lock`**.

## See also

- [Installation](installation.md)
- [README – project layout](../README.md#project-layout)
