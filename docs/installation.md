# Installation

[← Documentation index](index.md)

## Requirements

- **Python** 3.10 or newer (abi3 wheel target)
- **Released wheel:** only a pip-compatible environment
- **From source:** [Rust](https://www.rust-lang.org/) (stable), [maturin](https://www.maturin.rs/) ≥ 1.4 (per `pyproject.toml` build backend)

## Install from PyPI

When the package is published:

```bash
pip install oxyroute
```

Optional development and test dependencies (Granian, pytest, httpx, oxyjwt):

```bash
pip install "oxyroute[dev]"
```

## Build from a checkout

Use a virtual environment to avoid clashing with system Python (especially on **PEP 668** / “externally managed” distros).

```bash
python -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install -U pip maturin
maturin develop
```

`maturin develop` compiles the Rust extension and installs the `oxyroute` package in editable form.

Alternatives:

```bash
pip install .        # build + install
maturin build --release
pip install target/wheels/oxyroute-*.whl
```

## Troubleshooting

- **`patchelf` warning (Linux):** Maturin may log that setting `rpath` failed if `patchelf` is missing. For local development the extension often still loads; for minimal wheels, install `patchelf` or `pip install patchelf` as suggested in the Maturin message.
- **Import errors after editing Rust:** Re-run `maturin develop` (or your chosen build) so `oxyroute._oxyroute` matches `src/`.
- **Tests import the wrong package:** Run pytest from a **different directory** than the repository root or install a wheel, so that `import oxyroute` resolves to the **installed** package (see [development.md](development.md)).

## See also

- [Development](development.md)
- [RSGI and Granian](rsgi.md)
