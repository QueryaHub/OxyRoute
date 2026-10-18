//! JSON to Python and minimal body validation (object/array root).

use std::io::Write;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
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

fn write_py_json<W: Write>(obj: &Bound<'_, PyAny>, writer: &mut W, depth: usize) -> PyResult<()> {
    if depth > 128 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "maximum JSON nesting depth exceeded",
        ));
    }
    if obj.is_none() {
        writer
            .write_all(b"null")
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        return Ok(());
    }
    if let Ok(b) = obj.downcast::<PyBool>() {
        let s: &[u8] = if b.is_true() { b"true" } else { b"false" };
        writer
            .write_all(s)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        return Ok(());
    }
    if let Ok(i) = obj.downcast::<PyInt>() {
        if let Ok(val) = i.extract::<i64>() {
            write!(writer, "{}", val)
                .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            return Ok(());
        }
        if let Ok(val) = i.extract::<u64>() {
            write!(writer, "{}", val)
                .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            return Ok(());
        }
        if let Ok(val) = i.extract::<i128>() {
            write!(writer, "{}", val)
                .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            return Ok(());
        }
        return Err(pyo3::exceptions::PyValueError::new_err(
            "integer exceeds 128-bit range for native JSON",
        ));
    }
    if let Ok(f) = obj.downcast::<PyFloat>() {
        let val = f.value();
        if !val.is_finite() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "non-finite float in JSON",
            ));
        }
        serde_json::to_writer(&mut *writer, &val)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        return Ok(());
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        let s_str = s.to_str()?;
        serde_json::to_writer(&mut *writer, s_str)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        return Ok(());
    }
    if let Ok(d) = obj.downcast::<PyDict>() {
        writer
            .write_all(b"{")
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        let mut first = true;
        for (k, v) in d.iter() {
            if !first {
                writer
                    .write_all(b",")
                    .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            } else {
                first = false;
            }
            let k_str = match k.downcast::<PyString>() {
                Ok(s) => s.to_str()?,
                Err(_) => {
                    return Err(pyo3::exceptions::PyTypeError::new_err(
                        "JSON dict key must be str",
                    ))
                }
            };
            serde_json::to_writer(&mut *writer, k_str)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
            writer
                .write_all(b":")
                .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            write_py_json(&v, writer, depth + 1)?;
        }
        writer
            .write_all(b"}")
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        return Ok(());
    }
    if let Ok(l) = obj.downcast::<PyList>() {
        writer
            .write_all(b"[")
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        let mut first = true;
        for item in l.iter() {
            if !first {
                writer
                    .write_all(b",")
                    .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            } else {
                first = false;
            }
            write_py_json(&item, writer, depth + 1)?;
        }
        writer
            .write_all(b"]")
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        return Ok(());
    }
    if let Ok(t) = obj.downcast::<PyTuple>() {
        writer
            .write_all(b"[")
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        let mut first = true;
        for item in t.iter() {
            if !first {
                writer
                    .write_all(b",")
                    .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
            } else {
                first = false;
            }
            write_py_json(&item, writer, depth + 1)?;
        }
        writer
            .write_all(b"]")
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))?;
        return Ok(());
    }
    Err(pyo3::exceptions::PyTypeError::new_err(format!(
        "unsupported type for native JSON serialization: {}",
        obj.get_type().name()?
    )))
}

/// Serialize a Python object directly to JSON bytes in Rust without invoking ``json.dumps``.
///
/// Supports ``None``, ``bool``, ``int``, ``float``, ``str``, ``dict`` (with string keys),
/// ``list``, and ``tuple``. Returns an error if any unsupported object is encountered.
pub fn py_to_json_bytes(_py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let capacity = if let Ok(d) = obj.downcast::<PyDict>() {
        d.len().saturating_mul(32).max(128)
    } else if let Ok(l) = obj.downcast::<PyList>() {
        l.len().saturating_mul(16).max(128)
    } else {
        128
    };
    let mut buf = Vec::with_capacity(capacity);
    write_py_json(obj, &mut buf, 0)?;
    Ok(buf)
}

/// Fallback JSON serialization via Python's standard library ``json.dumps``.
pub fn json_dumps_bytes(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let jmod = py.import("json")?;
    let dumped = jmod.call_method1("dumps", (obj.clone().unbind(),))?;
    let s: String = dumped.extract()?;
    Ok(s.into_bytes())
}

/// Serialize a Python object to JSON bytes. First attempts direct native serialization;
/// if an unsupported type or non-string key is encountered, falls back to Python's ``json.dumps``.
pub fn py_to_json_bytes_or_fallback(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    match py_to_json_bytes(py, obj) {
        Ok(bytes) => Ok(bytes),
        Err(_) => json_dumps_bytes(py, obj),
    }
}
