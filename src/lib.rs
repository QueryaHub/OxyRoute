use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;

use parking_lot::RwLock;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use serde_json::json;

mod dispatch;
mod form;
mod params;
mod response;
mod schema;
mod state;
mod token;
mod websocket;

use dispatch::run_rsgi;
use state::AppState;

type ParsedDependencies = (Vec<String>, Vec<Py<PyAny>>, Vec<bool>, Vec<bool>);

/// Parameter names the route handler accepts, plus whether it has `**kwargs`.
fn handler_signature_kinds(
    py: Python<'_>,
    handler: &Bound<'_, PyAny>,
) -> PyResult<(HashSet<String>, bool)> {
    let d = PyDict::new(py);
    d.set_item("f", handler)?;
    py.run(
        c"import inspect\n_s = inspect.signature(f)\n_p = []\nfor _n, _param in _s.parameters.items():\n    if _param.kind in (inspect.Parameter.POSITIONAL_OR_KEYWORD, inspect.Parameter.KEYWORD_ONLY):\n        _p.append(_n)\n_w = False\nfor _x in _s.parameters.values():\n    if _x.kind == inspect.Parameter.VAR_KEYWORD:\n        _w = True\n        break",
        None,
        Some(&d),
    )?;
    let pl = d
        .get_item("_p")?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("handler _p"))?;
    let v: Vec<String> = pl.extract()?;
    let names: HashSet<String> = v.into_iter().collect();
    let w: bool = d
        .get_item("_w")?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("handler _w"))?
        .extract()?;
    Ok((names, w))
}

fn parse_algorithm(s: &str) -> PyResult<jsonwebtoken::Algorithm> {
    jsonwebtoken::Algorithm::from_str(s).map_err(|_| {
        pyo3::exceptions::PyValueError::new_err(
            "unknown JWT algorithm (see jsonwebtoken: HS* / RS* / PS* / ES* / EdDSA)",
        )
    })
}

fn parse_dependencies(py: Python<'_>, dep_list: &Bound<PyList>) -> PyResult<ParsedDependencies> {
    let inspect = py.import("inspect")?;
    let iscoro = inspect.getattr("iscoroutinefunction")?;
    let n = dep_list.len();
    let mut names = Vec::with_capacity(n);
    let mut facts = Vec::with_capacity(n);
    let mut asy = Vec::with_capacity(n);
    let mut want_req = Vec::with_capacity(n);
    for i in 0..n {
        let it = dep_list.get_item(i)?;
        let tup = it.downcast::<PyTuple>()?;
        if tup.len() != 2 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "each dependency must be (name, callable)",
            ));
        }
        let name: String = tup.get_item(0)?.extract()?;
        if names.contains(&name) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "duplicate dependency name",
            ));
        }
        let f: Py<PyAny> = tup.get_item(1)?.unbind();
        let is_a: bool = iscoro.call1((f.clone_ref(py),))?.extract()?;
        let f_b = f.bind(py);
        let has_req: bool = dependency_wants_request(py, f_b)?;
        names.push(name);
        facts.push(f);
        asy.push(is_a);
        want_req.push(has_req);
    }
    Ok((names, facts, asy, want_req))
}

/// True if the factory declares a `request` parameter (for the request context dict).
fn dependency_wants_request(py: Python<'_>, f: &Bound<'_, PyAny>) -> PyResult<bool> {
    let inspect = py.import("inspect")?;
    let sig = inspect.getattr("signature")?.call1((f,))?;
    let params = sig.getattr("parameters")?;
    params
        .getattr("__contains__")?
        .call1((pyo3::types::PyString::new(py, "request"),))?
        .extract()
}

#[pyclass]
pub struct App {
    state: Arc<RwLock<AppState>>,
}

impl App {
    fn openapi_add_path(
        oa: &mut serde_json::Value,
        method: &str,
        path: &str,
        op_id: &str,
        request_schema: Option<serde_json::Value>,
    ) {
        if let Some(paths) = oa
            .as_object_mut()
            .and_then(|m| m.get_mut("paths"))
            .and_then(|p| p.as_object_mut())
        {
            let method_lc = method.to_lowercase();
            let path_entry = paths.entry(path).or_insert_with(|| json!({}));
            if let Some(obj) = path_entry.as_object_mut() {
                let op = if let Some(schema) = request_schema {
                    json!({
                        "summary": op_id,
                        "operationId": op_id,
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": schema
                                }
                            }
                        },
                        "responses": { "200": { "description": "OK" } }
                    })
                } else {
                    json!({
                        "summary": op_id,
                        "operationId": op_id,
                        "responses": { "200": { "description": "OK" } }
                    })
                };
                obj.insert(method_lc, op);
            }
        }
    }
}

#[pymethods]
impl App {
    #[new]
    #[pyo3(signature = (include_openapi=true))]
    fn new(include_openapi: bool) -> Self {
        let mut s = AppState::new();
        s.include_openapi = include_openapi;
        Self {
            state: Arc::new(RwLock::new(s)),
        }
    }

    /// Paths use **matchit 0.7** style: `/user/:id`. Pass `dependencies=[("x", get_x), ...]`.
    #[pyo3(
        signature = (method, path, handler, require_jwt=false, jwt_secret=None, algorithms=None, read_json_body=true, read_form_body=false, dependencies=None, jwt_issuer=None, jwt_audience=None, jwt_leeway=None, jwt_cookie=None, body_schema_json=None)
    )]
    #[allow(clippy::too_many_arguments)]
    fn add_route(
        &self,
        py: Python<'_>,
        method: String,
        path: String,
        handler: Py<PyAny>,
        require_jwt: bool,
        jwt_secret: Option<String>,
        algorithms: Option<Bound<'_, PyList>>,
        read_json_body: bool,
        read_form_body: bool,
        dependencies: Option<Bound<'_, PyList>>,
        jwt_issuer: Option<String>,
        jwt_audience: Option<String>,
        jwt_leeway: Option<u64>,
        jwt_cookie: Option<String>,
        body_schema_json: Option<String>,
    ) -> PyResult<()> {
        {
            let st = self.state.read();
            if st.frozen {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "app is frozen; no more add_route",
                ));
            }
        }
        let inspect = py.import("inspect")?;
        let f = inspect.getattr("iscoroutinefunction")?;
        let is_async: bool = f.call1((handler.clone_ref(py),))?.extract()?;
        let mut algs: Vec<jsonwebtoken::Algorithm> = if let Some(list) = algorithms {
            let n = list.len();
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                let s: String = list.get_item(i)?.extract()?;
                v.push(parse_algorithm(&s)?);
            }
            v
        } else {
            vec![jsonwebtoken::Algorithm::HS256]
        };
        if algs.is_empty() {
            algs.push(jsonwebtoken::Algorithm::HS256);
        }
        if read_json_body && read_form_body {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "read_json_body and read_form_body are mutually exclusive",
            ));
        }
        if require_jwt && jwt_secret.is_none() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "require_jwt needs jwt_secret (HMAC shared secret, or public key PEM for RS*/PS*/ES*/EdDSA)",
            ));
        }
        if require_jwt {
            if let Some(k) = jwt_secret.as_deref() {
                crate::token::build_decoding_key(k, &algs).map_err(|e| {
                    pyo3::exceptions::PyValueError::new_err(format!(
                        "jwt_secret and algorithms are incompatible: {e}"
                    ))
                })?;
            }
        }
        let (dep_names, dep_factories, dep_is_async, dep_wants_request) =
            if let Some(d) = dependencies {
                parse_dependencies(py, &d)?
            } else {
                (vec![], vec![], vec![], vec![])
            };
        let op_id: String = handler
            .bind(py)
            .getattr(pyo3::intern!(py, "__name__"))?
            .extract()?;
        let (handler_param_names, handler_varkw) = handler_signature_kinds(py, handler.bind(py))?;
        let mut st = self.state.write();
        let idx = st.routes.len();
        st.routes.push(state::RouteEntry {
            handler,
            is_async,
            require_jwt,
            jwt_secret,
            algs: algs.clone(),
            jwt_issuer,
            jwt_audience,
            jwt_leeway: jwt_leeway.unwrap_or(60),
            jwt_cookie,
            read_json_body,
            read_form_body,
            dep_names,
            dep_factories,
            dep_is_async,
            dep_wants_request,
            handler_param_names,
            handler_varkw,
        });
        let request_schema: Option<serde_json::Value> = match body_schema_json
            .as_deref()
            .map(str::trim)
        {
            None | Some("") => None,
            Some(s) => Some(serde_json::from_str(s).map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("invalid body_schema JSON: {e}"))
            })?),
        };
        {
            let mut oa = st.openapi.lock();
            App::openapi_add_path(&mut oa, &method, &path, &op_id, request_schema);
        }
        {
            let mut m = state::map_method_router(&st, &method).ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err("unsupported HTTP method")
            })?;
            m.insert(&path, idx)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))?;
        }
        // Keep auto-compiled routing snapshots fresh when routes are added before explicit freeze().
        st.compiled = None;
        Ok(())
    }

    /// Register a WebSocket route. ``handler`` receives a single
    /// :class:`oxyroute._oxyroute.WebSocket` argument; sync handlers run inline, async
    /// handlers are awaited on Granian's loop. Same matchit ``/ws/:room`` syntax as HTTP routes.
    fn add_websocket_route(
        &self,
        py: Python<'_>,
        path: String,
        handler: Py<PyAny>,
    ) -> PyResult<()> {
        {
            let st = self.state.read();
            if st.frozen {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "app is frozen; no more add_websocket_route",
                ));
            }
        }
        let inspect = py.import("inspect")?;
        let f = inspect.getattr("iscoroutinefunction")?;
        let is_async: bool = f.call1((handler.clone_ref(py),))?.extract()?;
        let mut st = self.state.write();
        let idx = st.websocket_routes.len();
        st.websocket_routes
            .push(state::WebsocketRoute { handler, is_async });
        {
            let mut m = st.websocket.lock();
            m.insert(&path, idx)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))?;
        }
        st.compiled = None;
        Ok(())
    }

    /// Lock route registration. Linear dependency list is a valid topological order (DAG of independent roots).
    fn freeze(&self) -> PyResult<()> {
        let mut st = self.state.write();
        st.frozen = true;
        if st.compiled.is_none() {
            st.compiled = Some(Arc::new(st.snapshot_routers()));
        }
        Ok(())
    }

    fn set_openapi_served(&self, enabled: bool) -> PyResult<()> {
        let mut st = self.state.write();
        st.include_openapi = enabled;
        Ok(())
    }

    fn set_openapi_title(&self, title: &str) -> PyResult<()> {
        let st = self.state.read();
        let mut oa = st.openapi.lock();
        if let Some(info) = oa
            .as_object_mut()
            .and_then(|m| m.get_mut("info"))
            .and_then(|i| i.as_object_mut())
        {
            info.insert("title".to_string(), json!(title));
        }
        Ok(())
    }

    /// Single optional pre-route hook. Return ``None`` to continue; otherwise the return value
    /// is mapped like a route handler (e.g. :class:`oxyroute.Response`, ``dict`` with ``status`` / ``body`` / ``headers``).
    fn set_middleware(&self, handler: Option<Py<PyAny>>) -> PyResult<()> {
        let mut st = self.state.write();
        st.middleware = handler;
        Ok(())
    }

    /// Optional Python CORS config with ``response_header_pairs(scope)`` (see ``oxyroute.cors``).
    fn set_cors(&self, config: Option<Py<PyAny>>) -> PyResult<()> {
        let mut st = self.state.write();
        st.cors = config;
        Ok(())
    }

    /// Optional security-headers preset; ``response_header_pairs(scope)`` (see
    /// ``oxyroute.security_headers``), merged only for header names not already on the response.
    fn set_security_headers(&self, config: Option<Py<PyAny>>) -> PyResult<()> {
        let mut st = self.state.write();
        st.security_headers = config;
        Ok(())
    }

    fn handle_rsgi<'py>(
        this: PyRef<'py, Self>,
        py: Python<'py>,
        scope: &Bound<'py, PyAny>,
        protocol: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let state = this.state.clone();
        let scope_py: Py<PyAny> = scope.as_any().clone().unbind();
        let protocol_py: Py<PyAny> = protocol.as_any().clone().unbind();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            run_rsgi(state, scope_py, protocol_py).await
        })
    }

    fn openapi_json(&self) -> PyResult<String> {
        let st = self.state.read();
        let oa = st.openapi.lock();
        Ok(oa.to_string())
    }
}

/// FastAPI-style marker; the callable is read via :py:attr:`dependency`.
#[pyclass]
pub struct PyDepends {
    _call: Py<PyAny>,
}

#[pymethods]
impl PyDepends {
    #[new]
    fn new(call: Py<PyAny>) -> Self {
        Self { _call: call }
    }

    /// Underlying async or sync factory (resolved before the route handler in declaration order).
    fn dependency(slf: PyRef<'_, Self>, py: Python<'_>) -> Py<PyAny> {
        slf._call.clone_ref(py)
    }
}

#[pymodule]
fn _oxyroute(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<App>()?;
    m.add_class::<PyDepends>()?;
    m.add_class::<websocket::WebSocket>()?;
    m.add_function(wrap_pyfunction!(token::decode_jwt_hs, m)?)?;
    Ok(())
}
