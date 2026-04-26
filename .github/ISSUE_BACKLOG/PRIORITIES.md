# Priority and sequencing (OxyRoute backlog)

This file tracks **priority tiers** for items in [bodies/](bodies/). Use it with the GitHub milestone (see [README](README.md)). The **v0.2.0** issue batch (issues 1–3, 5–7, 9–16, 19–20, 15) is **shipped**; follow-up work is on milestone **[v0.3.0](https://github.com/QueryaHub/OxyRoute/milestone/2)** (issues **4, 8, 17, 18** on GitHub).

## P0 (active: performance and integration hardening)

| # | File | Rationale |
|---|------|-----------|
| 4 | [04.md](bodies/04.md) | **Route hot path** — reduce lock contention / route snapshot after `freeze()`. |
| 17 | [17.md](bodies/17.md) | **ASGI bridge** — `run_coroutine_threadsafe` / thread safety under concurrent load. |
| 18 | [18.md](bodies/18.md) | **App state / lifespan** — documented pattern and optional native `State` for shared resources. |

## P1 (next: auth and API polish)

| # | File | Rationale |
|---|------|-----------|
| 8 | [08.md](bodies/08.md) | **JWK / JWKS / OxyJWT alignment** — beyond current PEM/RS256; key rotation, docs. |
| 9 | [09.md](bodies/09.md) | **OpenAPI depth** (optional) — `$ref` / `$defs` and richer `requestBody` where needed. |

## Done (shipped; keep bodies for history)

- **Query, errors, E2E:** 2, 3, 12  
- **API surface:** 1, 5, 6, 7, 10, 11, 19, 20  
- **CI / release / docs:** 13, 14, 15, 16  

## Roadmap phasing (summary)

1. **P0:** 4 → 17 → 18 (order can overlap by contributor capacity).  
2. **P1:** 8 when product needs JWKS; 9 as polish.  

[← Back to README](README.md)
