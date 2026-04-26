use std::sync::Arc;
use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::PyList;
use serde_json::json;

mod dispatch;
mod params;
mod response;
mod schema;
mod state;
mod token;

use dispatch::run_rsgi;
use state::AppState;

pub(crate) fn lock_err<T>(e: std::sync::PoisonError<T>) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
}

fn parse_algorithm(s: &str) -> PyResult<jsonwebtoken::Algorithm> {
    match s {
        "HS256" => Ok(jsonwebtoken::Algorithm::HS256),
        "HS384" => Ok(jsonwebtoken::Algorithm::HS384),
        "HS512" => Ok(jsonwebtoken::Algorithm::HS512),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "HMAC-only in core: HS256, HS384, HS512. Use handler + oxyjwt for asymmetric.",
        )),
    }
}

fn parse_dependencies(
    py: Python<'_>,
    dep_list: &Bound<PyList>,
) -> PyResult<(Vec<String>, Vec<Py<PyAny>>, Vec<bool>)> {
    let inspect = py.import_bound("inspect")?;
    let iscoro = inspect.getattr("iscoroutinefunction")?;
    let n = dep_list.len();
    let mut names = Vec::with_capacity(n);
    let mut facts = Vec::with_capacity(n);
    let mut asy = Vec::with_capacity(n);
    for i in 0..n {
        let it = dep_list.get_item(i)?;
        let tup = it.downcast::<pyo3::types::PyTuple>()?;
        if tup.len() != 2 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "each dependency must be (name, callable)",
            ));
        }
        let name: String = tup.get_item(0)?.extract()?;
        let f: Py<PyAny> = tup.get_item(1)?.unbind();
        let is_a: bool = iscoro.call1((f.clone_ref(py),))?.extract()?;
        names.push(name);
        facts.push(f);
        asy.push(is_a);
    }
    Ok((names, facts, asy))
}

#[pyclass]
pub struct App {
    state: Arc<Mutex<AppState>>,
}

impl App {
    fn openapi_add_path(oa: &mut serde_json::Value, method: &str, path: &str, op_id: &str) {
        if let Some(paths) = oa
            .as_object_mut()
            .and_then(|m| m.get_mut("paths"))
            .and_then(|p| p.as_object_mut())
        {
            let method_lc = method.to_lowercase();
            let path_entry = paths.entry(path).or_insert_with(|| json!({}));
            if let Some(obj) = path_entry.as_object_mut() {
                obj.insert(
                    method_lc,
                    json!({ "summary": op_id, "operationId": op_id, "responses": { "200": { "description": "OK" } } }),
                );
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
            state: Arc::new(Mutex::new(s)),
        }
    }

    /// Paths use **matchit 0.7** style: `/user/:id`. Pass `dependencies=[("x", get_x), ...]`.
    #[pyo3(
        signature = (method, path, handler, require_jwt=false, jwt_secret=None, algorithms=None, read_json_body=true, dependencies=None)
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
        dependencies: Option<Bound<'_, PyList>>,
    ) -> PyResult<()> {
        let st = self.state.lock().map_err(|e| lock_err(e))?;
        if st.frozen {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "app is frozen; no more add_route",
            ));
        }
        drop(st);
        let inspect = py.import_bound("inspect")?;
        let f = inspect.getattr("iscoroutinefunction")?;
        let is_async: bool = f
            .call1((handler.clone_ref(py),))?
            .extract()?;
        let algs: Vec<jsonwebtoken::Algorithm> = if let Some(list) = algorithms {
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
        if require_jwt {
            let need = algs
                .iter()
                .all(|a| {
                    matches!(
                        a,
                        jsonwebtoken::Algorithm::HS256
                            | jsonwebtoken::Algorithm::HS384
                            | jsonwebtoken::Algorithm::HS512
                    )
                });
            if need && jwt_secret.is_none() {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "require_jwt with HMAC needs jwt_secret",
                ));
            }
        }
        let (dep_names, dep_factories, dep_is_async) = if let Some(d) = dependencies {
            parse_dependencies(py, &d)?
        } else {
            (vec![], vec![], vec![])
        };
        let op_id: String = handler
            .bind(py)
            .getattr(pyo3::intern!(py, "__name__"))?
            .extract()?;
        let mut st = self.state.lock().map_err(|e| lock_err(e))?;
        let idx = st.routes.len();
        st.routes.push(state::RouteEntry {
            handler,
            is_async,
            require_jwt,
            jwt_secret,
            algs: algs.clone(),
            read_json_body,
            dep_names,
            dep_factories,
            dep_is_async,
        });
        {
            let mut oa = st.openapi.lock().map_err(|e| lock_err(e))?;
            App::openapi_add_path(&mut oa, &method, &path, &op_id);
        }
        {
            let mut m = state::map_method_router(&*st, &method)
                .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("unsupported HTTP method"))?;
            m.insert(&path, idx)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))?;
        }
        Ok(())
    }

    /// Lock route registration. Linear dependency list is a valid topological order (DAG of independent roots).
    fn freeze(&self) -> PyResult<()> {
        let mut st = self.state.lock().map_err(|e| lock_err(e))?;
        st.frozen = true;
        Ok(())
    }

    fn set_openapi_served(&self, enabled: bool) -> PyResult<()> {
        let mut st = self.state.lock().map_err(|e| lock_err(e))?;
        st.include_openapi = enabled;
        Ok(())
    }

    fn set_openapi_title(&self, title: &str) -> PyResult<()> {
        let st = self.state.lock().map_err(|e| lock_err(e))?;
        let mut oa = st.openapi.lock().map_err(|e| lock_err(e))?;
        if let Some(info) = oa
            .as_object_mut()
            .and_then(|m| m.get_mut("info"))
            .and_then(|i| i.as_object_mut())
        {
            info.insert("title".to_string(), json!(title));
        }
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
        pyo3_asyncio_0_21::tokio::future_into_py(py, async move { run_rsgi(state, scope_py, protocol_py).await })
    }

    fn openapi_json(&self) -> PyResult<String> {
        let st = self.state.lock().map_err(|e| lock_err(e))?;
        let oa = st.openapi.lock().map_err(|e| lock_err(e))?;
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
    m.add_function(wrap_pyfunction!(token::decode_jwt_hs, m)?)?;
    Ok(())
}
