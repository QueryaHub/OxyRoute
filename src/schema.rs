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

struct PyStringVisitor<'py>(Python<'py>);

impl<'de, 'py> serde::de::Visitor<'de> for PyStringVisitor<'py> {
    type Value = Py<PyString>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string key")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PyString::new(self.0, v).unbind())
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PyString::new(self.0, v).unbind())
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PyString::new(self.0, &v).unbind())
    }
}

struct PyStringSeed<'py>(Python<'py>);

impl<'de, 'py> serde::de::DeserializeSeed<'de> for PyStringSeed<'py> {
    type Value = Py<PyString>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_str(PyStringVisitor(self.0))
    }
}

struct PyJsonVisitor<'py>(Python<'py>);

impl<'de, 'py> serde::de::Visitor<'de> for PyJsonVisitor<'py> {
    type Value = Py<PyAny>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("any valid JSON value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.into_py_any(self.0).map_err(serde::de::Error::custom)
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.into_py_any(self.0).map_err(serde::de::Error::custom)
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.into_py_any(self.0).map_err(serde::de::Error::custom)
    }

    fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.into_py_any(self.0).map_err(serde::de::Error::custom)
    }

    fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.into_py_any(self.0).map_err(serde::de::Error::custom)
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PyFloat::new(self.0, v).into_any().unbind())
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PyString::new(self.0, v).into_any().unbind())
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PyString::new(self.0, v).into_any().unbind())
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PyString::new(self.0, &v).into_any().unbind())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.0.None())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(self.0.None())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let py = self.0;
        let list = PyList::empty(py);
        while let Some(elem) = seq.next_element_seed(PyJsonSeed(py))? {
            list.append(elem.bind(py))
                .map_err(serde::de::Error::custom)?;
        }
        Ok(list.into_any().unbind())
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: serde::de::MapAccess<'de>,
    {
        let py = self.0;
        let dict = PyDict::new(py);
        while let Some(k) = access.next_key_seed(PyStringSeed(py))? {
            let v = access.next_value_seed(PyJsonSeed(py))?;
            dict.set_item(k.bind(py), v.bind(py))
                .map_err(serde::de::Error::custom)?;
        }
        Ok(dict.into_any().unbind())
    }
}

pub struct PyJsonSeed<'py>(pub Python<'py>);

impl<'de, 'py> serde::de::DeserializeSeed<'de> for PyJsonSeed<'py> {
    type Value = Py<PyAny>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_any(PyJsonVisitor(self.0))
    }
}

/// Deserialize raw JSON bytes directly into Python objects (`dict`, `list`, `str`, `int`,
/// `float`, `bool`, `None`) in a single pass without intermediate `serde_json::Value` AST
/// allocations.
pub fn json_bytes_to_py(py: Python<'_>, bytes: &[u8]) -> Result<Py<PyAny>, serde_json::Error> {
    use serde::de::DeserializeSeed;
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let val = PyJsonSeed(py).deserialize(&mut de)?;
    de.end()?;
    Ok(val)
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
                    .write_all(b", ")
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
                .write_all(b": ")
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
                    .write_all(b", ")
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
                    .write_all(b", ")
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
