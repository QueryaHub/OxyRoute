//! Query string, headers, and lightweight path value coercion (schema-lite).

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPy;

pub fn parse_query(q: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    if q.is_empty() {
        return m;
    }
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            m.insert(k.to_string(), v.to_string());
        } else {
            m.insert(pair.to_string(), String::new());
        }
    }
    m
}

/// Best-effort: integers, floats, bools, else original string.
pub fn value_for_path_param(py: Python<'_>, s: &str) -> Py<PyAny> {
    if let Ok(i) = s.parse::<i64>() {
        if !s.contains('.') {
            return i.into_py(py);
        }
    }
    if let Ok(f) = s.parse::<f64>() {
        return f.into_py(py);
    }
    if s == "true" {
        return true.into_py(py);
    }
    if s == "false" {
        return false.into_py(py);
    }
    s.into_py(py)
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
