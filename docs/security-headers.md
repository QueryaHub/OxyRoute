# Security headers (preset)

[← Documentation index](index.md)

A small preset reduces mistakes when hardening **browser-facing** HTTP responses. OxyRoute merges headers after your handler runs (and before CORS, when configured), **only for names that are not already set** on the response, so you can still override a single value with :class:`oxyroute.response.Response` headers.

## Usage

```python
from oxyroute import App, SecurityHeadersConfig

app = App()
app.set_security_headers(
    SecurityHeadersConfig(
        hsts="max-age=31536000; includeSubDomains",
        content_security_policy="default-src 'none'; frame-ancestors 'none'; base-uri 'none'",
    )
)
```

`set_security_headers(None)` disables the preset.

## HSTS and HTTPS

`Strict-Transport-Security` is emitted **only** when the RSGI `scope.scheme` is `https`. Use it in production where TLS terminates (or the reverse proxy sets the forwarded scheme the server sees as `https`). **Do not** send a long `max-age` over plain `http` in dev — some browsers will cache HSTS and keep forcing HTTPS, which is painful for local work.

**Staging** behind the same host name as production: a low `max-age` or a separate hostname avoids baking short-lived dev certs into long-lived HSTS on real domains.

## Fields

| Field | Default | Header |
|--------|---------|--------|
| `hsts` | `None` | `Strict-Transport-Security` (only if `https`) |
| `x_content_type_options` | `nosniff` | `X-Content-Type-Options` |
| `x_frame_options` | `DENY` | `X-Frame-Options` |
| `referrer_policy` | `strict-origin-when-cross-origin` | `Referrer-Policy` |
| `content_security_policy` | `None` | `Content-Security-Policy` (usually app-specific) |
| `permissions_policy` | `None` | `Permissions-Policy` |
| `extra` | `{}` | Additional `str` → `str` pairs |

CORS, when set via :func:`oxyroute.cors.apply_cors` or :meth:`oxyroute.app.App.set_cors`, is merged **after** this preset and may replace the same header name (for `Access-Control-*` there is no overlap in normal use).

## See also

- [CORS](cors.md)
- [Handlers](handlers.md) — return types and `Response` headers
- [MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CSP) and [OWASP](https://owasp.org/) for each header
