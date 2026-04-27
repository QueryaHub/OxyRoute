# Contributing to OxyRoute

Thank you for helping improve OxyRoute. This document covers how to build, test, and propose changes.

## Quick links

- [Documentation](docs/index.md) — RSGI, routing, handlers, JWT, OpenAPI, ASGI, development.
- [Issue backlog (batch templates)](.github/ISSUE_BACKLOG/README.md) — 20 planned issues and [PRIORITIES.md](.github/ISSUE_BACKLOG/PRIORITIES.md) (P0 / P1).
- A **code of conduct** may be linked from the repository home page (Community Standards) if the maintainers add one.

## Environment

- **Python** 3.10+ (see [pyproject.toml](pyproject.toml)).
- **Rust** stable toolchain and [maturin](https://www.maturin.rs/) to build the `oxyroute._oxyroute` extension.
- A **virtual environment** is strongly recommended (especially on “externally managed” Linux distributions, PEP 668).

```bash
python -m venv .venv
source .venv/bin/activate
pip install -U pip maturin
maturin develop
pip install -e ".[dev]"   # optional: granian, pytest, httpx, oxyjwt
```

## Running tests

**Avoid importing the unbuilt source tree** when pytest picks up `oxyroute/` without a matching native module. The CI job runs from a **temporary directory** and imports the **installed** wheel. Locally you can either:

- `cd` outside the repo and run:  
  `python -m pytest /path/to/OxyRoute/tests -v`  
  after `pip install` / `maturin develop` in the same environment, or  
- `pip install` the built wheel in a clean directory.

See [docs/development.md](docs/development.md) for more detail.

## Rust

```bash
cargo build
cargo clippy
```

## Branches, PRs, and atomic commits

- **Integration branch is `dev`:** always `git fetch origin`, `git checkout dev`, `git pull origin dev` **before** creating a branch. Open PRs with **base = `dev`**; maintainers promote to `main` for releases as needed.
- **One issue, one PR (when possible):** name the branch with the issue number, e.g. `issue-2-query-decode` or `feat/2-query-decode`. Read the GitHub issue (and backlog body if linked) before coding.
- After implementation, run **the same tests and linters as CI** (Ruff, `cargo fmt`/`clippy`, pytest) — see [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
- Use **atomic commits** (one logical change per commit). **Do not** put `.github/ISSUE_BACKLOG/` changes in the same commit as product code—backlog/templating updates should be a separate `docs:` (or `chore:`) commit, or a separate PR.
- Prefer **conventional commits**: `feat:`, `fix:`, `docs:`, `ci:`, `chore:`, `test:`.
- Full checklist: [docs/development-workflow.md](docs/development-workflow.md).

## Creating GitHub issues from the template backlog

If you maintain the repo and want to file the planned roadmap in one go:

1. `gh auth login`
2. From the repo root: `chmod +x scripts/create-github-issues.sh` (first time) then `./scripts/create-github-issues.sh`  
   **Warning:** the script is **not** idempotent; a second run will create duplicate issues.

To open a **single** issue: see `.github/ISSUE_BACKLOG/README.md`.

## License

By contributing, you agree that your contributions are licensed under the same terms as the project ([LICENSE](LICENSE), MIT).
