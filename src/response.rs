//! RSGI `response_bytes` / `response_str` / `response_empty` helpers (Granian RSGI spec).

use pyo3::prelude::*;
use pyo3::types::{PyList, PyString, PyTuple};

pub async fn send_text(
    protocol: &Py<PyAny>,
    status: u16,
    text: &str,
    content_type: &str,
) -> PyResult<PyObject> {
    if text.is_empty() {
        return send_empty(protocol, status, Some(content_type)).await;
    }
    send_str(protocol, status, text, content_type).await
}

pub async fn send_str(
    protocol: &Py<PyAny>,
    status: u16,
    text: &str,
    content_type: &str,
) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        let p = protocol.bind(py);
        let h = build_headers_ct(py, Some(content_type))?;
        p.getattr("response_str")?
            .call1((u16::from(status), h, text))?;
        Ok(pyo3::types::PyNone::get_bound(py).to_object(py))
    })
}

pub async fn send_empty(
    protocol: &Py<PyAny>,
    status: u16,
    content_type: Option<&str>,
) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        let p = protocol.bind(py);
        let h = build_headers_ct(py, content_type)?;
        p.getattr("response_empty")?
            .call1((u16::from(status), h))?;
        Ok(pyo3::types::PyNone::get_bound(py).to_object(py))
    })
}

pub async fn send_bytes(
    protocol: &Py<PyAny>,
    status: u16,
    body: &[u8],
    content_type: &str,
) -> PyResult<PyObject> {
    if body.is_empty() {
        return send_empty(protocol, status, Some(content_type)).await;
    }
    Python::with_gil(|py| {
        let p = protocol.bind(py);
        let h = build_headers_ct(py, Some(content_type))?;
        p.getattr("response_bytes")?
            .call1((u16::from(status), h, body))?;
        Ok(pyo3::types::PyNone::get_bound(py).to_object(py))
    })
}

fn build_headers_ct<'py>(
    py: Python<'py>,
    content_type: Option<&str>,
) -> PyResult<Bound<'py, PyList>> {
    match content_type {
        None => Ok(PyList::empty_bound(py)),
        Some(ct) => {
            let pair = PyTuple::new_bound(
                py,
                [
                    PyString::new_bound(py, "content-type"),
                    PyString::new_bound(py, ct),
                ],
            );
            Ok(PyList::new_bound(py, [pair]))
        }
    }
}
