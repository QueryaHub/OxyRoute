//! RSGI `response_bytes` / `response_str` / `response_empty` helpers (Granian RSGI spec).

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyString, PyTuple};

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
        send_str_sync(py, protocol, status, text, content_type)?;
        Ok(Py::from(pyo3::types::PyNone::get(py)).into_any())
    })
}

pub async fn send_empty(
    protocol: &Py<PyAny>,
    status: u16,
    content_type: Option<&str>,
) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        send_empty_sync(py, protocol, status, content_type)?;
        Ok(Py::from(pyo3::types::PyNone::get(py)).into_any())
    })
}

/// 405 with [`Allow`][1] and a small plain body (RFC 9110 §15.5.6).
///
/// [1]: https://www.rfc-editor.org/rfc/rfc9110#name-allow
pub async fn send_405_method_not_allowed(
    protocol: &Py<PyAny>,
    allow: &[String],
) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        send_405_method_not_allowed_sync(py, protocol, allow)?;
        Ok(Py::from(pyo3::types::PyNone::get(py)).into_any())
    })
}

/// RSGI `response_bytes` / `response_empty` with a full `[(name, value), ...]` header list.
pub async fn send_with_headers(
    protocol: &Py<PyAny>,
    status: u16,
    body: &[u8],
    headers: Vec<(String, String)>,
) -> PyResult<PyObject> {
    if body.is_empty() {
        return send_empty_with_header_pairs(protocol, status, headers).await;
    }
    Python::with_gil(|py| {
        let p = protocol.bind(py);
        let h = build_header_list_from_pairs(py, &headers)?;
        p.getattr("response_bytes")?.call1((status, h, body))?;
        Ok(Py::from(pyo3::types::PyNone::get(py)).into_any())
    })
}

async fn send_empty_with_header_pairs(
    protocol: &Py<PyAny>,
    status: u16,
    headers: Vec<(String, String)>,
) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        let p = protocol.bind(py);
        let h = build_header_list_from_pairs(py, &headers)?;
        p.getattr("response_empty")?.call1((status, h))?;
        Ok(Py::from(pyo3::types::PyNone::get(py)).into_any())
    })
}

/// HEAD: no message body, `Content-Length` as if a GET returned `full_body_len` (RFC 9110).
pub async fn send_head_simple(
    protocol: &Py<PyAny>,
    status: u16,
    full_body_len: usize,
    content_type: &str,
) -> PyResult<PyObject> {
    let headers = vec![
        ("content-type".to_string(), content_type.to_string()),
        ("content-length".to_string(), full_body_len.to_string()),
    ];
    send_empty_with_header_pairs(protocol, status, headers).await
}

fn build_header_list_from_pairs<'py>(
    py: Python<'py>,
    pairs: &[(String, String)],
) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    for (k, v) in pairs {
        let name: String = if k.eq_ignore_ascii_case("set-cookie") {
            "set-cookie".to_string()
        } else {
            k.to_ascii_lowercase()
        };
        let pair = PyTuple::new(
            py,
            [PyString::new(py, &name), PyString::new(py, v.as_str())],
        )?;
        out.append(pair)?;
    }
    Ok(out)
}

fn build_headers_ct<'py>(
    py: Python<'py>,
    content_type: Option<&str>,
) -> PyResult<Bound<'py, PyList>> {
    match content_type {
        None => Ok(PyList::empty(py)),
        Some(ct) => {
            let pair = PyTuple::new(
                py,
                [PyString::new(py, "content-type"), PyString::new(py, ct)],
            )?;
            Ok(PyList::new(py, [pair])?)
        }
    }
}

// ---------- inline (sync, GIL-already-held) RSGI dispatch helpers ----------

/// Sync `protocol.response_str(...)`; for [`try_rsgi_sync_short_circuit`] and async wrappers.
pub fn send_str_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    text: &str,
    content_type: &str,
) -> PyResult<()> {
    let p = protocol.bind(py);
    let h = build_headers_ct(py, Some(content_type))?;
    p.getattr("response_str")?.call1((status, h, text))?;
    Ok(())
}

/// Sync [`send_text`]: empty string uses `response_empty`.
pub fn send_text_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    text: &str,
    content_type: &str,
) -> PyResult<()> {
    if text.is_empty() {
        return send_empty_sync(py, protocol, status, Some(content_type));
    }
    send_str_sync(py, protocol, status, text, content_type)
}

/// Sync `protocol.response_bytes(...)` with a ``PyBytes`` buffer (no ``Vec`` copy).
pub fn send_pybytes_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    body: &Bound<'_, PyBytes>,
    content_type: &str,
) -> PyResult<()> {
    if body.as_bytes().is_empty() {
        return send_empty_sync(py, protocol, status, Some(content_type));
    }
    let p = protocol.bind(py);
    let h = build_headers_ct(py, Some(content_type))?;
    p.getattr("response_bytes")?.call1((status, h, body))?;
    Ok(())
}

/// Sync `protocol.response_bytes(...)`; falls back to `send_empty_sync` on empty body.
pub fn send_bytes_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    body: &[u8],
    content_type: &str,
) -> PyResult<()> {
    if body.is_empty() {
        return send_empty_sync(py, protocol, status, Some(content_type));
    }
    let p = protocol.bind(py);
    let h = build_headers_ct(py, Some(content_type))?;
    p.getattr("response_bytes")?.call1((status, h, body))?;
    Ok(())
}

/// Sync equivalent of [`send_empty`].
pub fn send_empty_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    content_type: Option<&str>,
) -> PyResult<()> {
    let p = protocol.bind(py);
    let h = build_headers_ct(py, content_type)?;
    p.getattr("response_empty")?.call1((status, h))?;
    Ok(())
}

/// Sync equivalent of [`send_with_headers`].
pub fn send_with_headers_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    body: &[u8],
    headers: Vec<(String, String)>,
) -> PyResult<()> {
    let p = protocol.bind(py);
    let h = build_header_list_from_pairs(py, &headers)?;
    if body.is_empty() {
        p.getattr("response_empty")?.call1((status, h))?;
    } else {
        p.getattr("response_bytes")?.call1((status, h, body))?;
    }
    Ok(())
}

/// Sync 405 with `Allow` and plain body.
pub fn send_405_method_not_allowed_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    allow: &[String],
) -> PyResult<()> {
    let headers = vec![
        ("allow".to_string(), allow.join(", ")),
        (
            "content-type".to_string(),
            "text/plain; charset=utf-8".to_string(),
        ),
    ];
    send_with_headers_sync(py, protocol, 405, b"Method Not Allowed", headers)
}

/// HEAD: no body, but `content-length` for `full_body_len`.
pub fn send_head_simple_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    full_body_len: usize,
    content_type: &str,
) -> PyResult<()> {
    let headers = vec![
        ("content-type".to_string(), content_type.to_string()),
        ("content-length".to_string(), full_body_len.to_string()),
    ];
    send_with_headers_sync(py, protocol, status, b"", headers)
}

/// HEAD with arbitrary headers; strips body, sets `content-length` from `full_body.len()`.
pub fn send_head_with_headers_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    full_body: &[u8],
    mut headers: Vec<(String, String)>,
) -> PyResult<()> {
    let len = full_body.len();
    headers.retain(|(k, _)| !k.eq_ignore_ascii_case("content-length"));
    headers.push(("content-length".to_string(), len.to_string()));
    send_with_headers_sync(py, protocol, status, b"", headers)
}
