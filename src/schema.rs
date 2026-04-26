//! JSON to Python and minimal body validation (object/array root).

use pyo3::prelude::*;
use serde_json::Value as JsonValue;

pub fn json_to_py(py: Python<'_>, v: JsonValue) -> PyResult<Py<PyAny>> {
    let s = serde_json::to_string(&v).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(e.to_string())
    })?;
    let json = py.import_bound("json")?;
    Ok(json
        .call_method1("loads", (s,))?
        .unbind())
}
