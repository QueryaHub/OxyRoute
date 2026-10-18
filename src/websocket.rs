//! Native RSGI WebSocket helper exposed to Python as ``oxyroute._oxyroute.WebSocket``.
//!
//! The Python `@app.websocket(path)` decorator (see [oxyroute/app.py](../oxyroute/app.py))
//! registers a coroutine handler that receives a [`WebSocket`] instance built around the
//! underlying Granian [`RSGIWebsocketProtocol`][1]. All async methods delegate directly to
//! the Granian protocol / transport coroutines so Python keeps awaiting the same objects
//! it would have awaited natively — no extra Tokio future bridge, no scheduling cost.
//!
//! [1]: https://github.com/emmett-framework/granian — see `granian/rsgi.py`.
use std::sync::OnceLock;

use parking_lot::Mutex;
use pyo3::prelude::*;
use pyo3::types::PyDict;

struct WsHelpers {
    accept: Py<PyAny>,
    receive: Py<PyAny>,
    receive_text: Py<PyAny>,
    receive_bytes: Py<PyAny>,
    send_text: Py<PyAny>,
    send_bytes: Py<PyAny>,
    send_json: Py<PyAny>,
    close_noop: Py<PyAny>,
}

static WS_HELPERS: OnceLock<WsHelpers> = OnceLock::new();

impl WsHelpers {
    fn get(py: Python<'_>) -> PyResult<&'static WsHelpers> {
        if let Some(helpers) = WS_HELPERS.get() {
            return Ok(helpers);
        }
        let m = py.import("oxyroute._ws")?;
        let helpers = WsHelpers {
            accept: m.getattr("ws_accept")?.unbind(),
            receive: m.getattr("ws_receive")?.unbind(),
            receive_text: m.getattr("ws_receive_text")?.unbind(),
            receive_bytes: m.getattr("ws_receive_bytes")?.unbind(),
            send_text: m.getattr("ws_send_text")?.unbind(),
            send_bytes: m.getattr("ws_send_bytes")?.unbind(),
            send_json: m.getattr("ws_send_json")?.unbind(),
            close_noop: m.getattr("ws_close_noop")?.unbind(),
        };
        let _ = WS_HELPERS.set(helpers);
        Ok(WS_HELPERS.get().expect("just set"))
    }
}

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
    transport: Mutex<Option<Py<PyAny>>>,
    path_params: Vec<(String, String)>,
    closed: Mutex<bool>,
    write_lock: Py<PyAny>,
}

impl WebSocket {
    pub fn new(
        py: Python<'_>,
        protocol: Py<PyAny>,
        scope: Py<PyAny>,
        path_params: Vec<(String, String)>,
    ) -> PyResult<Self> {
        let asyncio = py.import("asyncio")?;
        let write_lock = asyncio.call_method0("Lock")?.unbind();
        Ok(Self {
            protocol,
            scope,
            transport: Mutex::new(None),
            path_params,
            closed: Mutex::new(false),
            write_lock,
        })
    }
}

#[pymethods]
impl WebSocket {
    #[new]
    #[pyo3(signature = (protocol, scope, path_params=None))]
    fn py_new(
        py: Python<'_>,
        protocol: Py<PyAny>,
        scope: Py<PyAny>,
        path_params: Option<Vec<(String, String)>>,
    ) -> PyResult<Self> {
        Self::new(py, protocol, scope, path_params.unwrap_or_default())
    }

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

    /// The HTTP ``Origin`` header sent by the client during the WebSocket handshake, if present.
    #[getter]
    fn origin<'py>(&self, py: Python<'py>) -> PyResult<Option<String>> {
        let s = self.scope.bind(py);
        let Ok(headers) = s.getattr("headers") else {
            return Ok(None);
        };
        Ok(crate::params::header_get_lax(&headers, "origin"))
    }

    /// Internal callback from `ws_accept` coroutine to store the Granian transport.
    fn _set_transport(&self, transport: Py<PyAny>) {
        *self.transport.lock() = Some(transport);
    }

    /// Internal callback from `ws_receive_*` coroutines when close frame (kind = 0) is encountered.
    fn _mark_closed(&self) {
        *self.closed.lock() = true;
    }

    /// Perform the handshake. Stores the transport for subsequent send/receive calls.
    fn accept<'py>(this: Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = this.py();
        let proto = this.borrow().protocol.clone_ref(py);
        let helpers = WsHelpers::get(py)?;
        helpers.accept.bind(py).call1((proto, this))
    }

    /// Receive the next WebSocket message; returns ``str`` or ``bytes``. Raises ``RuntimeError``
    /// if the peer has closed the connection (Granian close kind = 0).
    fn receive<'py>(this: Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = this.py();
        let transport = this
            .borrow()
            .transport
            .lock()
            .as_ref()
            .map(|t| t.clone_ref(py));
        let transport = match transport {
            Some(t) => t,
            None => {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "WebSocket: call accept() before send/receive",
                ));
            }
        };
        let helpers = WsHelpers::get(py)?;
        helpers.receive.bind(py).call1((transport, this))
    }

    /// Receive the next message as ``str``; raises ``ValueError`` if a binary frame arrives.
    fn receive_text<'py>(this: Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = this.py();
        let transport = this
            .borrow()
            .transport
            .lock()
            .as_ref()
            .map(|t| t.clone_ref(py));
        let transport = match transport {
            Some(t) => t,
            None => {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "WebSocket: call accept() before send/receive",
                ));
            }
        };
        let helpers = WsHelpers::get(py)?;
        helpers.receive_text.bind(py).call1((transport, this))
    }

    /// Receive the next message as ``bytes``; raises ``ValueError`` if a text frame arrives.
    fn receive_bytes<'py>(this: Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = this.py();
        let transport = this
            .borrow()
            .transport
            .lock()
            .as_ref()
            .map(|t| t.clone_ref(py));
        let transport = match transport {
            Some(t) => t,
            None => {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "WebSocket: call accept() before send/receive",
                ));
            }
        };
        let helpers = WsHelpers::get(py)?;
        helpers.receive_bytes.bind(py).call1((transport, this))
    }

    /// Send a text frame. Concurrent send calls are serialized to prevent frame interleaving.
    fn send_text<'py>(&self, py: Python<'py>, data: String) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport.lock().as_ref().map(|t| t.clone_ref(py));
        let transport = match transport {
            Some(t) => t,
            None => {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "WebSocket: call accept() before send/receive",
                ));
            }
        };
        let helpers = WsHelpers::get(py)?;
        helpers
            .send_text
            .bind(py)
            .call1((transport, self.write_lock.clone_ref(py), data))
    }

    /// Send a binary frame. Concurrent send calls are serialized to prevent frame interleaving.
    fn send_bytes<'py>(
        &self,
        py: Python<'py>,
        data: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport.lock().as_ref().map(|t| t.clone_ref(py));
        let transport = match transport {
            Some(t) => t,
            None => {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "WebSocket: call accept() before send/receive",
                ));
            }
        };
        let helpers = WsHelpers::get(py)?;
        helpers
            .send_bytes
            .bind(py)
            .call1((transport, self.write_lock.clone_ref(py), data))
    }

    /// Send a JSON-serialised object as a text frame. Concurrent send calls are serialized.
    fn send_json<'py>(
        &self,
        py: Python<'py>,
        data: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let transport = self.transport.lock().as_ref().map(|t| t.clone_ref(py));
        let transport = match transport {
            Some(t) => t,
            None => {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(
                    "WebSocket: call accept() before send/receive",
                ));
            }
        };
        let helpers = WsHelpers::get(py)?;
        helpers
            .send_json
            .bind(py)
            .call1((transport, self.write_lock.clone_ref(py), data))
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
        let helpers = WsHelpers::get(py)?;
        helpers.close_noop.bind(py).call0()
    }
}
