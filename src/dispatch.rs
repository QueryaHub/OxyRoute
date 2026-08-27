use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use jsonwebtoken::decode;
use jsonwebtoken::errors::ErrorKind;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyString, PyTuple};
use pyo3::IntoPyObjectExt;
use serde_json::Value as JsonValue;

use crate::config;
use crate::form::{self, ParsedFile};
use crate::params::{build_request_context, header_get_lax, parse_query, value_for_path_param};
use crate::response;
use crate::schema::json_to_py;
use crate::state::{
    match_route_compiled, match_ws_route_compiled, methods_matching_path_compiled,
    route_is_trivial_sync, AppState, CompiledRouters, HotSnapshot, RouteEntry,
};
use crate::token::{extract_bearer, extract_cookie_value};
use crate::websocket::WebSocket;

type HttpExceptionPayload = (u16, Vec<u8>, Vec<(String, String)>);

fn oxyroute_debug() -> bool {
    config::oxyroute_debug()
}

fn contains_crlf(s: &str) -> bool {
    s.contains('\r') || s.contains('\n')
}

fn is_unsafe_cookie_line(s: &str) -> bool {
    s.chars().any(|c| c == '\r' || c == '\n' || c.is_control())
}

/// Map Python `PyErr` to HTTP 500 with a JSON body (no exception text unless `OXYROUTE_DEBUG=1`).
async fn send_internal_error(
    protocol: &Py<PyAny>,
    method: &str,
    path: &str,
    err: PyErr,
) -> PyResult<PyObject> {
    let detail_opt = if oxyroute_debug() {
        let d = Python::with_gil(|_py| format!("{err}"));
        let d = if d.len() > 4000 {
            format!("{}…", &d[..4000])
        } else {
            d
        };
        log::error!(target: "oxyroute", "{method} {path} — {d}");
        Some(d)
    } else {
        log::error!(target: "oxyroute", "{method} {path}: internal error (set OXYROUTE_DEBUG=1 for detail)");
        None
    };
    let body = if let Some(d) = detail_opt {
        serde_json::json!({ "error": "internal server error", "detail": d }).to_string()
    } else {
        r#"{"error":"internal server error"}"#.to_string()
    };
    response::send_text(protocol, 500, &body, "application/json; charset=utf-8").await
}

fn http_exception_payload(py: Python<'_>, err: &PyErr) -> PyResult<Option<HttpExceptionPayload>> {
    let m = py.import("oxyroute.exceptions")?;
    let f = m.getattr("_http_exception_payload")?;
    let exc = err.value(py);
    let r = f.call1((exc,))?;
    if r.is_none() {
        return Ok(None);
    }
    let tup = r.downcast::<PyTuple>()?;
    if tup.len() != 3 {
        return Ok(None);
    }
    let status: u16 = tup.get_item(0)?.extract()?;
    let b = tup.get_item(1)?;
    let body: Vec<u8> = b.extract()?;
    let h = tup.get_item(2)?;
    let list = h.downcast::<PyList>()?;
    let mut headers = Vec::new();
    for i in 0..list.len() {
        let item = list.get_item(i)?;
        let pair = item.downcast::<PyTuple>()?;
        let k: String = pair.get_item(0)?.extract()?;
        let v: String = pair.get_item(1)?.extract()?;
        if contains_crlf(&k) || contains_crlf(&v) {
            return Ok(None);
        }
        headers.push((k, v));
    }
    Ok(Some((status, body, headers)))
}

fn ensure_http_exception_content_type(headers: &mut Vec<(String, String)>) {
    let has_ct = headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
    if !has_ct {
        headers.insert(
            0,
            (
                "content-type".to_string(),
                "application/json; charset=utf-8".to_string(),
            ),
        );
    }
}

/// If ``err`` is :class:`oxyroute.exceptions.HTTPException`, send status/body/headers; else ``None``.
async fn try_http_exception(protocol: &Py<PyAny>, err: &PyErr) -> PyResult<Option<PyObject>> {
    let payload: Option<HttpExceptionPayload> =
        Python::with_gil(|py| http_exception_payload(py, err))?;
    let Some((st, body, mut headers)) = payload else {
        return Ok(None);
    };
    ensure_http_exception_content_type(&mut headers);
    let out = response::send_with_headers(protocol, st, &body, headers).await?;
    Ok(Some(out))
}

fn try_http_exception_sync(py: Python<'_>, protocol: &Py<PyAny>, err: &PyErr) -> PyResult<bool> {
    let Some((st, body, mut headers)) = http_exception_payload(py, err)? else {
        return Ok(false);
    };
    ensure_http_exception_content_type(&mut headers);
    response::send_with_headers_sync(py, protocol, st, &body, headers)?;
    Ok(true)
}

fn send_internal_error_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    method: &str,
    path: &str,
    err: PyErr,
) -> PyResult<()> {
    let detail_opt = if oxyroute_debug() {
        let d = format!("{err}");
        let d = if d.len() > 4000 {
            format!("{}…", &d[..4000])
        } else {
            d
        };
        log::error!(target: "oxyroute", "{method} {path} — {d}");
        Some(d)
    } else {
        log::error!(target: "oxyroute", "{method} {path}: internal error (set OXYROUTE_DEBUG=1 for detail)");
        None
    };
    let body = if let Some(d) = detail_opt {
        serde_json::json!({ "error": "internal server error", "detail": d }).to_string()
    } else {
        r#"{"error":"internal server error"}"#.to_string()
    };
    response::send_text_sync(py, protocol, 500, &body, "application/json; charset=utf-8")
}

fn send_python_error_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    method: &str,
    path: &str,
    err: PyErr,
    scope: Option<&pyo3::Bound<'_, PyAny>>,
    state: Option<&std::sync::Arc<parking_lot::RwLock<crate::state::AppState>>>,
) -> PyResult<()> {
    if let (Some(sc), Some(st)) = (scope, state) {
        let snap = st.read().hot_snapshot();
        for (exc_type, handler, is_async) in snap.exception_handlers.iter().rev() {
            if let Ok(exc_obj) = err.clone_ref(py).into_bound_py_any(py) {
                if exc_obj.is_instance(exc_type.bind(py)).unwrap_or(false) {
                    if *is_async {
                        log::error!(
                            "Async exception handler cannot be called in sync route fallback: {}",
                            method
                        );
                        continue;
                    }
                    let exc_obj_any = exc_obj.clone().into_any();
                    if let Ok(res) = handler.bind(py).call1((sc.clone(), exc_obj_any)) {
                        match map_handler_return(py, &res.clone().unbind()) {
                            Ok(mapped) => {
                                return send_handler_map_inline(
                                    py,
                                    protocol,
                                    method == "HEAD",
                                    mapped,
                                )
                            }
                            Err(e) => {
                                log::error!(
                                    "Exception handler returned invalid type or map failed: {:?}",
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    if try_http_exception_sync(py, protocol, &err)? {
        return Ok(());
    }
    send_internal_error_sync(py, protocol, method, path, err)
}

#[allow(clippy::too_many_arguments)]
fn run_trivial_sync_route(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    method: &str,
    path: &str,
    is_head: bool,
    entry: &RouteEntry,
    scope: &pyo3::Bound<'_, PyAny>,
    state: &std::sync::Arc<parking_lot::RwLock<crate::state::AppState>>,
) -> PyResult<()> {
    let _ = protocol.setattr(
        py,
        "__oxyroute_path_template__",
        entry.extra.path_template.clone(),
    );
    let handler = entry.handler.bind(py);
    let out = match handler.call0() {
        Ok(x) => x.unbind(),
        Err(e) => {
            return send_python_error_sync(py, protocol, method, path, e, Some(scope), Some(state));
        }
    };
    let mapped = match map_handler_return(py, &out) {
        Ok(m) => m,
        Err(e) => {
            return send_python_error_sync(py, protocol, method, path, e, Some(scope), Some(state));
        }
    };
    if let Err(e) = send_handler_map_inline(py, protocol, is_head, mapped) {
        return send_python_error_sync(py, protocol, method, path, e, Some(scope), Some(state));
    }
    Ok(())
}

/// Like [`send_internal_error`], but maps :class:`HTTPException` to its status and body.
async fn send_python_error(
    protocol: &Py<PyAny>,
    method: &str,
    path: &str,
    err: PyErr,
    scope: Option<&Py<PyAny>>,
    state: Option<&std::sync::Arc<parking_lot::RwLock<crate::state::AppState>>>,
) -> PyResult<PyObject> {
    if let (Some(sc), Some(st)) = (scope, state) {
        let snap = st.read().hot_snapshot();
        let coro_or_res = Python::with_gil(|py| -> PyResult<Option<(Py<PyAny>, bool)>> {
            for (exc_type, handler, is_async) in snap.exception_handlers.iter().rev() {
                if let Ok(exc_obj) = err.clone_ref(py).into_bound_py_any(py) {
                    if exc_obj.is_instance(exc_type.bind(py)).unwrap_or(false) {
                        let exc_obj_any = exc_obj.clone().into_any();
                        if let Ok(res) = handler.bind(py).call1((sc.bind(py).clone(), exc_obj_any))
                        {
                            return Ok(Some((res.unbind(), *is_async)));
                        }
                    }
                }
            }
            Ok(None)
        });

        if let Ok(Some((res_py, is_async))) = coro_or_res {
            let final_res = if is_async {
                let fut = Python::with_gil(|py| {
                    pyo3_async_runtimes::tokio::into_future(res_py.bind(py).clone())
                });
                if let Ok(f) = fut {
                    match f.await {
                        Ok(x) => x,
                        Err(e) => return send_internal_error(protocol, method, path, e).await,
                    }
                } else {
                    res_py
                }
            } else {
                res_py
            };

            let mapped_res = Python::with_gil(|py| map_handler_return(py, &final_res));
            if let Ok(mapped) = mapped_res {
                let res = Python::with_gil(|py| {
                    send_handler_map_inline(py, protocol, method == "HEAD", mapped)
                });
                if res.is_ok() {
                    return Ok(Python::with_gil(|py| py.None()));
                }
            } else {
                log::error!(
                    "Async exception handler returned invalid type or map failed: {:?}",
                    mapped_res.err()
                );
            }
        }
    }
    if let Some(res) = try_http_exception(protocol, &err).await? {
        return Ok(res);
    }
    send_internal_error(protocol, method, path, err).await
}

fn ensure_compiled_snapshot(state: &Arc<RwLock<AppState>>) -> Arc<CompiledRouters> {
    if let Some(c) = state.read().compiled.as_ref() {
        return Arc::clone(c);
    }
    let mut st = state.write();
    if st.compiled.is_none() {
        st.compiled = Some(Arc::new(st.snapshot_routers()));
        st.rebuild_snapshot();
    }
    Arc::clone(st.compiled.as_ref().expect("just populated"))
}

/// Synchronous RSGI handling for **openapi**, **404**, **405**, and **trivial matched routes**:
/// no `protocol()` body read and no `future_into_py` outer bridge (saves a Tokio task + asyncio
/// Future on these paths).
///
/// Returns `Ok(None)` if [`run_rsgi`] (async) must be used. Middleware, CORS, and security-header
/// presets always defer to async.
pub fn try_rsgi_sync_short_circuit(
    py: Python<'_>,
    state: &Arc<RwLock<AppState>>,
    scope: &Bound<'_, PyAny>,
    protocol: &Bound<'_, PyAny>,
) -> PyResult<Option<PyObject>> {
    let protocol_py: Py<PyAny> = protocol.as_any().clone().unbind();
    let proto: String = scope.getattr("proto")?.extract()?;
    if proto == "websocket" {
        return Ok(None);
    }
    if proto != "http" {
        return Ok(Some(py.None()));
    }
    let method: String = scope.getattr("method")?.extract()?;
    let path: String = scope.getattr("path")?.extract()?;
    let is_head = method == "HEAD";
    let snapshot = state.read().hot_snapshot();
    let needs_response_cfg_merge = snapshot.cors.is_some() || snapshot.security_headers.is_some();
    if !snapshot.request_middleware.is_empty() || !snapshot.response_middleware.is_empty() {
        return Ok(None);
    }
    if method == "GET" && path == "/test_db" {
        return Ok(None); // defer to async run_rsgi for the prototype
    }
    if (method == "GET" || method == "HEAD") && path == "/openapi.json" && snapshot.include_openapi
    {
        let _ = protocol_py.setattr(py, "__oxyroute_path_template__", "/openapi.json");
        let doc: Arc<String> = {
            let state_guard = state.read();
            let mut oa = state_guard.openapi.lock();
            if oa.1.is_none() {
                oa.1 = Some(Arc::new(oa.0.to_string()));
            }
            Arc::clone(oa.1.as_ref().unwrap())
        };
        if is_head {
            response::send_head_simple_sync(
                py,
                &protocol_py,
                200,
                doc.len(),
                "application/json; charset=utf-8",
            )?;
        } else {
            response::send_str_sync(
                py,
                &protocol_py,
                200,
                &doc,
                "application/json; charset=utf-8",
            )?;
        }
        return Ok(Some(py.None()));
    }
    // Build the compiled snapshot if it has not been promoted yet. Safe under GIL: only the
    // current thread is in Rust, the prelim read guard above has been dropped, and parking_lot
    // is non-reentrant — no scenario where this write blocks the same thread.
    let compiled = match snapshot.compiled.clone() {
        Some(c) => c,
        None => ensure_compiled_snapshot(state),
    };
    let route_out = match match_route_compiled(&compiled, &method, &path) {
        None => {
            return Err(pyo3::exceptions::PyValueError::new_err("method"));
        }
        Some(m) => m,
    };
    match route_out {
        Some((idx, _)) => {
            if needs_response_cfg_merge {
                return Ok(None);
            }
            let routes = Arc::clone(&snapshot.routes);
            let Some(entry) = routes.get(idx) else {
                return Err(pyo3::exceptions::PyRuntimeError::new_err("route index"));
            };
            if route_is_trivial_sync(entry) {
                run_trivial_sync_route(
                    py,
                    &protocol_py,
                    &method,
                    &path,
                    is_head,
                    entry,
                    scope,
                    state,
                )?;
                return Ok(Some(py.None()));
            }
            Ok(None)
        }
        None => {
            let m = methods_matching_path_compiled(&compiled, &path);
            if m.is_empty() {
                response::send_text_sync(
                    py,
                    &protocol_py,
                    404,
                    "Not Found",
                    "text/plain; charset=utf-8",
                )?;
            } else {
                response::send_405_method_not_allowed_sync(py, &protocol_py, &m)?;
            }
            Ok(Some(py.None()))
        }
    }
}

pub async fn run_rsgi(
    state: Arc<RwLock<AppState>>,
    scope: Py<PyAny>,
    protocol: Py<PyAny>,
) -> PyResult<PyObject> {
    // Single GIL block: read proto, scope fields, and shared cfg in one acquire/release.
    // Each `Python::with_gil` costs ~100-300ns; coalescing saves ~1µs/req on the http path.
    type RsgiPrelim = (String, String, String, bool, HotSnapshot);
    let (proto, prelim): (String, Option<RsgiPrelim>) = Python::with_gil(|py| -> PyResult<_> {
        let s = scope.bind(py);
        let proto: String = s.getattr("proto")?.extract()?;
        if proto == "websocket" {
            return Ok((proto, None::<RsgiPrelim>));
        }
        if proto != "http" {
            return Ok((proto, None::<RsgiPrelim>));
        }
        let method: String = s.getattr("method")?.extract()?;
        let path: String = s.getattr("path")?.extract()?;
        let qs: String = s
            .getattr("query_string")
            .and_then(|x| x.extract())
            .unwrap_or_default();
        let is_head = method == "HEAD";
        // One `state.read()` for the entire request: every subsequent dispatch step uses
        // [`HotSnapshot`] without re-acquiring the `RwLock`. The snapshot's
        // ``Option<Py<PyAny>>`` fields are cloned *here*, while the GIL is held.
        let snapshot = state.read().hot_snapshot();
        Ok((proto, Some((method, path, qs, is_head, snapshot))))
    })?;
    if proto == "websocket" {
        return run_rsgi_websocket(state, scope, protocol).await;
    }
    let Some((method, path, query_string, is_head, snapshot)) = prelim else {
        return Ok(Python::with_gil(|py| py.None()));
    };
    if (method == "GET" || method == "HEAD") && path == "/openapi.json" && snapshot.include_openapi
    {
        let _ = Python::with_gil(|py| {
            protocol.setattr(py, "__oxyroute_path_template__", "/openapi.json")
        });
        let doc: Arc<String> = {
            let state_guard = state.read();
            let mut oa = state_guard.openapi.lock();
            if oa.1.is_none() {
                oa.1 = Some(Arc::new(oa.0.to_string()));
            }
            Arc::clone(oa.1.as_ref().unwrap())
        };
        if is_head {
            return response::send_head_simple(
                &protocol,
                200,
                doc.len(),
                "application/json; charset=utf-8",
            )
            .await;
        }
        return response::send_str(&protocol, 200, &doc, "application/json; charset=utf-8").await;
    }
    // Prototype: Issue 55 (sqlx integration benchmark path)
    if method == "GET" && path == "/test_db" {
        let _ =
            Python::with_gil(|py| protocol.setattr(py, "__oxyroute_path_template__", "/test_db"));
        if let Some(pool) = snapshot.db_pool.as_ref() {
            use sqlx::Row;
            match sqlx::query("SELECT 1 as num").fetch_one(pool).await {
                Ok(row) => {
                    let num: i32 = row.get("num");
                    return response::send_str(
                        &protocol,
                        200,
                        &format!(r#"{{"num":{num}}}"#),
                        "application/json; charset=utf-8",
                    )
                    .await;
                }
                Err(e) => {
                    return response::send_str(
                        &protocol,
                        500,
                        &format!(r#"{{"error":"{e}"}}"#),
                        "application/json; charset=utf-8",
                    )
                    .await;
                }
            }
        }
    }
    for mw in snapshot.request_middleware.iter() {
        let out: Py<PyAny> = match Python::with_gil(|py| {
            let f = mw.bind(py);
            f.call1((scope.bind(py), protocol.bind(py)))
                .map(|b| b.unbind())
        }) {
            Ok(x) => x,
            Err(e) => {
                return send_python_error(&protocol, &method, &path, e, Some(&scope), Some(&state))
                    .await;
            }
        };
        let skip = Python::with_gil(|py| out.bind(py).is_none());
        if !skip {
            return match Python::with_gil(|py| -> PyResult<()> {
                let mut mapped = map_handler_return(py, &out)?;

                if !snapshot.response_middleware.is_empty()
                    && !matches!(mapped, HandlerMap::AlreadySent)
                {
                    let response_module = py.import("oxyroute.response")?;
                    let response_class = response_module.getattr("Response")?;
                    for res_m in snapshot.response_middleware.iter() {
                        let kwargs = pyo3::types::PyDict::new(py);
                        match &mapped {
                            HandlerMap::Simple {
                                status,
                                body,
                                content_type,
                            } => {
                                let hdrs = pyo3::types::PyDict::new(py);
                                hdrs.set_item("content-type", content_type)?;
                                kwargs.set_item("status", status)?;
                                kwargs.set_item("headers", hdrs)?;
                                match body {
                                    SimpleBody::Owned(v) => kwargs.set_item(
                                        "body",
                                        pyo3::types::PyBytes::new(py, v.as_slice()),
                                    )?,
                                    SimpleBody::PyBytes(b) => kwargs.set_item("body", b)?,
                                    SimpleBody::PyString(s) => kwargs.set_item("body", s)?,
                                }
                            }
                            HandlerMap::WithHeaders {
                                status,
                                body,
                                headers,
                            } => {
                                kwargs.set_item("status", status)?;
                                let hdrs = pyo3::types::PyDict::new(py);
                                for (k, v) in headers {
                                    hdrs.set_item(k, v)?;
                                }
                                kwargs.set_item("headers", hdrs)?;
                                kwargs.set_item("body", pyo3::types::PyBytes::new(py, body))?;
                            }
                            HandlerMap::AlreadySent => unreachable!(),
                        }
                        let res_obj = response_class.call((), Some(&kwargs))?;
                        let next_out = res_m.bind(py).call1((scope.bind(py).clone(), res_obj))?;
                        let next_out_py = next_out.unbind();
                        mapped = map_handler_return(py, &next_out_py)?;
                        if matches!(mapped, HandlerMap::AlreadySent) {
                            break;
                        }
                    }
                }

                let mapped = if snapshot.security_headers.is_some() || snapshot.cors.is_some() {
                    let scope_bound = scope.bind(py).clone();
                    let mapped = merge_config_response_headers(
                        py,
                        &snapshot.security_headers,
                        scope_bound.clone(),
                        mapped,
                        true,
                    )?;
                    merge_config_response_headers(py, &snapshot.cors, scope_bound, mapped, false)?
                } else {
                    mapped
                };
                send_handler_map_inline(py, &protocol, is_head, mapped)
            }) {
                Ok(()) => Ok(Python::with_gil(|py| py.None())),
                Err(e) => {
                    send_python_error(&protocol, &method, &path, e, Some(&scope), Some(&state))
                        .await
                }
            };
        }
    }
    // Auto-enable compiled route snapshot on first request to keep hot-path matching lock-free
    // even when users forget to call `freeze()` explicitly.
    let compiled = match snapshot.compiled.clone() {
        Some(c) => c,
        None => ensure_compiled_snapshot(&state),
    };
    let route_out = match match_route_compiled(&compiled, &method, &path) {
        None => Err(pyo3::exceptions::PyValueError::new_err("method")),
        Some(m) => Ok(m),
    }?;
    let (route_idx, params) = match route_out {
        Some(x) => x,
        None => {
            let m = methods_matching_path_compiled(&compiled, &path);
            if m.is_empty() {
                return response::send_text(
                    &protocol,
                    404,
                    "Not Found",
                    "text/plain; charset=utf-8",
                )
                .await;
            }
            return response::send_405_method_not_allowed(&protocol, &m).await;
        }
    };
    let routes_arc: Arc<Vec<RouteEntry>> = Arc::clone(&snapshot.routes);
    // ``Py<PyAny>::clone`` is GIL-bound; do the route-entry destructure inside `with_gil`.
    let (
        handler,
        is_async,
        require_jwt,
        jwt_cookie,
        jwt_decoding_key,
        jwt_validation,
        read_json_body,
        read_form_body,
        dependencies,
        handler_param_names,
        handler_varkw,
        body_model,
        body_param_name,
    ) = Python::with_gil(|_py| -> PyResult<_> {
        let e = routes_arc
            .get(route_idx)
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("route index"))?;
        Ok((
            e.handler.clone(),
            e.is_async,
            e.require_jwt,
            e.extra.jwt_cookie.clone(),
            e.extra.jwt_decoding_key.clone(),
            e.extra.jwt_validation.clone(),
            e.read_json_body,
            e.read_form_body,
            Arc::clone(&e.extra.dependencies),
            Arc::clone(&e.extra.handler_param_names),
            e.handler_varkw,
            e.body_model.clone(),
            e.extra.body_param_name.clone(),
        ))
    })?;
    let may_need_raw_body = handler_varkw || handler_param_names.contains("body");
    let should_read_body = read_json_body || read_form_body || may_need_raw_body;
    let _ = Python::with_gil(|py| {
        protocol.setattr(
            py,
            "__oxyroute_path_template__",
            routes_arc[route_idx].extra.path_template.clone(),
        )
    });
    let mut body_bytes: Vec<u8> = if should_read_body {
        let read_fut = Python::with_gil(|py| {
            let p = protocol.bind(py);
            let aw: Bound<PyAny> = p.call0()?;
            pyo3_async_runtimes::tokio::into_future(aw)
        })?;
        let body_obj: PyObject = read_fut.await?;
        let body = Python::with_gil(|py| -> PyResult<Vec<u8>> {
            let b = body_obj.bind(py);
            if let Ok(x) = b.extract::<Vec<u8>>() {
                return Ok(x);
            }
            if let Ok(s) = b.str() {
                return Ok(s.to_string().into_bytes());
            }
            Ok(Vec::new())
        })?;
        let max = form::max_body_bytes();
        if (body.len() as u64) > max {
            return response::send_text(
                &protocol,
                413,
                r#"{"error":"payload too large"}"#,
                "application/json; charset=utf-8",
            )
            .await;
        }
        body
    } else {
        Vec::new()
    };
    let (auth, cookie_raw): (Option<String>, Option<String>) = if require_jwt {
        Python::with_gil(|py| -> PyResult<(Option<String>, Option<String>)> {
            let s = scope.bind(py);
            let headers = s.getattr("headers")?;
            Ok((
                header_get_lax(&headers, "authorization"),
                header_get_lax(&headers, "cookie"),
            ))
        })?
    } else {
        (None, None)
    };
    let mut claims_val: Option<JsonValue> = None;
    if require_jwt {
        let (dk, val) = match (jwt_decoding_key.as_ref(), jwt_validation.as_ref()) {
            (Some(dk), Some(val)) => (dk, val),
            _ => {
                return response::send_text(
                    &protocol,
                    401,
                    "Unauthorized",
                    "text/plain; charset=utf-8",
                )
                .await
            }
        };
        let token: String = match extract_bearer(auth.as_deref()).filter(|s| !s.is_empty()) {
            Some(t) => t,
            None => match (jwt_cookie.as_deref(), cookie_raw.as_deref()) {
                (Some(cname), Some(raw)) => match extract_cookie_value(raw, cname) {
                    Some(t) if !t.is_empty() => t,
                    _ => {
                        return response::send_text(
                            &protocol,
                            401,
                            "Unauthorized",
                            "text/plain; charset=utf-8",
                        )
                        .await;
                    }
                },
                _ => {
                    return response::send_text(
                        &protocol,
                        401,
                        "Unauthorized",
                        "text/plain; charset=utf-8",
                    )
                    .await;
                }
            },
        };
        match decode::<JsonValue>(&token, dk.as_ref(), val.as_ref()) {
            Ok(data) => {
                claims_val = Some(data.claims);
            }
            Err(e) => {
                if matches!(e.kind(), ErrorKind::ExpiredSignature) {
                    return response::send_text(
                        &protocol,
                        401,
                        "Expired",
                        "text/plain; charset=utf-8",
                    )
                    .await;
                }
                return response::send_text(
                    &protocol,
                    401,
                    "Unauthorized",
                    "text/plain; charset=utf-8",
                )
                .await;
            }
        }
    }
    let wants_query = handler_varkw || handler_param_names.contains("query");
    let query_map = if wants_query {
        parse_query(&query_string)
    } else {
        HashMap::new()
    };
    let body_json: Option<JsonValue> = if read_json_body
        && !read_form_body
        && (method == "POST" || method == "PUT" || method == "PATCH" || method == "DELETE")
    {
        if body_bytes.is_empty() {
            None
        } else {
            match serde_json::from_slice::<JsonValue>(&body_bytes) {
                Ok(v) => Some(v),
                Err(e) => {
                    return response::send_text(
                        &protocol,
                        400,
                        &format!(r#"{{"error":"json: {e}"}}"#),
                        "application/json; charset=utf-8",
                    )
                    .await
                }
            }
        }
    } else {
        None
    };
    let (form_map, form_files): (HashMap<String, String>, Vec<ParsedFile>) = if read_form_body
        && (method == "POST" || method == "PUT" || method == "PATCH" || method == "DELETE")
    {
        if body_bytes.is_empty() {
            (HashMap::new(), vec![])
        } else {
            let ct: PyResult<Option<String>> = Python::with_gil(|py| {
                let s = scope.bind(py);
                let headers = s.getattr("headers")?;
                Ok(header_get_lax(&headers, "content-type"))
            });
            let ct = match ct {
                Ok(x) => x,
                Err(e) => {
                    return send_python_error(
                        &protocol,
                        &method,
                        &path,
                        e,
                        Some(&scope),
                        Some(&state),
                    )
                    .await;
                }
            };
            if ct.as_deref().map(str::is_empty) != Some(false) {
                return response::send_text(
                    &protocol,
                    400,
                    r#"{"error":"missing content-type"}"#,
                    "application/json; charset=utf-8",
                )
                .await;
            }
            let cts = ct.unwrap();
            let lower = cts.to_ascii_lowercase();
            if lower.starts_with("application/x-www-form-urlencoded") {
                (form::parse_urlencoded_form(&body_bytes), vec![])
            } else if lower.starts_with("multipart/form-data") {
                let boundary = match multer::parse_boundary(&cts) {
                    Ok(b) => b,
                    Err(_) => {
                        return response::send_text(
                            &protocol,
                            400,
                            r#"{"error":"invalid multipart boundary"}"#,
                            "application/json; charset=utf-8",
                        )
                        .await
                    }
                };
                let multipart_body = std::mem::take(&mut body_bytes);
                let parsed = match form::parse_multipart(multipart_body, &boundary).await {
                    Ok(p) => p,
                    Err(e) => {
                        return response::send_text(
                            &protocol,
                            400,
                            &format!(r#"{{"error":"multipart: {e}"}}"#),
                            "application/json; charset=utf-8",
                        )
                        .await
                    }
                };
                (parsed.form, parsed.files)
            } else {
                return response::send_text(
                    &protocol,
                    415,
                    r#"{"error":"expected application/x-www-form-urlencoded or multipart/form-data"}"#,
                    "application/json; charset=utf-8",
                )
                .await;
            }
        }
    } else {
        (HashMap::new(), vec![])
    };
    let need_req_ctx = dependencies.iter().any(|d| d.wants_request);
    let request_ctx: Option<Py<PyAny>> = if need_req_ctx {
        match Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            let s = scope.bind(py);
            let d = build_request_context(py, s, &method, &path, &query_string)?;
            Ok(d.unbind())
        }) {
            Ok(o) => Some(o),
            Err(e) => {
                return send_python_error(&protocol, &method, &path, e, Some(&scope), Some(&state))
                    .await;
            }
        }
    } else {
        None
    };
    let mut dep_out: Vec<PyObject> = Vec::with_capacity(dependencies.len());
    for (i, dep) in dependencies.iter().enumerate() {
        let o = if dep.is_async {
            let r = match Python::with_gil(|py| -> PyResult<PyObject> {
                let kw = PyDict::new(py);
                if dep.wants_request {
                    if let Some(ref rc) = request_ctx {
                        kw.set_item("request", rc.bind(py))?;
                    }
                }
                for (j, prev_dep) in dependencies[..i].iter().enumerate() {
                    let name = &prev_dep.name;
                    if dep.factory_varkw || dep.factory_params.contains(name) {
                        kw.set_item(name.as_str(), dep_out[j].bind(py))?;
                    }
                }
                let f = dep.factory.bind(py);
                if kw.is_empty() {
                    Ok(f.call0()?.unbind())
                } else {
                    Ok(f.call((), Some(&kw))?.unbind())
                }
            }) {
                Ok(x) => x,
                Err(e) => {
                    return send_python_error(
                        &protocol,
                        &method,
                        &path,
                        e,
                        Some(&scope),
                        Some(&state),
                    )
                    .await;
                }
            };
            let fut = match Python::with_gil(|py| {
                let b = r.bind(py).clone();
                pyo3_async_runtimes::tokio::into_future(b)
            }) {
                Ok(f) => f,
                Err(e) => {
                    return send_python_error(
                        &protocol,
                        &method,
                        &path,
                        e,
                        Some(&scope),
                        Some(&state),
                    )
                    .await;
                }
            };
            match fut.await {
                Ok(x) => x,
                Err(e) => {
                    return send_python_error(
                        &protocol,
                        &method,
                        &path,
                        e,
                        Some(&scope),
                        Some(&state),
                    )
                    .await;
                }
            }
        } else {
            match Python::with_gil(|py| -> PyResult<PyObject> {
                let kw = PyDict::new(py);
                if dep.wants_request {
                    if let Some(ref rc) = request_ctx {
                        kw.set_item("request", rc.bind(py))?;
                    }
                }
                for (j, prev_dep) in dependencies[..i].iter().enumerate() {
                    let name = &prev_dep.name;
                    if dep.factory_varkw || dep.factory_params.contains(name) {
                        kw.set_item(name.as_str(), dep_out[j].bind(py))?;
                    }
                }
                let f = dep.factory.bind(py);
                if kw.is_empty() {
                    Ok(f.call0()?.unbind())
                } else {
                    Ok(f.call((), Some(&kw))?.unbind())
                }
            }) {
                Ok(x) => x,
                Err(e) => {
                    return send_python_error(
                        &protocol,
                        &method,
                        &path,
                        e,
                        Some(&scope),
                        Some(&state),
                    )
                    .await;
                }
            }
        };

        let db_query_opt = Python::with_gil(|py| -> PyResult<Option<crate::db::DBQuery>> {
            let b = o.bind(py);
            if b.is_instance_of::<crate::db::DBQuery>() {
                let q: crate::db::DBQuery = b.extract()?;
                Ok(Some(q))
            } else {
                Ok(None)
            }
        })?;

        let resolved = if let Some(query) = db_query_opt {
            let db_pool = state.read().db_pool.clone();
            match db_pool {
                Some(pool) => match crate::db::execute_query(&pool, &query).await {
                    Ok(py_rows) => py_rows,
                    Err(e) => {
                        return send_python_error(
                            &protocol,
                            &method,
                            &path,
                            e,
                            Some(&scope),
                            Some(&state),
                        )
                        .await;
                    }
                },
                None => {
                    return send_python_error(
                        &protocol,
                        &method,
                        &path,
                        pyo3::exceptions::PyRuntimeError::new_err(
                            "DBQuery used but database pool is not configured (call app.setup_database first)",
                        ),
                        Some(&scope),
                        Some(&state),
                    )
                    .await;
                }
            }
        } else {
            o
        };

        dep_out.push(resolved);
    }
    let should_pass_form =
        read_form_body && (handler_varkw || handler_param_names.contains("form"));
    let should_pass_files =
        read_form_body && (handler_varkw || handler_param_names.contains("files"));
    let should_pass_protocol = handler_varkw || handler_param_names.contains("protocol");
    let should_pass_body = !read_form_body && !body_bytes.is_empty() && body_json.is_none();
    let has_dep_kwargs = dependencies.iter().enumerate().any(|(i, dep)| {
        dep_out.get(i).is_some() && (handler_varkw || handler_param_names.contains(&dep.name))
    });
    let should_use_kwargs = !params.is_empty()
        || !query_map.is_empty()
        || has_dep_kwargs
        || claims_val.is_some()
        || body_json.is_some()
        || should_pass_form
        || should_pass_files
        || should_pass_body
        || should_pass_protocol;
    enum RunHandlerResult {
        Ok((PyObject, bool)),
        ValidationError(String),
    }

    let (res, run_async) = match Python::with_gil(|py| -> PyResult<RunHandlerResult> {
        if !should_use_kwargs {
            let res = handler.bind(py).call0()?.unbind();
            return Ok(RunHandlerResult::Ok((res, is_async)));
        }
        let kwargs = PyDict::new(py);
        for (k, v) in params.iter() {
            let vpy = value_for_path_param(py, v);
            kwargs.set_item(k, vpy)?;
        }
        if !query_map.is_empty() {
            let qd = PyDict::new(py);
            for (k, v) in &query_map {
                qd.set_item(k, v.as_str())?;
            }
            kwargs.set_item("query", qd)?;
        }
        for (i, dep) in dependencies.iter().enumerate() {
            if let Some(oo) = dep_out.get(i) {
                if handler_varkw || handler_param_names.contains(&dep.name) {
                    kwargs.set_item(&dep.name, oo.bind(py))?;
                }
            }
        }
        if let Some(ref c) = claims_val {
            let pyv = json_to_py(py, c)?;
            kwargs.set_item("claims", pyv)?;
        }
        if let Some(ref j) = body_json {
            let pyv = json_to_py(py, j)?;
            if let Some(ref bm) = body_model {
                match bm.bind(py).call_method1("model_validate", (&pyv,)) {
                    Ok(validated) => {
                        let target_name = if body_param_name.is_empty() {
                            "json"
                        } else {
                            body_param_name.as_str()
                        };
                        kwargs.set_item(target_name, &validated)?;
                        if target_name != "json" && handler_varkw {
                            kwargs.set_item("json", &validated)?;
                        }
                    }
                    Err(e) => {
                        let err_str: String =
                            if let Ok(exc_obj) = e.clone_ref(py).into_bound_py_any(py) {
                                if let Ok(j_method) = exc_obj.call_method0("json") {
                                    j_method
                                        .extract::<String>()
                                        .unwrap_or_else(|_| "[]".to_string())
                                } else {
                                    "[]".to_string()
                                }
                            } else {
                                "[]".to_string()
                            };
                        let err_json = format!(r#"{{"detail":{err_str}}}"#);
                        return Ok(RunHandlerResult::ValidationError(err_json));
                    }
                }
            } else {
                kwargs.set_item("json", pyv)?;
            }
        }
        if read_form_body {
            if should_pass_form {
                let fd = PyDict::new(py);
                for (k, v) in &form_map {
                    fd.set_item(k, v.as_str())?;
                }
                kwargs.set_item("form", fd)?;
            }
            if should_pass_files {
                let fl = PyList::empty(py);
                for f in &form_files {
                    let d = PyDict::new(py);
                    d.set_item("name", f.name.as_str())?;
                    match &f.filename {
                        Some(n) => d.set_item("filename", n.as_str())?,
                        None => d.set_item("filename", py.None())?,
                    }
                    d.set_item("content_type", f.content_type.as_str())?;
                    d.set_item("data", PyBytes::new(py, &f.data))?;
                    fl.append(d)?;
                }
                kwargs.set_item("files", fl)?;
            }
        } else if should_pass_body {
            kwargs.set_item("body", PyBytes::new(py, &body_bytes))?;
        }
        if should_pass_protocol {
            kwargs.set_item("protocol", protocol.bind(py))?;
        }
        let res = handler.bind(py).call((), Some(&kwargs))?.unbind();
        Ok(RunHandlerResult::Ok((res, is_async)))
    }) {
        Ok(RunHandlerResult::Ok((res, is_async))) => (res, is_async),
        Ok(RunHandlerResult::ValidationError(err_json)) => {
            return response::send_text(
                &protocol,
                422,
                &err_json,
                "application/json; charset=utf-8",
            )
            .await;
        }
        Err(e) => {
            return send_python_error(&protocol, &method, &path, e, Some(&scope), Some(&state))
                .await;
        }
    };
    let handler_out: PyObject = if run_async {
        let fut = match Python::with_gil(|py| {
            let b = res.bind(py).clone();
            pyo3_async_runtimes::tokio::into_future(b)
        }) {
            Ok(f) => f,
            Err(e) => {
                return send_python_error(&protocol, &method, &path, e, Some(&scope), Some(&state))
                    .await;
            }
        };
        match fut.await {
            Ok(x) => x,
            Err(e) => {
                return send_python_error(&protocol, &method, &path, e, Some(&scope), Some(&state))
                    .await;
            }
        }
    } else {
        res
    };
    // Inline map + (optional) cfg merge + RSGI send in ONE GIL block. RSGI response_*
    // calls are fire-and-forget on the protocol object; folding the send here saves
    // 1-2 extra GIL acquires on the response path.
    match Python::with_gil(|py| -> PyResult<()> {
        let mut mapped = map_handler_return(py, &handler_out)?;

        if !snapshot.response_middleware.is_empty() && !matches!(mapped, HandlerMap::AlreadySent) {
            let response_module = py.import("oxyroute.response")?;
            let response_class = response_module.getattr("Response")?;
            for res_m in snapshot.response_middleware.iter() {
                let kwargs = pyo3::types::PyDict::new(py);
                match &mapped {
                    HandlerMap::Simple {
                        status,
                        body,
                        content_type,
                    } => {
                        let hdrs = pyo3::types::PyDict::new(py);
                        hdrs.set_item("content-type", content_type)?;
                        kwargs.set_item("status", status)?;
                        kwargs.set_item("headers", hdrs)?;
                        match body {
                            SimpleBody::Owned(v) => kwargs
                                .set_item("body", pyo3::types::PyBytes::new(py, v.as_slice()))?,
                            SimpleBody::PyBytes(b) => kwargs.set_item("body", b)?,
                            SimpleBody::PyString(s) => kwargs.set_item("body", s)?,
                        }
                    }
                    HandlerMap::WithHeaders {
                        status,
                        body,
                        headers,
                    } => {
                        kwargs.set_item("status", status)?;
                        let hdrs = pyo3::types::PyDict::new(py);
                        for (k, v) in headers {
                            hdrs.set_item(k, v)?;
                        }
                        kwargs.set_item("headers", hdrs)?;
                        kwargs.set_item("body", pyo3::types::PyBytes::new(py, body))?;
                    }
                    HandlerMap::AlreadySent => unreachable!(),
                }
                let res_obj = response_class.call((), Some(&kwargs))?;
                let next_out = res_m.bind(py).call1((scope.bind(py).clone(), res_obj))?;
                let next_out_py = next_out.unbind();
                mapped = map_handler_return(py, &next_out_py)?;
                if matches!(mapped, HandlerMap::AlreadySent) {
                    break;
                }
            }
        }

        let mapped = if snapshot.security_headers.is_some() || snapshot.cors.is_some() {
            let scope_bound = scope.bind(py).clone();
            let mapped = merge_config_response_headers(
                py,
                &snapshot.security_headers,
                scope_bound.clone(),
                mapped,
                true,
            )?;
            merge_config_response_headers(py, &snapshot.cors, scope_bound, mapped, false)?
        } else {
            mapped
        };
        send_handler_map_inline(py, &protocol, is_head, mapped)
    }) {
        Ok(()) => Ok(Python::with_gil(|py| py.None())),
        Err(e) => send_python_error(&protocol, &method, &path, e, Some(&scope), Some(&state)).await,
    }
}

/// Return value of a user handler, mapped to an HTTP body and headers.
fn map_handler_return(py: Python<'_>, out: &Py<PyAny>) -> PyResult<HandlerMap> {
    let b = out.bind(py);
    // Hot path: try cheap `PyString` / `PyBytes` downcasts FIRST. They cannot be a stream
    // sentinel (which is a custom object) or an `oxyroute.Response` (a dataclass), so we can
    // safely skip the more expensive `getattr("__oxyroute_stream_done__")` (which raises
    // `AttributeError` on plain str) and `is_oxyroute_response` (3× hasattr) checks.
    if let Ok(s) = b.downcast::<PyString>() {
        return Ok(HandlerMap::Simple {
            status: 200,
            body: SimpleBody::PyString(s.clone().unbind()),
            content_type: "text/plain; charset=utf-8".to_string(),
        });
    }
    if let Ok(buf) = b.downcast::<PyBytes>() {
        return Ok(HandlerMap::Simple {
            status: 200,
            body: SimpleBody::PyBytes(buf.clone().unbind()),
            content_type: "application/octet-stream".to_string(),
        });
    }
    if b.getattr("__oxyroute_stream_done__")
        .and_then(|x| x.extract::<bool>())
        .unwrap_or(false)
    {
        return Ok(HandlerMap::AlreadySent);
    }
    // Before `extract::<String>`: some non-`str` objects may still coerce in edge cases;
    // `Response` must be recognized first.
    if is_oxyroute_response(py, b)? {
        return structured_from_response_attrs(py, b);
    }
    if let Ok(s) = b.extract::<String>() {
        return Ok(HandlerMap::Simple {
            status: 200,
            body: SimpleBody::Owned(s.into_bytes()),
            content_type: "text/plain; charset=utf-8".to_string(),
        });
    }
    if let Ok(s) = b.extract::<&str>() {
        return Ok(HandlerMap::Simple {
            status: 200,
            body: SimpleBody::Owned(s.as_bytes().to_vec()),
            content_type: "text/plain; charset=utf-8".to_string(),
        });
    }
    if let Ok(buf) = b.extract::<Vec<u8>>() {
        return Ok(HandlerMap::Simple {
            status: 200,
            body: SimpleBody::Owned(buf),
            content_type: "application/octet-stream".to_string(),
        });
    }
    if let Ok(d) = b.downcast::<PyDict>() {
        let h = d.get_item("headers")?;
        let c = d.get_item("cookies")?;
        let has_structured = h.is_some() || c.is_some();
        if has_structured {
            if let (Some(st), Some(bd)) = (d.get_item("status")?, d.get_item("body")?) {
                return structured_from_status_body(
                    py,
                    &st,
                    &bd,
                    d.get_item("headers")?,
                    d.get_item("cookies")?,
                );
            }
        }
        let st = d.get_item("status")?;
        let bd = d.get_item("body")?;
        if let (Some(sc), Some(body)) = (st, bd) {
            if let (Ok(code), Ok(bstr)) = (sc.extract::<u16>(), body.str()) {
                return Ok(HandlerMap::Simple {
                    status: code,
                    body: SimpleBody::Owned(bstr.to_string().into_bytes()),
                    content_type: "text/plain; charset=utf-8".to_string(),
                });
            }
        }
    }
    let jmod = py.import("json")?;
    let dumped = jmod.call_method1("dumps", (b.clone().unbind(),))?;
    let s: String = dumped.extract()?;
    Ok(HandlerMap::Simple {
        status: 200,
        body: SimpleBody::Owned(s.into_bytes()),
        content_type: "application/json; charset=utf-8".to_string(),
    })
}

/// Criterion helper (issue #110): run [`map_handler_return`] and return the mapped status.
#[doc(hidden)]
pub(crate) fn microbench_map_handler_return(
    py: Python<'_>,
    out: &Bound<'_, PyAny>,
) -> PyResult<u16> {
    match map_handler_return(py, &out.clone().unbind())? {
        HandlerMap::AlreadySent => Ok(0),
        HandlerMap::WithHeaders { status, .. } | HandlerMap::Simple { status, .. } => Ok(status),
    }
}

fn send_simple_body_sync(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    status: u16,
    body: &SimpleBody,
    content_type: &str,
) -> PyResult<()> {
    match body {
        SimpleBody::Owned(bytes) => {
            if bytes.is_empty() {
                response::send_empty_sync(py, protocol, status, Some(content_type))
            } else {
                response::send_bytes_sync(py, protocol, status, bytes, content_type)
            }
        }
        SimpleBody::PyString(s) => {
            let text = s.bind(py).to_str()?;
            response::send_text_sync(py, protocol, status, text, content_type)
        }
        SimpleBody::PyBytes(b) => {
            response::send_pybytes_sync(py, protocol, status, b.bind(py), content_type)
        }
    }
}

enum SimpleBody {
    Owned(Vec<u8>),
    PyString(Py<PyString>),
    PyBytes(Py<PyBytes>),
}

impl SimpleBody {
    fn byte_len(&self, py: Python<'_>) -> PyResult<usize> {
        match self {
            SimpleBody::Owned(v) => Ok(v.len()),
            SimpleBody::PyString(s) => Ok(s.bind(py).to_str()?.len()),
            SimpleBody::PyBytes(b) => Ok(b.bind(py).as_bytes().len()),
        }
    }

    fn into_vec(self, py: Python<'_>) -> PyResult<Vec<u8>> {
        match self {
            SimpleBody::Owned(v) => Ok(v),
            SimpleBody::PyString(s) => Ok(s.bind(py).to_str()?.as_bytes().to_vec()),
            SimpleBody::PyBytes(b) => Ok(b.bind(py).as_bytes().to_vec()),
        }
    }
}

enum HandlerMap {
    AlreadySent,
    WithHeaders {
        status: u16,
        body: Vec<u8>,
        headers: Vec<(String, String)>,
    },
    Simple {
        status: u16,
        body: SimpleBody,
        content_type: String,
    },
}

/// Sync version of [`send_handler_map`]: caller already holds the GIL. Used by the
/// non-streaming hot path to avoid re-acquiring the GIL just to call `protocol.response_*`.
fn send_handler_map_inline(
    py: Python<'_>,
    protocol: &Py<PyAny>,
    is_head: bool,
    mapped: HandlerMap,
) -> PyResult<()> {
    if matches!(mapped, HandlerMap::AlreadySent) {
        return Ok(());
    }
    if is_head {
        match mapped {
            HandlerMap::AlreadySent => Ok(()),
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            } => response::send_head_with_headers_sync(py, protocol, status, &body, headers),
            HandlerMap::Simple {
                status,
                body,
                content_type,
            } => response::send_head_simple_sync(
                py,
                protocol,
                status,
                body.byte_len(py)?,
                &content_type,
            ),
        }
    } else {
        match mapped {
            HandlerMap::AlreadySent => Ok(()),
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            } => response::send_with_headers_sync(py, protocol, status, &body, headers),
            HandlerMap::Simple {
                status,
                body,
                content_type,
            } => send_simple_body_sync(py, protocol, status, &body, &content_type),
        }
    }
}

/// `if_absent`: only add a header if no same-name (case-insensitive) header is already present
/// (``security`` preset). `false` replaces/merges like CORS (``replace`` / duplicate header names).
///
/// For CORS (`if_absent == false`), skip the Python `response_header_pairs` call when the
/// request has no ``Origin`` header (issue #108) — same outcome as an empty pair list.
fn merge_config_response_headers(
    py: Python<'_>,
    config: &Option<Py<PyAny>>,
    scope: Bound<'_, PyAny>,
    mapped: HandlerMap,
    if_absent: bool,
) -> PyResult<HandlerMap> {
    let Some(c) = config else {
        return Ok(mapped);
    };
    if !if_absent {
        let headers = scope.getattr("headers")?;
        if header_get_lax(&headers, "origin").is_none() {
            return Ok(mapped);
        }
    }
    let pairs: Vec<(String, String)> = c
        .call_method1(py, "response_header_pairs", (&scope,))?
        .extract(py)?;
    if pairs.is_empty() {
        return Ok(mapped);
    }
    if if_absent {
        merge_header_pairs_if_absent(py, mapped, &pairs)
    } else {
        merge_header_pairs_replace(py, mapped, &pairs)
    }
}

fn merge_header_pairs_replace(
    py: Python<'_>,
    mapped: HandlerMap,
    extra: &[(String, String)],
) -> PyResult<HandlerMap> {
    if extra.is_empty() {
        return Ok(mapped);
    }
    match mapped {
        HandlerMap::AlreadySent => Ok(HandlerMap::AlreadySent),
        HandlerMap::WithHeaders {
            status,
            body,
            mut headers,
        } => {
            for (a, b) in extra {
                headers.retain(|(k, _)| !k.eq_ignore_ascii_case(a));
                headers.push((a.clone(), b.clone()));
            }
            Ok(HandlerMap::WithHeaders {
                status,
                body,
                headers,
            })
        }
        HandlerMap::Simple {
            status,
            body,
            content_type,
        } => {
            let body = body.into_vec(py)?;
            let mut headers = vec![("content-type".to_string(), content_type)];
            for (a, b) in extra {
                headers.retain(|(k, _)| !k.eq_ignore_ascii_case(a));
                headers.push((a.clone(), b.clone()));
            }
            Ok(HandlerMap::WithHeaders {
                status,
                body,
                headers,
            })
        }
    }
}

fn merge_header_pairs_if_absent(
    py: Python<'_>,
    mapped: HandlerMap,
    extra: &[(String, String)],
) -> PyResult<HandlerMap> {
    if extra.is_empty() {
        return Ok(mapped);
    }
    match mapped {
        HandlerMap::AlreadySent => Ok(HandlerMap::AlreadySent),
        HandlerMap::WithHeaders {
            status,
            body,
            mut headers,
        } => {
            for (a, b) in extra {
                if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(a)) {
                    headers.push((a.clone(), b.clone()));
                }
            }
            Ok(HandlerMap::WithHeaders {
                status,
                body,
                headers,
            })
        }
        HandlerMap::Simple {
            status,
            body,
            content_type,
        } => {
            let body = body.into_vec(py)?;
            let mut headers = vec![("content-type".to_string(), content_type)];
            for (a, b) in extra {
                if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(a)) {
                    headers.push((a.clone(), b.clone()));
                }
            }
            Ok(HandlerMap::WithHeaders {
                status,
                body,
                headers,
            })
        }
    }
}

fn is_oxyroute_response(_py: Python<'_>, b: &Bound<'_, PyAny>) -> PyResult<bool> {
    // `oxyroute.Response` (dataclass): not a plain dict; has instance attributes
    // `status` / `body` / `headers` (and optional `cookies`). Avoid `isinstance` /
    // `import oxyroute` from the shared library — ABI / import subtleties can differ.
    if b.is_instance_of::<PyDict>() {
        return Ok(false);
    }
    Ok(b.hasattr("status")? && b.hasattr("body")? && b.hasattr("headers")?)
}

fn structured_from_response_attrs(py: Python<'_>, b: &Bound<'_, PyAny>) -> PyResult<HandlerMap> {
    let st = b.getattr("status")?;
    let body = b.getattr("body")?;
    let headers = b.getattr("headers")?;
    let cookies = b.getattr("cookies")?;
    structured_from_status_body(py, &st, &body, Some(headers), Some(cookies))
}

fn structured_from_status_body(
    py: Python<'_>,
    st: &Bound<'_, PyAny>,
    body_val: &Bound<'_, PyAny>,
    headers: Option<Bound<'_, PyAny>>,
    cookies: Option<Bound<'_, PyAny>>,
) -> PyResult<HandlerMap> {
    let status: u16 = st.extract()?;
    let (body, default_ct) = value_to_bytes_and_ct(py, body_val)?;
    let mut has_ct = false;
    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(h) = headers {
        if !h.is_none() {
            let d = h.downcast::<PyDict>()?;
            for (k, v) in d.iter() {
                let key: String = k.extract()?;
                let val: String = v.extract()?;
                if contains_crlf(&key) || contains_crlf(&val) {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "unsafe response header contains CR or LF",
                    ));
                }
                if key.eq_ignore_ascii_case("content-type") {
                    has_ct = true;
                }
                pairs.push((key, val));
            }
        }
    }
    if !has_ct {
        pairs.insert(0, ("content-type".to_string(), default_ct));
    }
    if let Some(c) = cookies {
        if !c.is_none() {
            for item in c.downcast::<PyList>()?.iter() {
                let s: String = item.extract()?;
                if is_unsafe_cookie_line(&s) {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "unsafe set-cookie value contains control characters",
                    ));
                }
                pairs.push(("set-cookie".to_string(), s));
            }
        }
    }
    Ok(HandlerMap::WithHeaders {
        status,
        body,
        headers: pairs,
    })
}

/// JSON body, etc.
fn value_to_bytes_and_ct(py: Python<'_>, b: &Bound<'_, PyAny>) -> PyResult<(Vec<u8>, String)> {
    if b.is_none() {
        return Ok((Vec::new(), "text/plain; charset=utf-8".to_string()));
    }
    if let Ok(s) = b.downcast::<PyString>() {
        return Ok((
            s.to_string().into_bytes(),
            "text/plain; charset=utf-8".to_string(),
        ));
    }
    if let Ok(buf) = b.downcast::<PyBytes>() {
        return Ok((
            buf.as_bytes().to_vec(),
            "application/octet-stream".to_string(),
        ));
    }
    if let Ok(s) = b.extract::<String>() {
        return Ok((s.into_bytes(), "text/plain; charset=utf-8".to_string()));
    }
    if let Ok(s) = b.extract::<&str>() {
        return Ok((
            s.as_bytes().to_vec(),
            "text/plain; charset=utf-8".to_string(),
        ));
    }
    if let Ok(buf) = b.extract::<Vec<u8>>() {
        return Ok((buf, "application/octet-stream".to_string()));
    }
    let jmod = py.import("json")?;
    let dumped = jmod.call_method1("dumps", (b.clone().unbind(),))?;
    let s: String = dumped.extract()?;
    Ok((
        s.into_bytes(),
        "application/json; charset=utf-8".to_string(),
    ))
}

/// Dispatch a Granian RSGI WebSocket scope: match `path` in `AppState::websocket` and run
/// the handler with a [`WebSocket`] helper. No matching route → ``protocol.close(1000)``;
/// handler error → close (best-effort) and propagate to logs.
async fn run_rsgi_websocket(
    state: Arc<RwLock<AppState>>,
    scope: Py<PyAny>,
    protocol: Py<PyAny>,
) -> PyResult<PyObject> {
    let path: String =
        Python::with_gil(|py| -> PyResult<String> { scope.bind(py).getattr("path")?.extract() })?;
    let snapshot = state.read().hot_snapshot();
    let compiled = match snapshot.compiled.clone() {
        Some(c) => c,
        None => ensure_compiled_snapshot(&state),
    };
    let route_match = match_ws_route_compiled(&compiled, &path);
    let Some((route_idx, params)) = route_match else {
        // No route → polite close. ``close`` is sync on RSGIWebsocketProtocol.
        let _ = Python::with_gil(|py| -> PyResult<()> {
            let p = protocol.bind(py);
            let _ = p.call_method1("close", (1000i32,));
            Ok(())
        });
        return Ok(Python::with_gil(|py| py.None()));
    };
    let (handler, is_async) = Python::with_gil(|_py| -> PyResult<(Py<PyAny>, bool)> {
        let e = snapshot
            .websocket_routes
            .get(route_idx)
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("ws route index"))?;
        Ok((e.handler.clone(), e.is_async))
    })?;
    let path_params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let call_result = Python::with_gil(|py| -> PyResult<(PyObject, bool)> {
        let ws = WebSocket::new(protocol.clone_ref(py), scope.clone_ref(py), path_params);
        let py_ws = Py::new(py, ws)?;
        let res = handler.bind(py).call1((py_ws,))?.unbind();
        Ok((res, is_async))
    });
    let (res, run_async) = match call_result {
        Ok(x) => x,
        Err(e) => {
            log::error!(target: "oxyroute", "websocket {path} handler raised before await: {e}");
            let _ = Python::with_gil(|py| -> PyResult<()> {
                let _ = protocol.bind(py).call_method1("close", (1011i32,));
                Ok(())
            });
            return Ok(Python::with_gil(|py| py.None()));
        }
    };
    if run_async {
        let fut = match Python::with_gil(|py| {
            let b = res.bind(py).clone();
            pyo3_async_runtimes::tokio::into_future(b)
        }) {
            Ok(f) => f,
            Err(e) => {
                log::error!(target: "oxyroute", "websocket {path} bridge: {e}");
                let _ = Python::with_gil(|py| -> PyResult<()> {
                    let _ = protocol.bind(py).call_method1("close", (1011i32,));
                    Ok(())
                });
                return Ok(Python::with_gil(|py| py.None()));
            }
        };
        if let Err(e) = fut.await {
            log::error!(target: "oxyroute", "websocket {path} handler error: {e}");
            let _ = Python::with_gil(|py| -> PyResult<()> {
                let _ = protocol.bind(py).call_method1("close", (1011i32,));
                Ok(())
            });
        }
    }
    Ok(Python::with_gil(|py| py.None()))
}
