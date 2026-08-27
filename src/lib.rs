use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;

use parking_lot::RwLock;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use serde_json::json;

mod config;
mod db;
mod dispatch;
mod form;
mod params;
mod response;
mod schema;
mod state;
mod token;
mod websocket;

use dispatch::{run_rsgi, try_rsgi_sync_short_circuit};
use state::AppState;

/// Hidden Criterion / microbench surface (issue #110). Not part of the stable Python API.
#[doc(hidden)]
pub mod microbench {
    use matchit::Router;
    use pyo3::prelude::*;

    pub use crate::schema::json_to_py;
    pub use crate::state::{match_route_compiled, CompiledRouters};

    /// Build a compiled GET router with one static and one param route for matching benches.
    pub fn sample_compiled_routers() -> CompiledRouters {
        let mut get = Router::new();
        get.insert("/hello", 0usize).expect("static route");
        get.insert("/items/:id", 1usize).expect("param route");
        let mut all_paths = Router::new();
        all_paths
            .insert("/hello", crate::state::MethodMask::from_method("GET"))
            .expect("static all_paths");
        all_paths
            .insert("/items/:id", crate::state::MethodMask::from_method("GET"))
            .expect("param all_paths");
        CompiledRouters {
            get,
            post: Router::new(),
            put: Router::new(),
            patch: Router::new(),
            delete: Router::new(),
            options: Router::new(),
            websocket: Router::new(),
            all_paths,
        }
    }

    /// Map a handler return value; returns HTTP status (0 if already sent).
    pub fn map_handler_return_status(py: Python<'_>, out: &Bound<'_, PyAny>) -> PyResult<u16> {
        crate::dispatch::microbench_map_handler_return(py, out)
    }
}

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

fn parse_dependencies(
    py: Python<'_>,
    dep_list: &Bound<PyList>,
) -> PyResult<Vec<state::DependencyEntry>> {
    let inspect = py.import("inspect")?;
    let iscoro = inspect.getattr("iscoroutinefunction")?;
    let n = dep_list.len();
    let mut names = HashSet::with_capacity(n);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let it = dep_list.get_item(i)?;
        let tup = it.downcast::<PyTuple>()?;
        if tup.len() != 2 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "each dependency must be (name, callable)",
            ));
        }
        let name: String = tup.get_item(0)?.extract()?;
        if !names.insert(name.clone()) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "duplicate dependency name",
            ));
        }
        let f: Py<PyAny> = tup.get_item(1)?.unbind();
        let is_async: bool = iscoro.call1((f.clone_ref(py),))?.extract()?;
        let f_b = f.bind(py);
        let wants_request: bool = dependency_wants_request(py, f_b)?;
        let (factory_params, factory_varkw) = handler_signature_kinds(py, f_b)?;
        out.push(state::DependencyEntry {
            name,
            factory: f,
            is_async,
            wants_request,
            factory_params,
            factory_varkw,
        });
    }
    Ok(out)
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
    /// Convert matchit `:name` / `*rest` templates to OpenAPI `{name}` / `{rest}` and
    /// collect path parameter objects.
    fn openapi_path_and_params(path: &str) -> (String, Vec<serde_json::Value>) {
        let mut out = String::with_capacity(path.len() + 8);
        let mut params = Vec::new();
        let chars: Vec<char> = path.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == ':' || c == '*' {
                i += 1;
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                if !name.is_empty() {
                    out.push('{');
                    out.push_str(&name);
                    out.push('}');
                    params.push(json!({
                        "name": name,
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    }));
                }
            } else {
                out.push(c);
                i += 1;
            }
        }
        (out, params)
    }

    fn openapi_ensure_bearer_auth(oa: &mut serde_json::Value) {
        let Some(root) = oa.as_object_mut() else {
            return;
        };
        let components = root.entry("components").or_insert_with(|| json!({}));
        let Some(comp) = components.as_object_mut() else {
            return;
        };
        let schemes = comp.entry("securitySchemes").or_insert_with(|| json!({}));
        if let Some(s) = schemes.as_object_mut() {
            s.entry("bearerAuth").or_insert_with(|| {
                json!({
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "JWT"
                })
            });
        }
    }

    fn openapi_add_path(
        oa: &mut serde_json::Value,
        method: &str,
        path: &str,
        op_id: &str,
        request_schema: Option<serde_json::Value>,
        require_jwt: bool,
        tags: Option<Vec<String>>,
    ) {
        let (oa_path, path_params) = Self::openapi_path_and_params(path);
        if require_jwt {
            Self::openapi_ensure_bearer_auth(oa);
        }
        if let Some(paths) = oa
            .as_object_mut()
            .and_then(|m| m.get_mut("paths"))
            .and_then(|p| p.as_object_mut())
        {
            let method_lc = method.to_lowercase();
            let path_entry = paths.entry(oa_path).or_insert_with(|| json!({}));
            if let Some(obj) = path_entry.as_object_mut() {
                let mut op = if let Some(schema) = request_schema {
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
                if let Some(op_obj) = op.as_object_mut() {
                    if !path_params.is_empty() {
                        op_obj.insert("parameters".to_string(), json!(path_params));
                    }
                    if require_jwt {
                        op_obj.insert("security".to_string(), json!([{ "bearerAuth": [] }]));
                    }
                    if let Some(t) = tags {
                        if !t.is_empty() {
                            op_obj.insert("tags".to_string(), json!(t));
                        }
                    }
                }
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
        signature = (method, path, handler, require_jwt=false, jwt_secret=None, algorithms=None, read_json_body=true, read_form_body=false, dependencies=None, jwt_issuer=None, jwt_audience=None, jwt_leeway=None, jwt_cookie=None, body_schema_json=None, body_model=None, tags=None, body_param_name=None)
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
        body_model: Option<Py<PyAny>>,
        tags: Option<Bound<'_, PyList>>,
        body_param_name: Option<String>,
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
        let jwt_leeway_v = jwt_leeway.unwrap_or(60);
        let (jwt_decoding_key, jwt_validation) = if require_jwt {
            let k = jwt_secret.as_deref().ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(
                    "require_jwt needs jwt_secret (HMAC shared secret, or public key PEM for RS*/PS*/ES*/EdDSA)",
                )
            })?;
            let (dk, val) = crate::token::build_route_jwt_state(
                k,
                &algs,
                jwt_issuer.as_deref(),
                jwt_audience.as_deref(),
                jwt_leeway_v,
            )
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "jwt_secret and algorithms are incompatible: {e}"
                ))
            })?;
            (Some(Arc::new(dk)), Some(Arc::new(val)))
        } else {
            (None, None)
        };
        let dependencies = if let Some(d) = dependencies {
            parse_dependencies(py, &d)?
        } else {
            vec![]
        };
        let op_id: String = handler
            .bind(py)
            .getattr(pyo3::intern!(py, "__name__"))?
            .extract()?;
        let (handler_param_names, handler_varkw) = handler_signature_kinds(py, handler.bind(py))?;
        let trivial_sync = !is_async
            && !require_jwt
            && !read_json_body
            && !read_form_body
            && dependencies.is_empty()
            && !handler_varkw
            && handler_param_names.is_empty();
        let mut st = self.state.write();
        let routes = Arc::make_mut(&mut st.routes);
        let idx = routes.len();
        let extra = Arc::new(state::RouteExtra {
            path_template: path.to_string(),
            jwt_cookie,
            jwt_decoding_key,
            jwt_validation,
            dependencies: Arc::<[state::DependencyEntry]>::from(dependencies),
            handler_param_names: Arc::new(handler_param_names),
            body_param_name: body_param_name.unwrap_or_else(|| "json".to_string()),
        });
        routes.push(state::RouteEntry {
            handler,
            body_model,
            extra,
            is_async,
            require_jwt,
            read_json_body,
            read_form_body,
            handler_varkw,
            trivial_sync,
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
        let tag_list: Option<Vec<String>> = if let Some(list) = tags {
            let n = list.len();
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                v.push(list.get_item(i)?.extract()?);
            }
            Some(v)
        } else {
            None
        };
        {
            let mut oa = st.openapi.lock();
            App::openapi_add_path(
                &mut oa.0,
                &method,
                &path,
                &op_id,
                request_schema,
                require_jwt,
                tag_list,
            );
            oa.1 = None;
        }
        {
            let mut m = state::map_method_router(&st, &method).ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err("unsupported HTTP method")
            })?;
            m.insert(&path, idx)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))?;
        }
        {
            let mut masks = st.path_method_masks.lock();
            masks.entry(path).or_default().insert_method(&method);
        }
        // Keep auto-compiled routing snapshots fresh when routes are added before explicit freeze().
        st.compiled = None;
        st.rebuild_snapshot();
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
        let ws_routes = Arc::make_mut(&mut st.websocket_routes);
        let idx = ws_routes.len();
        ws_routes.push(state::WebsocketRoute { handler, is_async });
        {
            let mut m = st.websocket.lock();
            m.insert(&path, idx)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))?;
        }
        st.compiled = None;
        st.rebuild_snapshot();
        Ok(())
    }

    /// Lock route registration. Linear dependency list is a valid topological order (DAG of independent roots).
    fn freeze(&self) -> PyResult<()> {
        let mut st = self.state.write();
        st.frozen = true;
        if st.compiled.is_none() {
            st.compiled = Some(Arc::new(st.snapshot_routers()));
        }
        st.rebuild_snapshot();
        Ok(())
    }

    fn set_openapi_served(&self, enabled: bool) -> PyResult<()> {
        let mut st = self.state.write();
        st.include_openapi = enabled;
        st.rebuild_snapshot();
        Ok(())
    }

    fn set_openapi_title(&self, title: &str) -> PyResult<()> {
        let st = self.state.read();
        let mut oa = st.openapi.lock();
        if let Some(info) =
            oa.0.as_object_mut()
                .and_then(|m| m.get_mut("info"))
                .and_then(|i| i.as_object_mut())
        {
            info.insert("title".to_string(), json!(title));
            oa.1 = None;
        }
        Ok(())
    }

    /// Enrich OpenAPI ``info`` / ``servers``. Pass JSON strings for ``contact`` and ``servers``.
    #[pyo3(signature = (description=None, contact_json=None, servers_json=None))]
    fn set_openapi_info(
        &self,
        description: Option<String>,
        contact_json: Option<String>,
        servers_json: Option<String>,
    ) -> PyResult<()> {
        let st = self.state.read();
        let mut oa = st.openapi.lock();
        let Some(root) = oa.0.as_object_mut() else {
            return Ok(());
        };
        if let Some(desc) = description {
            if let Some(info) = root.get_mut("info").and_then(|i| i.as_object_mut()) {
                info.insert("description".to_string(), json!(desc));
            }
        }
        if let Some(raw) = contact_json {
            let contact: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("invalid contact JSON: {e}"))
            })?;
            if let Some(info) = root.get_mut("info").and_then(|i| i.as_object_mut()) {
                info.insert("contact".to_string(), contact);
            }
        }
        if let Some(raw) = servers_json {
            let servers: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("invalid servers JSON: {e}"))
            })?;
            root.insert("servers".to_string(), servers);
        }
        oa.1 = None;
        Ok(())
    }

    /// Single optional pre-route hook. Return ``None`` to continue; otherwise the return value
    /// is mapped like a route handler (e.g. :class:`oxyroute.Response`, ``dict`` with ``status`` / ``body`` / ``headers``).

    #[pyo3(signature = (exc_type, handler))]
    fn add_exception_handler(
        &self,
        exc_type: pyo3::Bound<'_, pyo3::types::PyType>,
        handler: Py<PyAny>,
    ) -> PyResult<()> {
        let mut st = self.state.write();
        let is_async = pyo3::Python::with_gil(|py| -> PyResult<bool> {
            let inspect = py.import("inspect")?;
            inspect
                .getattr("iscoroutinefunction")?
                .call1((&handler,))?
                .extract::<bool>()
        })
        .unwrap_or(false);
        Arc::make_mut(&mut st.exception_handlers).push((exc_type.unbind(), handler, is_async));
        st.rebuild_snapshot();
        Ok(())
    }

    fn set_middleware(&self, handler: Option<Py<PyAny>>) -> PyResult<()> {
        let mut st = self.state.write();
        if let Some(h) = handler {
            st.request_middleware = Arc::new(vec![h]);
        } else {
            st.request_middleware = Arc::new(Vec::new());
        }
        st.rebuild_snapshot();
        Ok(())
    }

    /// Add a middleware to the stack. `phase` must be "request", "response", or "both".
    #[pyo3(signature = (handler, phase="request"))]
    fn add_middleware(&self, handler: Py<PyAny>, phase: &str) -> PyResult<()> {
        let mut st = self.state.write();
        if phase == "request" || phase == "both" {
            Arc::make_mut(&mut st.request_middleware).push(handler.clone());
        }
        if phase == "response" || phase == "both" {
            Arc::make_mut(&mut st.response_middleware).push(handler);
        }
        if phase != "request" && phase != "response" && phase != "both" {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "phase must be 'request', 'response', or 'both'",
            ));
        }
        st.rebuild_snapshot();
        Ok(())
    }

    /// Optional Python CORS config with ``response_header_pairs(scope)`` (see ``oxyroute.cors``).
    fn set_cors(&self, config: Option<Py<PyAny>>) -> PyResult<()> {
        let mut st = self.state.write();
        st.cors = config;
        st.rebuild_snapshot();
        Ok(())
    }

    /// Optional security-headers preset; ``response_header_pairs(scope)`` (see
    /// ``oxyroute.security_headers``), merged only for header names not already on the response.
    fn set_security_headers(&self, config: Option<Py<PyAny>>) -> PyResult<()> {
        let mut st = self.state.write();
        st.security_headers = config;
        st.rebuild_snapshot();
        Ok(())
    }

    /// Connect to a PostgreSQL database and store the pool in `AppState`.
    #[pyo3(signature = (url, max_connections=10))]
    fn setup_database<'py>(
        &self,
        py: Python<'py>,
        url: String,
        max_connections: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let state = self.state.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(max_connections)
                .connect(&url)
                .await
                .map_err(|e| {
                    pyo3::exceptions::PyRuntimeError::new_err(format!("DB connect error: {}", e))
                })?;
            let mut st = state.write();
            st.db_pool = Some(pool);
            st.rebuild_snapshot();
            Ok(())
        })
    }

    /// Close the PostgreSQL connection pool.
    fn close_database<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let state = self.state.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let pool = {
                let mut st = state.write();
                let p = st.db_pool.take();
                st.rebuild_snapshot();
                p
            };
            if let Some(p) = pool {
                p.close().await;
            }
            Ok(())
        })
    }

    fn handle_rsgi<'py>(
        this: PyRef<'py, Self>,
        py: Python<'py>,
        scope: &Bound<'py, PyAny>,
        protocol: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Some(obj) = try_rsgi_sync_short_circuit(py, &this.state, scope, protocol)? {
            return Ok(obj.into_bound(py));
        }
        let state = this.state.clone();
        let scope_py: Py<PyAny> = scope.as_any().clone().unbind();
        let protocol_py: Py<PyAny> = protocol.as_any().clone().unbind();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            run_rsgi(state, scope_py, protocol_py).await
        })
    }

    fn openapi_json(&self) -> PyResult<String> {
        let st = self.state.read();
        let mut oa = st.openapi.lock();
        if oa.1.is_none() {
            oa.1 = Some(Arc::new(oa.0.to_string()));
        }
        Ok(oa.1.as_ref().unwrap().to_string())
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
    m.add_class::<db::DBQuery>()?;
    m.add_function(wrap_pyfunction!(token::decode_jwt_hs, m)?)?;
    Ok(())
}
