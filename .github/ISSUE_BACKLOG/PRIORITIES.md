# Priority and sequencing (OxyRoute backlog)

This file tracks **priority tiers** for the 20 items in [bodies/](bodies/); it does not replace the GitHub milestone—use it when triaging and when batch-creating issues.

## P0 (do first: correctness, safety, and coverage)

| # | File | Rationale |
|---|------|-----------|
| 2 | [02.md](bodies/02.md) | **Query decoding** — many clients break without percent-decoding. |
| 3 | [03.md](bodies/03.md) | **500 and logging** — unhandled `PyErr` and unclear production behavior. |
| 12 | [12.md](bodies/12.md) | **E2E Granian** — the main integration path (RSGI) is not validated with a real server in CI. |

## P1 (next: API and responses)

| # | File | Rationale |
|---|------|-----------|
| 1 | [01.md](bodies/01.md) | **PATCH** — Rust already supports it; Python surface is missing. |
| 5 | [05.md](bodies/05.md) | **DI chains + request context** — unlocks real dependency patterns. |
| 6 | [06.md](bodies/06.md) | **JWT `aud` / `iss` / leeway** — expected for production auth. |
| 10 | [10.md](bodies/10.md) | **Structured `Response` (headers, status)** — required for many APIs. |

## Research and architecture (heavier; schedule after P0/P1)

- **8, 15, 17** — asymmetric JWT / PyO3 upgrade / ASGI hardening: see [bodies/08.md](bodies/08.md), [15.md](bodies/15.md), [17.md](bodies/17.md).

## CI, release, and documentation

- **13, 14, 16** — clippy/CI, PyPI on tag, OpenAPI doc/test consistency.

## Broader feature set

- **4, 7, 9, 11, 18, 19, 20** — performance snapshot, cookies, Pydantic OpenAPI, middleware, lifespan, 405, HEAD/OPTIONS.

## Roadmap phasing (summary)

1. **Hardening:** P0 items + Issue **13** (clippy in CI) where feasible.  
2. **API surface:** P1 (1, 10) + **20** (HEAD/OPTIONS) as needed.  
3. **Auth and docs shape:** 6, 9, 16.  
4. **Scale and DI:** 4, 5.

[← Back to README](README.md)
