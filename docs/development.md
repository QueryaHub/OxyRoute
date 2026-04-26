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

## Continuous integration

The workflow at `.github/workflows/ci.yml` (job name: **ci**):

- Matrix across **OS** (e.g. Ubuntu, macOS, Windows) and **Python** 3.10 / 3.12 (see the file for current exclusions)
- Installs `maturin`, `pytest`, `httpx`, `oxyjwt`, builds a release wheel, installs it, and runs the tests as above

## See also

- [Installation](installation.md)
- [README – project layout](../README.md#project-layout)
