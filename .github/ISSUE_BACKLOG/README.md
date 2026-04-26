# GitHub issue backlog (batch-ready)

This directory holds **20 issue bodies** ([bodies/](bodies/)) and a [PRIORITIES.md](PRIORITIES.md) file. They were generated from the OxyRoute roadmap for **QueryaHub/OxyRoute**.

## Status and GitHub (living backlog)

- **Closed milestone [v0.2.0](https://github.com/QueryaHub/OxyRoute/milestone/1):** the original **20-issue** batch is **shipped** in `main`/`dev` (PyPI 0.1.x, CI, RSGI/OpenAPI surface, PyO3 upgrade, etc.).  
- **Active milestone [v0.3.0](https://github.com/QueryaHub/OxyRoute/milestone/2):** remaining **open** work — [#4](https://github.com/QueryaHub/OxyRoute/issues/4) (perf), [#8](https://github.com/QueryaHub/OxyRoute/issues/8) (JWK/oxyjwt), [#17](https://github.com/QueryaHub/OxyRoute/issues/17) (ASGI), [#18](https://github.com/QueryaHub/OxyRoute/issues/18) (lifespan/state). Optional OpenAPI depth (`$ref` / `$defs`): [09.md](bodies/09.md); issue #9 is closed — open a new issue if you pick this up.  
- **Do not re-run** `./scripts/create-github-issues.sh` on an already-populated repo (duplicates). Check open work with: `gh issue list -R QueryaHub/OxyRoute --state open`.

## Prerequisites

1. [GitHub CLI](https://cli.github.com/) installed (`gh`).
2. Authenticate: `gh auth login` (the repository cannot create issues in your project without a logged-in `gh` or a `GH_TOKEN` with `issues:write`).

## One-shot: create the milestone, labels, and all issues

From the **repository root**:

```bash
./scripts/create-github-issues.sh
```

The script:

- Ensures you are logged into `gh`.
- Creates labels (`P0`, `P1`, `research`, `enhancement`, `tech-debt`, `ci`, `documentation`, `test`) if missing.
- Creates milestone **`v0.2.0`** if missing.
- Opens **20 issues** with titles, bodies, labels, and the milestone (see the script for the exact mapping).

**Warning:** Running the script twice will **duplicate** issues. If you need idempotency, check open issues on GitHub first or add guards (not included).

## Manual: single issue

```bash
gh issue create --title "feat: expose PATCH routes on Python App" \
  --body-file .github/ISSUE_BACKLOG/bodies/01.md \
  -l enhancement -l P1 -m v0.2.0
```

## Table of issues

| # | Body file | Title (for `gh issue create`) | Labels (suggested) | Milestone |
|---|-----------|---------------------------------|--------------------|-----------|
| 1 | [bodies/01.md](bodies/01.md) | feat: expose PATCH routes on Python App | `enhancement`, `P1` | v0.2.0 |
| 2 | [bodies/02.md](bodies/02.md) | fix: apply URL decoding to query string keys and values | `bug`, `P0` | v0.2.0 |
| 3 | [bodies/03.md](bodies/03.md) | feat: map Python exceptions in dispatch to HTTP 500 with safe body | `enhancement`, `P0` | v0.2.0 |
| 4 | [bodies/04.md](bodies/04.md) | perf: reduce lock contention on route match (RWLock or route snapshot) | `tech-debt`, `enhancement` | v0.2.0 |
| 5 | [bodies/05.md](bodies/05.md) | feat: pass request context into dependencies and support dependency chains | `enhancement`, `P1` | v0.2.0 |
| 6 | [bodies/06.md](bodies/06.md) | feat: extend JWT Validation with iss/aud and optional leeway | `enhancement`, `P1` | v0.2.0 |
| 7 | [bodies/07.md](bodies/07.md) | feat: read JWT from Cookie header for require_jwt | `enhancement` | v0.2.0 |
| 8 | [bodies/08.md](bodies/08.md) | research+feat: JWK/PEM-based JWT verify in Rust (align with oxyjwt) | `enhancement`, `research` | v0.2.0 |
| 9 | [bodies/09.md](bodies/09.md) | feat: add optional Pydantic / JSON schema hooks for OpenAPI body | `enhancement` | v0.2.0 |
| 10 | [bodies/10.md](bodies/10.md) | feat: structured Response object for headers and status | `enhancement`, `P1` | v0.2.0 |
| 11 | [bodies/11.md](bodies/11.md) | feat: optional middleware chain in run_rsgi before route match | `enhancement` | v0.2.0 |
| 12 | [bodies/12.md](bodies/12.md) | test: e2e HTTP against granian --interface rsgi | `test`, `P0` | v0.2.0 |
| 13 | [bodies/13.md](bodies/13.md) | ci: add cargo clippy and Rust unit tests to workflow | `ci` | v0.2.0 |
| 14 | [bodies/14.md](bodies/14.md) | ci: PyPI publish on version tag (trusted publishing) | `ci` | v0.2.0 |
| 15 | [bodies/15.md](bodies/15.md) | tech-debt: upgrade pyo3 0.21 -> current and validate Granian RSGI | `tech-debt`, `research` | v0.2.0 |
| 16 | [bodies/16.md](bodies/16.md) | docs+test: clarify include_openapi vs set_openapi_served and 404 for /openapi.json | `documentation`, `test` | v0.2.0 |
| 17 | [bodies/17.md](bodies/17.md) | fix: review ASGI protocol bridge (run_coroutine_threadsafe) under load | `bug`, `research` | v0.2.0 |
| 18 | [bodies/18.md](bodies/18.md) | feat: pass app state / lifespan for shared resources (optional design) | `enhancement` | v0.2.0 |
| 19 | [bodies/19.md](bodies/19.md) | feat: return 405 when path exists for another method | `enhancement` | v0.2.0 |
| 20 | [bodies/20.md](bodies/20.md) | feat: register HEAD/OPTIONS and sensible defaults | `enhancement` | v0.2.0 |

## Priority tier (P0 / P1)

See [PRIORITIES.md](PRIORITIES.md) for the **current** order (v0.3.0 follow-ups: **4, 17, 18, 8**; optional **9**).

## Documentation

- [docs/index.md](../../docs/index.md) — main English docs for contributors and users.
- [CONTRIBUTING.md](../../CONTRIBUTING.md) — how to build and test locally.
