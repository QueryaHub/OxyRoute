//! JSON to Python and minimal body validation (object/array root).

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PyString};
use pyo3::IntoPyObjectExt;
use serde_json::Number;
use serde_json::Value as JsonValue;

fn json_number_to_py(py: Python<'_>, n: &Number) -> PyResult<Py<PyAny>> {
    if let Some(i) = n.as_i64() {
        return i.into_py_any(py);
    }
    if let Some(u) = n.as_u64() {
        return u.into_py_any(py);
    }
    if let Some(f) = n.as_f64() {
        return Ok(PyFloat::new(py, f).unbind().into());
    }
    Err(pyo3::exceptions::PyValueError::new_err(
        "invalid JSON number",
    ))
}

fn json_value_to_py(py: Python<'_>, v: &JsonValue) -> PyResult<Py<PyAny>> {
    match v {
        JsonValue::Null => Ok(py.None()),
        JsonValue::Bool(b) => Ok(b.into_py_any(py)?),
        JsonValue::Number(n) => json_number_to_py(py, n),
        JsonValue::String(s) => Ok(PyString::new(py, s).into()),
        JsonValue::Array(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(json_value_to_py(py, item)?)?;
            }
            Ok(list.into())
        }
        JsonValue::Object(map) => {
            let dict = PyDict::new(py);
            for (k, val) in map {
                dict.set_item(k, json_value_to_py(py, val)?)?;
            }
            Ok(dict.into())
        }
    }
}

/// Convert a parsed ``serde_json::Value`` into an equivalent Python object without a JSON
/// string roundtrip through ``json.loads``.
pub fn json_to_py(py: Python<'_>, v: &JsonValue) -> PyResult<Py<PyAny>> {
    json_value_to_py(py, v)
}
