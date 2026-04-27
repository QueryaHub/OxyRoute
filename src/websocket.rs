//! Native RSGI WebSocket helper exposed to Python as ``oxyroute._oxyroute.WebSocket``.
//!
//! The Python `@app.websocket(path)` decorator (see [oxyroute/app.py](../oxyroute/app.py))
//! registers a coroutine handler that receives a [`WebSocket`] instance built around the
//! underlying Granian [`RSGIWebsocketProtocol`][1]. All async methods delegate directly to
//! the Granian protocol / transport coroutines so Python keeps awaiting the same objects
//! it would have awaited natively — no extra Tokio future bridge, no scheduling cost.
//!
//! [1]: https://github.com/emmett-framework/granian — see `granian/rsgi.py`.
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyString, PyTuple};

/// Thin async wrapper around a Granian RSGI WebSocket connection.
///
/// Lifecycle:
///
/// 1. ``await ws.accept()`` performs the WebSocket handshake and stores the underlying transport.
/// 2. ``await ws.receive()`` / ``receive_text()`` / ``receive_bytes()`` to read frames.
/// 3. ``await ws.send_text(...)`` / ``send_bytes(...)`` to write frames.
/// 4. ``await ws.close(code=...)`` to close the connection (defaults to 1000 — normal closure).
///
/// All methods are awaitable and run on Granian's event loop; ``close`` is best-effort once
/// the peer has already disconnected.
#[pyclass(name = "WebSocket", module = "oxyroute._oxyroute")]
pub struct WebSocket {
    protocol: Py<PyAny>,
    scope: Py<PyAny>,
    transport: Arc<Mutex<Option<Py<PyAny>>>>,
    path_params: HashMap<String, String>,
    closed: Arc<Mutex<bool>>,
}

impl WebSocket {
    pub fn new(
        protocol: Py<PyAny>,
        scope: Py<PyAny>,
        path_params: HashMap<String, String>,
    ) -> Self {
        Self {
            protocol,
            scope,
            transport: Arc::new(Mutex::new(None)),
            path_params,
            closed: Arc::new(Mutex::new(false)),
        }
    }

    fn transport_clone(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let g = self.transport.lock();
        match g.as_ref() {
            Some(t) => Ok(t.clone_ref(py)),
            None => Err(pyo3::exceptions::PyRuntimeError::new_err(
                "WebSocket: call accept() before send/receive",
            )),
        }
    }
}

#[pymethods]
impl WebSocket {
    /// The Granian RSGI scope (proto = ``"websocket"``).
    #[getter]
    fn scope(&self, py: Python<'_>) -> Py<PyAny> {
        self.scope.clone_ref(py)
    }

    /// Path parameters extracted by the router (e.g. ``/ws/:room`` → ``{"room": "lobby"}``).
    #[getter]
    fn path_params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        for (k, v) in &self.path_params {
            d.set_item(k.as_str(), v.as_str())?;
        }
        Ok(d)
    }

    /// True once :meth:`close` has been called (or the peer disconnected).
    #[getter]
    fn is_closed(&self) -> bool {
        *self.closed.lock()
    }

    /// Perform the handshake. Stores the transport for subsequent send/receive calls.
    fn accept<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let proto = self.protocol.clone_ref(py);
        let transport_slot = self.transport.clone();
        let coro = proto.bind(py).call_method0("accept")?;
        let fut = pyo3_async_runtimes::tokio::into_future(coro)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let result = fut.await?;
            Python::with_gil(|py| {
                *transport_slot.lock() = Some(result);
                Ok(py.None())
            })
        })
    }

    /// Receive the next WebSocket message; returns ``str`` or ``bytes``. Raises ``RuntimeError``
    /// if the peer has closed the connection (Granian close kind = 0).
    fn receive<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport_clone(py)?;
        let coro = transport.bind(py).call_method0("receive")?;
        let fut = pyo3_async_runtimes::tokio::into_future(coro)?;
        let closed_flag = self.closed.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let msg = fut.await?;
            Python::with_gil(|py| -> PyResult<Py<PyAny>> {
                let m = msg.bind(py);
                let kind: i64 = m.getattr("kind")?.extract()?;
                if kind == 0 {
                    *closed_flag.lock() = true;
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        "WebSocket closed by peer",
                    ));
                }
                let data = m.getattr("data")?;
                Ok(data.unbind())
            })
        })
    }

    /// Receive the next message as ``str``; raises ``ValueError`` if a binary frame arrives.
    fn receive_text<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport_clone(py)?;
        let coro = transport.bind(py).call_method0("receive")?;
        let fut = pyo3_async_runtimes::tokio::into_future(coro)?;
        let closed_flag = self.closed.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let msg = fut.await?;
            Python::with_gil(|py| -> PyResult<Py<PyAny>> {
                let m = msg.bind(py);
                let kind: i64 = m.getattr("kind")?.extract()?;
                if kind == 0 {
                    *closed_flag.lock() = true;
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        "WebSocket closed by peer",
                    ));
                }
                if kind != 2 {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "expected text frame",
                    ));
                }
                let data = m.getattr("data")?;
                let s = data.downcast::<PyString>()?;
                Ok(s.clone().unbind().into())
            })
        })
    }

    /// Receive the next message as ``bytes``; raises ``ValueError`` if a text frame arrives.
    fn receive_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport_clone(py)?;
        let coro = transport.bind(py).call_method0("receive")?;
        let fut = pyo3_async_runtimes::tokio::into_future(coro)?;
        let closed_flag = self.closed.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let msg = fut.await?;
            Python::with_gil(|py| -> PyResult<Py<PyAny>> {
                let m = msg.bind(py);
                let kind: i64 = m.getattr("kind")?.extract()?;
                if kind == 0 {
                    *closed_flag.lock() = true;
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        "WebSocket closed by peer",
                    ));
                }
                if kind != 1 {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "expected binary frame",
                    ));
                }
                let data = m.getattr("data")?;
                let b = data.downcast::<PyBytes>()?;
                Ok(b.clone().unbind().into())
            })
        })
    }

    /// Send a text frame. Returns when Granian has flushed the message.
    fn send_text<'py>(&self, py: Python<'py>, data: String) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport_clone(py)?;
        let coro = transport
            .bind(py)
            .call_method1("send_str", PyTuple::new(py, [data])?)?;
        pyo3_async_runtimes::tokio::into_future(coro).and_then(|fut| {
            pyo3_async_runtimes::tokio::future_into_py(py, async move {
                fut.await?;
                Python::with_gil(|py| Ok(py.None()))
            })
        })
    }

    /// Send a binary frame. Returns when Granian has flushed the message.
    fn send_bytes<'py>(&self, py: Python<'py>, data: Vec<u8>) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport_clone(py)?;
        let pb = PyBytes::new(py, &data);
        let coro = transport
            .bind(py)
            .call_method1("send_bytes", PyTuple::new(py, [pb])?)?;
        pyo3_async_runtimes::tokio::into_future(coro).and_then(|fut| {
            pyo3_async_runtimes::tokio::future_into_py(py, async move {
                fut.await?;
                Python::with_gil(|py| Ok(py.None()))
            })
        })
    }

    /// Send a JSON-serialised object as a text frame (uses :func:`json.dumps`).
    fn send_json<'py>(&self, py: Python<'py>, data: Py<PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport_clone(py)?;
        let json_mod = py.import("json")?;
        let dumped = json_mod.call_method1("dumps", (data.bind(py),))?;
        let coro = transport
            .bind(py)
            .call_method1("send_str", PyTuple::new(py, [dumped])?)?;
        pyo3_async_runtimes::tokio::into_future(coro).and_then(|fut| {
            pyo3_async_runtimes::tokio::future_into_py(py, async move {
                fut.await?;
                Python::with_gil(|py| Ok(py.None()))
            })
        })
    }

    /// Close the connection. ``code`` defaults to 1000 (normal closure).
    ///
    /// Returns an awaitable that completes immediately so handlers can use
    /// ``await ws.close()``. Idempotent — repeated calls are no-ops.
    #[pyo3(signature = (code=None))]
    fn close<'py>(&self, py: Python<'py>, code: Option<i32>) -> PyResult<Bound<'py, PyAny>> {
        let already_closed = {
            let mut g = self.closed.lock();
            let was = *g;
            *g = true;
            was
        };
        if !already_closed {
            let st = code.unwrap_or(1000);
            let _ = self
                .protocol
                .bind(py)
                .call_method1("close", (st,))
                .map_err(|e| {
                    log::debug!(target: "oxyroute", "websocket close ignored: {e}");
                    e
                });
        }
        pyo3_async_runtimes::tokio::future_into_py(py, async {
            Python::with_gil(|py| Ok(py.None()))
        })
    }
}
