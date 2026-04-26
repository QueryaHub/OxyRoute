# Development

[← Documentation index](index.md)

## Rust

```bash
cargo build
cargo clippy
```

Release settings are in `Cargo.toml` (`lto`, `codegen-units`).

## Python extension (Maturin)

```bash
maturin develop
# or
maturin build --release
```

See [installation.md](installation.md) for venvs and the `patchelf` note on some Linux systems.

## Tests

The suite uses **pytest** and is configured in `pyproject.toml` with `testpaths = ["tests"]`.

**Shadowing the installed package:** If you run `pytest` from the **repository root**, Python can import the **source tree** `oxyroute/` (without a rebuilt `._oxyroute` binary) and fail in confusing ways. The CI job runs from a **temporary directory** and points pytest at the workspace tests, so the **installed** wheel is imported.

**Locally**, either:

- `cd` to a different directory and run:  
  `python -m pytest /path/to/OxyRoute/tests`  
  after `pip install` / `maturin develop` in your environment, or  
- `pip install -e` / install the wheel, then use a **clean** working directory for pytest.

**Optional dev extra:**

```bash
pip install "oxyroute[dev]"  # in your dev env, from source after maturin develop
```

## Granian RSGI (end-to-end)

`tests/test_granian_e2e.py` starts a real **Granian** subprocess with `--interface rsgi`, sends HTTP requests with **httpx**, then stops the server. It is part of the normal **pytest** run when `granian` is installed (`oxyroute[dev]` includes it). The same file runs in **CI** on every matrix combination (Linux, macOS, Windows), so the native RSGI path is exercised against a real server, not only the ASGI in-process transport.

## Continuous integration

The workflow at `.github/workflows/ci.yml` (job name: **ci**):

- Matrix across **OS** (Ubuntu, macOS, Windows) and **Python** 3.10 through 3.14 (see `.github/workflows/ci.yml`)
- Installs dev dependencies (including **granian**), builds a release wheel, installs it, and runs the full pytest suite as above

## See also

- [Installation](installation.md)
- [README – project layout](../README.md#project-layout)
