# Priority and sequencing (OxyRoute backlog)

This file tracks **priority tiers** for items in [bodies/](bodies/). The **next PyPI and GitHub milestone** is **[v0.2.0](https://github.com/QueryaHub/OxyRoute/milestone/1)** (release **0.2.0**). The first **20-issue** batch (01–20, plus shipped items) is in `main`/`dev` at **0.1.x**; everything below targets **0.2.0** unless noted.

## P0 (performance and core reliability)

| # | File | Rationale |
|---|------|-----------|
| 4 | [04.md](bodies/04.md) | **Route hot path** — reduce lock contention / route snapshot after `freeze()`. |
| 17 | [17.md](bodies/17.md) | **ASGI bridge** — `run_coroutine_threadsafe` / thread safety under concurrent load. |

## P1 (API ergonomics, security helpers, protocol features)

| # | File | Rationale |
|---|------|-----------|
| 8 | [08.md](bodies/08.md) | **JWK / JWKS** — [GitHub #8](https://github.com/QueryaHub/OxyRoute/issues/8). |
| 9 | [09.md](bodies/09.md) | **OpenAPI depth** (optional) — `$ref` / `$defs`. |

## Research / heavier (may slip past 0.2.0)

| # | File | Rationale |
|---|------|-----------|
| 25 | [25.md](bodies/25.md) | **HTTP/2** — docs + deployment guarantees — [GitHub #50](https://github.com/QueryaHub/OxyRoute/issues/50). |
| 26 | [26.md](bodies/26.md) | **SSE** streaming — [GitHub #51](https://github.com/QueryaHub/OxyRoute/issues/51). |
| 27 | [27.md](bodies/27.md) | **WebSockets** — [GitHub #52](https://github.com/QueryaHub/OxyRoute/issues/52). |

## Done (0.1.x; keep bodies for history)

- **Query, errors, E2E:** 2, 3, 12  
- **API surface:** 1, 5, 6, 7, 10, 11, 19, 20  
- **CI / release / docs / PyO3:** 13, 14, 15, 16  
- **Lifespan / `app.state`:** 18 (see `App.state`, `examples/rsgi_lifespan_app.py`, `docs/rsgi.md`)  
- **Form bodies:** 22 / [#47](https://github.com/QueryaHub/OxyRoute/issues/47) — `read_form_body`, `form` / `files` kwargs, `docs/handlers.md`  
- **HTTPException:** 23 / [#48](https://github.com/QueryaHub/OxyRoute/issues/48) — `oxyroute.exceptions`, `docs/handlers.md` (per-type `register_exception_handler` not in scope)  
- **Sub-routers:** 21 / [#46](https://github.com/QueryaHub/OxyRoute/issues/46) — `APIRouter`, `include_router`, `docs/routing.md`  
- **CORS:** 24 / [#49](https://github.com/QueryaHub/OxyRoute/issues/49) — `CORSConfig`, `apply_cors`, `set_cors`, `docs/cors.md`  
- **Security headers:** 29 / [#54](https://github.com/QueryaHub/OxyRoute/issues/54) — `SecurityHeadersConfig`, `set_security_headers`, `docs/security-headers.md`  
- **CSRF:** 28 / [#53](https://github.com/QueryaHub/OxyRoute/issues/53) — `CSRFConfig`, `apply_csrf`, `csrf_layer`, `docs/csrf.md`  

## Roadmap phasing (summary)

1. **P0:** 4, 17; **18** / **#47 (form)** done (order flexible).  
2. **P1:** 8; **#48** / **#46** / **#49 (CORS)** / **#54 (security headers)** / **#53 (CSRF)** done; 9 as polish.  
3. **Research:** 50, 51, 52 — as capacity allows.  

[← Back to README](README.md)
