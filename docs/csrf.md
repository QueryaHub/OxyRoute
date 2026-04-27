# CSRF (optional)

[← Documentation index](index.md)

[Cross-Site Request Forgery (CSRF)](https://owasp.org/www-community/attacks/csrf/) is a **browser** risk: a logged-in user’s **cookies** (session, `HttpOnly` auth) can be sent to your site on a request you did not intend (e.g. a malicious form or script on another site). It does not apply the same way to stateless **Bearer token** flows where the client sets `Authorization` itself.

## When to use this module

- **Use** `CSRFConfig` / `apply_csrf` in `oxyroute/csrf.py` when your app uses **cookie-based** sessions or any cookie that the browser sends automatically, **and** the browser is allowed to hit **mutating** routes (`POST` / `PUT` / `PATCH` / `DELETE`).

- **Usually skip** for pure **JSON APIs** with only `Authorization: Bearer ...` and **no** session cookie — the attacker’s page cannot set `Authorization` for your API origin.

## How it works (double-submit)

1. The server (or a prior response) issues a **random token** and sets it in a **cookie** (e.g. via :meth:`oxyroute.csrf.CSRFConfig.set_cookie_value` on :class:`oxyroute.response.Response`).

2. The client must send the **same** value in a header (default: `X-CSRF-Token`).

3. The pre-route check compares cookie and header with **constant-time** equality. Mismatch or missing value → **403** JSON: `{"error":"csrf",...}`.

4. The check runs **before** the request body is read on the app path (same as :meth:`oxyroute.app.App.set_middleware`).

[SameSite](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Set-Cookie#samesitesamesite-value) on the session cookie (e.g. `Lax` or `Strict`) is a separate layer; double-submit is an extra control when you still need cross-site or legacy flows that cookies alone do not cover.

## Combining with CORS

Use a **single** `set_middleware` stack. CORS preflight is handled in :func:`oxyroute.cors.apply_cors`; run CSRF **inside** the `chain` so `OPTIONS` preflight is not subject to the CSRF token rule:

```python
from oxyroute import CORSConfig, apply_cors
from oxyroute.csrf import CSRFConfig, csrf_layer

cors = CORSConfig(allow_origins=["https://app.example"], allow_methods=[...])
csrf = CSRFConfig()
apply_cors(app, cors, chain=csrf_layer(csrf))
```

## See also

- [Handlers](handlers.md) — `set_middleware`, `Response`, cookies
- [CORS](cors.md)
- [OWASP CSRF Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html)
