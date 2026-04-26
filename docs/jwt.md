# JWT (HMAC) on the request path

[← Documentation index](index.md)

OxyRoute can validate **Bearer JWTs** in the **Rust** layer for routes you mark as protected. The implementation uses the [`jsonwebtoken`](https://crates.io/crates/jsonwebtoken) crate with an allow-list of algorithms per route. For **HMAC (symmetric)**, the built-in path supports **HS256**, **HS384**, and **HS512** when a secret and algorithm list are configured.

## Route options

The HTTP verb decorators on `App` accept:

- `require_jwt: bool` — if true, a valid `Authorization: Bearer <token>` header and successful verification are required before the handler runs
- `jwt_secret: str | None` — required for the default HMAC flow when `require_jwt` is set
- `algorithms: list[str] | None` — e.g. `["HS256"]` (defaults in code if omitted)
- `jwt_issuer: str | None` — if set, the `iss` claim must match (via [`jsonwebtoken`](https://crates.io/crates/jsonwebtoken) validation)
- `jwt_audience: str | None` — if set, the `aud` claim is validated against this value. If **omitted**, audience checking is **disabled** for that route (so tokens with an `aud` claim are not rejected for audience mismatch; opt in by passing `jwt_audience`)
- `jwt_leeway: int | None` — clock skew in **seconds** for `exp` / `nbf` (default when omitted: **60**, matching the `jsonwebtoken` crate default)

If `require_jwt` is set but the secret is missing for an all-HMAC algorithm set, **registration** fails with a clear error.

## Failure responses (no handler call)

The handler is **not** executed when:

- The route requires JWT but there is no usable secret
- The `Authorization` header is missing or not a Bearer token
- The token does not verify (including wrong algorithm/secret) — typically **401** with a plain `Unauthorized` body
- The token is expired in a way the library classifies as signature expiry — **401** with body `Expired` in the current implementation

Tuning beyond HMAC (RSA/EC) is not done in the core today; the error messages from the Rust `add_route` layer mention using **oxyjwt in the handler** for asymmetric algorithms in some cases.

## `decode_jwt_hs` (Python, for tests and parity)

The native module exports **`decode_jwt_hs(token, key, algorithm_list)`**, which is also re-exported from the top-level `oxyroute` package. It decodes a token and returns a Python object of claims, **HS\*** only, aligned for golden tests against the [`oxyjwt`](https://pypi.org/project/oxyjwt) package. See the test suite in `tests/test_jwt_parity.py` for an example of comparing outputs.

**Optional dev dependency:** `oxyjwt` is listed in `oxyroute[dev]` for these comparisons—it is not required in production for the minimal Rust request path.

## See also

- [Handlers](handlers.md) — `claims` in kwargs
- [Installation](installation.md) — `oxyroute[dev]`
