//! Query string, headers, and lightweight path value coercion (schema-lite).

use std::collections::HashMap;

use form_urlencoded::parse as parse_urlencoded;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;

/// Context dict passed as the `request` keyword to dependency callables that declare it:
/// `method`, `path`, `query_string`, and `headers` (str → str, lowercased names when from the ASGI bridge).
pub fn build_request_context<'py>(
    py: Python<'py>,
    scope: &Bound<'py, PyAny>,
    method: &str,
    path: &str,
    query_string: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let module = py.import("oxyroute.request")?;
    let req_cls = module.getattr("Request")?;
    req_cls.call1((scope, method, path, query_string))
}

/// Parse an HTTP `query` string (the part after `?`, without the `?`).
/// Uses [`form_urlencoded`] so keys and values are **percent-decoded** and `+` in values is
/// treated as a space, consistent with `application/x-www-form-urlencoded` / URLSearchParams
/// (see [WHATWG](https://url.spec.whatwg.org/#application/x-www-form-urlencoded)). Duplicate keys
/// are **last-wins** in the returned `HashMap` (not a multimap).
pub fn parse_query(q: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if q.is_empty() {
        return m;
    }
    for (k, v) in parse_urlencoded(q.as_bytes()) {
        m.insert(k.into_owned(), v.into_owned());
    }
    m
}

/// Best-effort: integers, floats, bools, else original string.
pub fn value_for_path_param(py: Python<'_>, s: &str) -> Py<PyAny> {
    if let Ok(i) = s.parse::<i64>() {
        if !s.contains('.') {
            return i.into_py_any(py).expect("i64 to Python");
        }
    }
    if let Ok(f) = s.parse::<f64>() {
        return f.into_py_any(py).expect("f64 to Python");
    }
    if s == "true" {
        return true.into_py_any(py).expect("bool to Python");
    }
    if s == "false" {
        return false.into_py_any(py).expect("bool to Python");
    }
    s.to_string().into_py_any(py).expect("str to Python")
}

pub fn header_get_lax(headers: &Bound<'_, PyAny>, name: &str) -> Option<String> {
    if let Ok(d) = headers.downcast::<PyDict>() {
        for key in [name, &name.to_lowercase(), &name.to_uppercase()] {
            if let Ok(Some(v)) = d.get_item(key) {
                if let Ok(s) = v.extract::<String>() {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
    }
    let get = headers.getattr("get").ok()?;
    for key in [name, &name.to_lowercase(), &name.to_uppercase()] {
        if let Ok(v) = get.call1((key,)) {
            if v.is_none() {
                continue;
            }
            if let Ok(s) = v.extract::<String>() {
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_query;

    #[test]
    fn decodes_percent_encoded_space() {
        let m = parse_query("q=hello%20world");
        assert_eq!(m.get("q").map(String::as_str), Some("hello world"));
    }

    #[test]
    fn decodes_utf8_in_value() {
        let m = parse_query("x=%C3%A9");
        assert_eq!(m.get("x").map(String::as_str), Some("é"));
    }

    #[test]
    fn plus_treated_as_space() {
        let m = parse_query("q=a+b");
        assert_eq!(m.get("q").map(String::as_str), Some("a b"));
    }

    #[test]
    fn duplicate_keys_last_wins() {
        let m = parse_query("a=1&a=2");
        assert_eq!(m.get("a").map(String::as_str), Some("2"));
    }

    #[test]
    fn empty_string_yields_empty_map() {
        assert!(parse_query("").is_empty());
    }

    #[test]
    fn decodes_key_and_value() {
        let m = parse_query("k%3Dey=v%3Dalue");
        assert_eq!(m.get("k=ey").map(String::as_str), Some("v=alue"));
    }
}
