# JWT on the request path

[← Documentation index](index.md)

OxyRoute can validate **Bearer JWTs** in the **Rust** layer for routes you mark as protected. The implementation uses the [`jsonwebtoken`](https://crates.io/crates/jsonwebtoken) crate: you pass an allow-list of **algorithm names** and **`jwt_secret`** text whose interpretation depends on the algorithm family.

## Algorithms and keys

- **HMAC (symmetric):** `HS256`, `HS384`, `HS512` — `jwt_secret` is the **shared secret** (raw bytes of the string).
- **RSA:** `RS256`, `RS384`, `RS512`, `PS256`, `PS384`, `PS512` — `jwt_secret` must be a **PEM-encoded public key** (or certificate PEM) for verification.
- **ECDSA:** `ES256`, `ES384` — `jwt_secret` is a **PEM-encoded public key** for the curve.
- **EdDSA:** `EdDSA` — `jwt_secret` is **PEM** for the public key.

Only one **family** (HMAC vs RSA vs EC vs Ed) may appear in `algorithms` for a given route. Unsupported or unknown names are **rejected at route registration** when `algorithms` is parsed from Python.

## Route options

The HTTP verb decorators on `App` accept:

- `require_jwt: bool` — if true, a valid JWT and successful verification are required before the handler runs (see below)
- `jwt_cookie: str | None` — if set, and there is no usable `Authorization: Bearer` value, the token is read from the `Cookie` header: the first non-empty `name=<value>` pair where `name` matches this string (case-sensitive). `Bearer` still wins when both are present
- `jwt_secret: str | None` — when `require_jwt` is set: HMAC **secret**, or **asymmetric public key PEM** (see above)
- `algorithms: list[str] | None` — e.g. `["HS256"]` or `["RS256"]` (defaults in code to `["HS256"]` if the list is omitted or empty is normalized to HMAC-256)
- `jwt_issuer: str | None` — if set, the `iss` claim must match (via [`jsonwebtoken`](https://crates.io/crates/jsonwebtoken) validation)
- `jwt_audience: str | None` — if set, the `aud` claim is validated against this value. If **omitted**, audience checking is **disabled** for that route (so tokens with an `aud` claim are not rejected for audience mismatch; opt in by passing `jwt_audience`)
- `jwt_leeway: int | None` — clock skew in **seconds** for `exp` / `nbf` (default when omitted: **60**, matching the `jsonwebtoken` crate default)

If `require_jwt` is set, **`jwt_secret` is required** and must **match** the chosen algorithms (e.g. PEM for `RS256`). Registration **fails** with a clear error if the key cannot be loaded or the family does not match.

## Failure responses (no handler call)

The handler is **not** executed when:

- The route requires JWT but `jwt_secret` is missing
- The `Authorization` header is not a usable Bearer token **and** there is no `jwt_cookie` value, or the named cookie is missing/empty; otherwise cookie-based token is used when `jwt_cookie` is set
- The token does not verify (including wrong algorithm/secret) — typically **401** with a plain `Unauthorized` body
- The token is expired in a way the library classifies as signature expiry — **401** with body `Expired` in the current implementation

## `decode_jwt_hs` (Python, for tests and HS parity)

The native module exports **`decode_jwt_hs(token, key, algorithm_list)`**, re-exported from the top-level `oxyroute` package. It decodes a token and returns claims, **HMAC (HS\*) only**, for golden tests against the [`oxyjwt`](https://pypi.org/project/oxyjwt) package. For **RSA/EC/Ed** verification and token generation in Python tests, use [`oxyjwt`](https://pypi.org/project/oxyjwt) (`EncodingKey.from_*_pem` / `DecodingKey.from_*_pem`).

**Optional dev dependencies:** `oxyjwt` is in `oxyroute[dev]` for tests; production only needs the native extension and your `jwt_secret` / `algorithms` configuration.

## See also

- [Handlers](handlers.md) — `claims` in kwargs
- [Installation](installation.md) — `oxyroute[dev]`
