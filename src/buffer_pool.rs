//! Zero-copy request body abstraction (issues #160, #219).

use std::ops::Deref;

use pyo3::prelude::*;
use pyo3::pybacked::PyBackedBytes;
use pyo3::types::{PyByteArray, PyBytes, PyString};

/// Zero-copy request body container (issue #219).
///
/// Avoids copying bytes out of Python's memory when the request body arrives as
/// `PyBytes`, avoids pool churn when no body is read, and safely implements
/// `Deref<Target = [u8]>`, `Send`, and `Sync`.
pub enum RequestBody {
    /// Zero-copy borrow of a Python `bytes` or `bytearray` buffer.
    Py(PyBackedBytes),
    /// Owned bytes (for fallback when body is a string or custom sequence).
    Bytes(Vec<u8>),
    /// Empty body (GET, HEAD, or routes where body is not read).
    Empty,
}

impl RequestBody {
    /// Construct a `RequestBody` from a Python object without copying byte buffers if possible.
    /// Returns the `RequestBody` along with an `Option<PyObject>` representing the original
    /// `PyBytes` if the input was already a `PyBytes` instance (for zero-copy kwargs pass-through).
    pub fn from_py_any(b: &Bound<'_, PyAny>) -> (Self, Option<PyObject>) {
        if let Ok(py_bytes) = b.downcast::<PyBytes>() {
            let backed = PyBackedBytes::from(py_bytes.clone());
            let py_obj = py_bytes.as_any().clone().unbind();
            (RequestBody::Py(backed), Some(py_obj))
        } else if let Ok(py_bytearray) = b.downcast::<PyByteArray>() {
            (
                RequestBody::Py(PyBackedBytes::from(py_bytearray.clone())),
                None,
            )
        } else if let Ok(py_str) = b.downcast::<PyString>() {
            if let Ok(s) = py_str.to_str() {
                (RequestBody::Bytes(s.as_bytes().to_vec()), None)
            } else {
                (RequestBody::Empty, None)
            }
        } else if let Ok(bytes_vec) = b.extract::<Vec<u8>>() {
            (RequestBody::Bytes(bytes_vec), None)
        } else {
            (RequestBody::Empty, None)
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            RequestBody::Py(b) => b,
            RequestBody::Bytes(v) => v,
            RequestBody::Empty => &[],
        }
    }
}

impl Deref for RequestBody {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl AsRef<[u8]> for RequestBody {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_body_py_bytes() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_b = PyBytes::new(py, b"hello world");
            let (body, py_obj) = RequestBody::from_py_any(py_b.as_any());
            assert_eq!(body.len(), 11);
            assert_eq!(body.as_bytes(), b"hello world");
            assert_eq!(&*body, b"hello world");
            assert!(!body.is_empty());
            assert!(py_obj.is_some());
            let py_obj_bound = py_obj.unwrap().into_bound(py);
            assert!(py_obj_bound.is_exact_instance_of::<PyBytes>());
            assert_eq!(
                py_obj_bound.downcast::<PyBytes>().unwrap().as_bytes(),
                b"hello world"
            );
        });
    }

    #[test]
    fn test_request_body_py_bytearray() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_ba = PyByteArray::new(py, b"bytearray content");
            let (body, py_obj) = RequestBody::from_py_any(py_ba.as_any());
            assert_eq!(body.as_bytes(), b"bytearray content");
            assert!(py_obj.is_none());
        });
    }

    #[test]
    fn test_request_body_py_string() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let py_s = PyString::new(py, "string content");
            let (body, py_obj) = RequestBody::from_py_any(py_s.as_any());
            assert_eq!(body.as_bytes(), b"string content");
            assert!(py_obj.is_none());
        });
    }

    #[test]
    fn test_request_body_empty() {
        let body = RequestBody::Empty;
        assert!(body.is_empty());
        assert_eq!(body.len(), 0);
        assert_eq!(body.as_bytes(), b"");
    }
}
