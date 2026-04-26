//! JWT: Bearer extraction and HS* verification (aligned with oxyjwt validation defaults where applicable).

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{DecodingKey, Validation, decode};
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
    let mut val = Validation::new(algs[0]);
    val.algorithms = algs.to_vec();
    val.validate_nbf = true;
    let dk = DecodingKey::from_secret(key.as_bytes());
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
    let claims = decode_hs_claims(token, key, &v)
        .map_err(|e| {
            if matches!(e.kind(), ErrorKind::ExpiredSignature) {
                pyo3::exceptions::PyValueError::new_err("expired")
            } else {
                pyo3::exceptions::PyValueError::new_err(e.to_string())
            }
        })?;
    let s = serde_json::to_string(&claims).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(e.to_string())
    })?;
    let j = py.import_bound("json")?;
    Ok(j.call_method1("loads", (s,))?.unbind())
}
