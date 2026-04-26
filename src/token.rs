//! JWT: Bearer extraction and HS* verification (aligned with oxyjwt validation defaults where applicable).

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use pyo3::prelude::*;
use pyo3::types::PyList;
use serde_json::Value as JsonValue;

pub fn extract_bearer(h: Option<&str>) -> Option<String> {
    let h = h?.trim();
    let b = h.strip_prefix("Bearer ")?.trim();
    if b.is_empty() {
        None
    } else {
        Some(b.to_string())
    }
}

/// Read a case-sensitive cookie name from a raw [`Cookie`][1] header value.
///
/// [1]: https://www.rfc-editor.org/rfc/rfc6265#section-4.2.1
pub fn extract_cookie_value(raw: &str, name: &str) -> Option<String> {
    for part in raw.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (k, v) = part.split_once('=')?;
        if k.trim() != name {
            continue;
        }
        let v = v.trim();
        let v = trim_cookie_dquotes(v);
        if v.is_empty() {
            return None;
        }
        return Some(v.to_string());
    }
    None
}

fn trim_cookie_dquotes(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Same `Algorithm` family as [`jsonwebtoken`][0] (HMAC, RSA, EC, EdDSA). Mixed families on
/// one route are rejected.
///
/// [0]: https://docs.rs/jsonwebtoken/
fn key_family(alg: Algorithm) -> u8 {
    use Algorithm::{
        EdDSA, ES256, ES384, HS256, HS384, HS512, PS256, PS384, PS512, RS256, RS384, RS512,
    };
    match alg {
        HS256 | HS384 | HS512 => 0,
        RS256 | RS384 | RS512 | PS256 | PS384 | PS512 => 1,
        ES256 | ES384 => 2,
        EdDSA => 3,
    }
}

/// Build a [`DecodingKey`] from `jwt_secret` text: HMAC **shared secret**, or **PEM** for
/// RSA / EC / Ed25519 (public key for verify).
pub fn build_decoding_key(
    key_material: &str,
    algs: &[Algorithm],
) -> jsonwebtoken::errors::Result<DecodingKey> {
    if algs.is_empty() {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidAlgorithm,
        ));
    }
    let fam = key_family(algs[0]);
    for a in algs {
        if key_family(*a) != fam {
            return Err(jsonwebtoken::errors::Error::from(
                jsonwebtoken::errors::ErrorKind::InvalidKeyFormat,
            ));
        }
    }
    match fam {
        0 => Ok(DecodingKey::from_secret(key_material.as_bytes())),
        1 => DecodingKey::from_rsa_pem(key_material.as_bytes()),
        2 => DecodingKey::from_ec_pem(key_material.as_bytes()),
        3 => DecodingKey::from_ed_pem(key_material.as_bytes()),
        _ => Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidAlgorithm,
        )),
    }
}

/// Used by the request path and for golden tests against `oxyjwt.decode`.
pub fn decode_hs_claims(
    token: &str,
    key: &str,
    algs: &[jsonwebtoken::Algorithm],
) -> jsonwebtoken::errors::Result<JsonValue> {
    if algs.is_empty() {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidAlgorithm,
        ));
    }
    for a in algs {
        if !matches!(a, Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512) {
            return Err(jsonwebtoken::errors::Error::from(
                jsonwebtoken::errors::ErrorKind::InvalidAlgorithm,
            ));
        }
    }
    let mut val = Validation::new(algs[0]);
    val.algorithms = algs.to_vec();
    val.validate_nbf = true;
    let dk = build_decoding_key(key, algs)?;
    let out = decode::<JsonValue>(token, &dk, &val)?;
    Ok(out.claims)
}

/// Exposed for golden tests next to `oxyjwt.decode` (same allow-list semantics for HS*).
#[pyfunction]
pub fn decode_jwt_hs(
    token: &str,
    key: &str,
    algorithm_list: &Bound<'_, PyList>,
) -> PyResult<Py<PyAny>> {
    let py = algorithm_list.py();
    let n = algorithm_list.len();
    let mut v = Vec::new();
    for i in 0..n {
        let s: String = algorithm_list.get_item(i)?.extract()?;
        v.push(match s.as_str() {
            "HS256" => jsonwebtoken::Algorithm::HS256,
            "HS384" => jsonwebtoken::Algorithm::HS384,
            "HS512" => jsonwebtoken::Algorithm::HS512,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "only HS* supported in decode_jwt_hs",
                ))
            }
        });
    }
    if v.is_empty() {
        v.push(jsonwebtoken::Algorithm::HS256);
    }
    let claims = decode_hs_claims(token, key, &v).map_err(|e| {
        if matches!(e.kind(), ErrorKind::ExpiredSignature) {
            pyo3::exceptions::PyValueError::new_err("expired")
        } else {
            pyo3::exceptions::PyValueError::new_err(e.to_string())
        }
    })?;
    let s = serde_json::to_string(&claims)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let j = py.import("json")?;
    Ok(j.call_method1("loads", (s,))?.unbind())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, Header};
    use serde_json::json;

    #[test]
    fn build_decoding_key_rs256_verifies() {
        let priv_pem = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/rsa/private_pkcs8.pem"
        ));
        let pub_pem = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/rsa/public_pkcs8.pem"
        ));
        let ek =
            jsonwebtoken::EncodingKey::from_rsa_pem(priv_pem.as_bytes()).expect("encoding key");
        let token = encode(
            &Header::new(Algorithm::RS256),
            &json!({ "sub": "a", "exp": 4_000_000_000_i64 }),
            &ek,
        )
        .expect("encode");
        let dk = build_decoding_key(pub_pem, &[Algorithm::RS256]).expect("decoding key");
        let mut val = Validation::new(Algorithm::RS256);
        val.validate_exp = false;
        val.algorithms = vec![Algorithm::RS256];
        let claims = decode::<JsonValue>(&token, &dk, &val)
            .expect("decode")
            .claims;
        assert_eq!(claims.get("sub"), Some(&json!("a")));
    }

    #[test]
    fn extract_cookie_finds_name() {
        assert_eq!(
            extract_cookie_value("a=1; access_token=eyJ.x.y; Path=/", "access_token").as_deref(),
            Some("eyJ.x.y")
        );
    }

    #[test]
    fn extract_cookie_quoted() {
        assert_eq!(
            extract_cookie_value(r#"t="a.b.c""#, "t").as_deref(),
            Some("a.b.c")
        );
    }
}
