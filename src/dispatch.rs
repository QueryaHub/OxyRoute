use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, Validation};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};
use serde_json::Value as JsonValue;

use crate::form::{self, ParsedFile};
use crate::params::{build_request_context, header_get_lax, parse_query, value_for_path_param};
use crate::response;
use crate::schema::json_to_py;
use crate::state::{match_route, match_ws_route, methods_matching_path, AppState};
use crate::token::{build_decoding_key, extract_bearer, extract_cookie_value};
use crate::websocket::WebSocket;

type HttpExceptionPayload = (u16, Vec<u8>, Vec<(String, String)>);

type RouteCallSnapshot = (
    Py<PyAny>,
    bool,
    bool,
    Option<String>,
    Vec<jsonwebtoken::Algorithm>,
    Option<String>,
    Option<String>,
    u64,
    Option<String>,
    bool, // read_json_body
    bool, // read_form_body
    Vec<String>,
    Vec<Py<PyAny>>,
    Vec<bool>,
    Vec<bool>,
    HashSet<String>,
    bool,
);

fn oxyroute_debug() -> bool {
    std::env::var("OXYROUTE_DEBUG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
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

/// If ``err`` is :class:`oxyroute.exceptions.HTTPException`, send status/body/headers; else ``None``.
async fn try_http_exception(protocol: &Py<PyAny>, err: &PyErr) -> PyResult<Option<PyObject>> {
    let payload: Option<HttpExceptionPayload> =
        Python::with_gil(|py| -> PyResult<Option<HttpExceptionPayload>> {
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
        })?;
    let Some((st, body, mut headers)) = payload else {
        return Ok(None);
    };
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
    let out = response::send_with_headers(protocol, st, &body, headers).await?;
    Ok(Some(out))
}

/// Like [`send_internal_error`], but maps :class:`HTTPException` to its status and body.
async fn send_python_error(
    protocol: &Py<PyAny>,
    method: &str,
    path: &str,
    err: PyErr,
) -> PyResult<PyObject> {
    if let Some(res) = try_http_exception(protocol, &err).await? {
        return Ok(res);
    }
    send_internal_error(protocol, method, path, err).await
}

fn ensure_compiled_snapshot(state: &Arc<RwLock<AppState>>) {
    let mut st = state.write();
    if st.compiled.is_none() {
        st.compiled = Some(Arc::new(st.snapshot_routers()));
    }
}

pub async fn run_rsgi(
    state: Arc<RwLock<AppState>>,
    scope: Py<PyAny>,
    protocol: Py<PyAny>,
) -> PyResult<PyObject> {
    let proto: String = Python::with_gil(|py| scope.bind(py).getattr("proto")?.extract())?;
    if proto == "websocket" {
        return run_rsgi_websocket(state, scope, protocol).await;
    }
    if proto != "http" {
        return Ok(Python::with_gil(|py| py.None()));
    }
    let (method, path, query_string) =
        Python::with_gil(|py| -> PyResult<(String, String, String)> {
            let s = scope.bind(py);
            let qs: String = s
                .getattr("query_string")
                .and_then(|x| x.extract())
                .unwrap_or_default();
            Ok((
                s.getattr("method")?.extract()?,
                s.getattr("path")?.extract()?,
                qs,
            ))
        })?;
    let is_head = method == "HEAD";
    // `Py<PyAny>` must be cloned while the GIL is held (see PyO3 0.21+).
    let cors_cfg = Python::with_gil(|_py| {
        let s = state.read();
        s.cors.clone()
    });
    let security_cfg = Python::with_gil(|_py| {
        let s = state.read();
        s.security_headers.clone()
    });
    if (method == "GET" || method == "HEAD") && path == "/openapi.json" {
        let (inc, doc) = {
            let st = state.read();
            if !st.include_openapi {
                (false, String::new())
            } else {
                let oa = st.openapi.lock();
                (true, oa.to_string())
            }
        };
        if inc {
            if is_head {
                return response::send_head_simple(
                    &protocol,
                    200,
                    doc.len(),
                    "application/json; charset=utf-8",
                )
                .await;
            }
            return response::send_str(&protocol, 200, &doc, "application/json; charset=utf-8")
                .await;
        }
    }
    let maybe_mw = Python::with_gil(|_py| {
        let st = state.read();
        st.middleware.clone()
    });
    if let Some(mw) = maybe_mw {
        let out: Py<PyAny> = match Python::with_gil(|py| {
            let f = mw.bind(py);
            f.call1((scope.bind(py), protocol.bind(py)))
                .map(|b| b.unbind())
        }) {
            Ok(x) => x,
            Err(e) => {
                return send_python_error(&protocol, &method, &path, e).await;
            }
        };
        let skip = Python::with_gil(|py| out.bind(py).is_none());
        if !skip {
            let mapped = match Python::with_gil(|py| map_handler_return(py, &out)) {
                Ok(m) => m,
                Err(e) => {
                    return send_python_error(&protocol, &method, &path, e).await;
                }
            };
            let mapped = match Python::with_gil(|py| {
                merge_config_response_headers(
                    py,
                    &security_cfg,
                    scope.bind(py).clone(),
                    mapped,
                    true,
                )
            }) {
                Ok(m) => m,
                Err(e) => {
                    return send_python_error(&protocol, &method, &path, e).await;
                }
            };
            let mapped = match Python::with_gil(|py| {
                merge_config_response_headers(py, &cors_cfg, scope.bind(py).clone(), mapped, false)
            }) {
                Ok(m) => m,
                Err(e) => {
                    return send_python_error(&protocol, &method, &path, e).await;
                }
            };
            return send_handler_map(&protocol, is_head, mapped).await;
        }
    }
    // Auto-enable compiled route snapshot on first request to keep hot-path matching lock-free
    // even when users forget to call `freeze()` explicitly.
    ensure_compiled_snapshot(&state);
    let route_out: Option<(usize, HashMap<String, String>)> = {
        let st = state.read();
        match match_route(&st, &method, &path) {
            None => Err(pyo3::exceptions::PyValueError::new_err("method")),
            Some(m) => Ok(m),
        }
    }?;
    let (route_idx, param_map) = match route_out {
        Some(x) => x,
        None => {
            let m = {
                let st = state.read();
                methods_matching_path(&st, &path)
            };
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
    let (
        handler,
        is_async,
        require_jwt,
        jwt_secret,
        algs,
        jwt_issuer,
        jwt_audience,
        jwt_leeway,
        jwt_cookie,
        read_json_body,
        read_form_body,
        dep_names,
        dep_factories,
        dep_is_async,
        dep_wants_request,
        handler_param_names,
        handler_varkw,
    ) = Python::with_gil(|_py| -> PyResult<RouteCallSnapshot> {
        let st = state.read();
        let e = st
            .routes
            .get(route_idx)
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("route index"))?;
        Ok((
            e.handler.clone(),
            e.is_async,
            e.require_jwt,
            e.jwt_secret.clone(),
            e.algs.clone(),
            e.jwt_issuer.clone(),
            e.jwt_audience.clone(),
            e.jwt_leeway,
            e.jwt_cookie.clone(),
            e.read_json_body,
            e.read_form_body,
            e.dep_names.clone(),
            e.dep_factories.clone(),
            e.dep_is_async.clone(),
            e.dep_wants_request.clone(),
            e.handler_param_names.clone(),
            e.handler_varkw,
        ))
    })?;
    let read_fut = Python::with_gil(|py| {
        let p = protocol.bind(py);
        let aw: Bound<PyAny> = p.call0()?;
        pyo3_async_runtimes::tokio::into_future(aw)
    })?;
    let body_obj: PyObject = read_fut.await?;
    let mut body_bytes: Vec<u8> = Python::with_gil(|py| -> PyResult<Vec<u8>> {
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
    if (body_bytes.len() as u64) > max {
        return response::send_text(
            &protocol,
            413,
            r#"{"error":"payload too large"}"#,
            "application/json; charset=utf-8",
        )
        .await;
    }
    let (auth, cookie_raw) =
        Python::with_gil(|py| -> PyResult<(Option<String>, Option<String>)> {
            let s = scope.bind(py);
            let headers = s.getattr("headers")?;
            Ok((
                header_get_lax(&headers, "authorization"),
                header_get_lax(&headers, "cookie"),
            ))
        })?;
    let mut claims_val: Option<JsonValue> = None;
    if require_jwt {
        let key = match jwt_secret {
            None => {
                return response::send_text(
                    &protocol,
                    401,
                    "Unauthorized",
                    "text/plain; charset=utf-8",
                )
                .await
            }
            Some(s) => s,
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
        let mut val = if let Some(f) = algs.first() {
            Validation::new(*f)
        } else {
            return response::send_text(
                &protocol,
                401,
                "Unauthorized",
                "text/plain; charset=utf-8",
            )
            .await;
        };
        val.algorithms = algs.clone();
        val.validate_nbf = true;
        val.leeway = jwt_leeway;
        if let Some(ref iss) = jwt_issuer {
            val.set_issuer(&[iss]);
        }
        if let Some(ref aud) = jwt_audience {
            val.set_audience(&[aud]);
        } else {
            // jsonwebtoken 9: with validate_aud + aud=None, a token that includes `aud` fails
            // (InvalidAudience). Disable unless the route opts in to an expected audience.
            val.validate_aud = false;
        }
        let dk = match build_decoding_key(&key, &algs) {
            Ok(d) => d,
            Err(_) => {
                return response::send_text(
                    &protocol,
                    401,
                    "Unauthorized",
                    "text/plain; charset=utf-8",
                )
                .await;
            }
        };
        match decode::<JsonValue>(&token, &dk, &val) {
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
    let query_map = parse_query(&query_string);
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
                    return send_python_error(&protocol, &method, &path, e).await;
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
    let need_req_ctx = dep_wants_request.iter().any(|&x| x);
    let request_ctx: Option<Py<PyAny>> = if need_req_ctx {
        match Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            let s = scope.bind(py);
            let d = build_request_context(py, s, &method, &path, &query_string)?;
            Ok(d.unbind().into())
        }) {
            Ok(o) => Some(o),
            Err(e) => {
                return send_python_error(&protocol, &method, &path, e).await;
            }
        }
    } else {
        None
    };
    let mut dep_out: Vec<PyObject> = Vec::with_capacity(dep_factories.len());
    for (i, fact) in dep_factories.iter().enumerate() {
        if dep_is_async.get(i) == Some(&true) {
            let r = match Python::with_gil(|py| -> PyResult<PyObject> {
                let kw = PyDict::new(py);
                if dep_wants_request.get(i) == Some(&true) {
                    if let Some(ref rc) = request_ctx {
                        kw.set_item("request", rc.bind(py))?;
                    }
                }
                for j in 0..i {
                    kw.set_item(dep_names[j].as_str(), dep_out[j].bind(py))?;
                }
                let f = fact.bind(py);
                if kw.is_empty() {
                    Ok(f.call((), None)?.unbind())
                } else {
                    Ok(f.call((), Some(&kw))?.unbind())
                }
            }) {
                Ok(x) => x,
                Err(e) => {
                    return send_python_error(&protocol, &method, &path, e).await;
                }
            };
            let fut = match Python::with_gil(|py| {
                let b = r.bind(py).clone();
                pyo3_async_runtimes::tokio::into_future(b)
            }) {
                Ok(f) => f,
                Err(e) => {
                    return send_python_error(&protocol, &method, &path, e).await;
                }
            };
            let o = match fut.await {
                Ok(x) => x,
                Err(e) => {
                    return send_python_error(&protocol, &method, &path, e).await;
                }
            };
            dep_out.push(o);
        } else {
            let o = match Python::with_gil(|py| -> PyResult<PyObject> {
                let kw = PyDict::new(py);
                if dep_wants_request.get(i) == Some(&true) {
                    if let Some(ref rc) = request_ctx {
                        kw.set_item("request", rc.bind(py))?;
                    }
                }
                for j in 0..i {
                    kw.set_item(dep_names[j].as_str(), dep_out[j].bind(py))?;
                }
                let f = fact.bind(py);
                if kw.is_empty() {
                    Ok(f.call((), None)?.unbind())
                } else {
                    Ok(f.call((), Some(&kw))?.unbind())
                }
            }) {
                Ok(x) => x,
                Err(e) => {
                    return send_python_error(&protocol, &method, &path, e).await;
                }
            };
            dep_out.push(o);
        }
    }
    let (res, run_async) = match Python::with_gil(|py| -> PyResult<(PyObject, bool)> {
        let kwargs = PyDict::new(py);
        for (k, v) in param_map {
            let vpy = value_for_path_param(py, &v);
            kwargs.set_item(k, vpy)?;
        }
        if !query_map.is_empty() {
            let qd = PyDict::new(py);
            for (k, v) in &query_map {
                qd.set_item(k, v.as_str())?;
            }
            kwargs.set_item("query", qd)?;
        }
        for (i, name) in dep_names.iter().enumerate() {
            if let Some(oo) = dep_out.get(i) {
                if handler_varkw || handler_param_names.contains(name) {
                    kwargs.set_item(name, oo.bind(py))?;
                }
            }
        }
        if let Some(c) = claims_val {
            let pyv = json_to_py(py, c)?;
            kwargs.set_item("claims", pyv)?;
        }
        if let Some(ref j) = body_json {
            let pyv = json_to_py(py, j.clone())?;
            kwargs.set_item("json", pyv)?;
        }
        if read_form_body {
            if handler_varkw || handler_param_names.contains("form") {
                let fd = PyDict::new(py);
                for (k, v) in &form_map {
                    fd.set_item(k, v.as_str())?;
                }
                kwargs.set_item("form", fd)?;
            }
            if handler_varkw || handler_param_names.contains("files") {
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
        } else if !body_bytes.is_empty() && body_json.is_none() {
            kwargs.set_item("body", PyBytes::new(py, &body_bytes))?;
        }
        if handler_varkw || handler_param_names.contains("protocol") {
            kwargs.set_item("protocol", protocol.bind(py))?;
        }
        let res = handler.bind(py).call((), Some(&kwargs))?.unbind();
        Ok((res, is_async))
    }) {
        Ok(x) => x,
        Err(e) => {
            return send_python_error(&protocol, &method, &path, e).await;
        }
    };
    let handler_out: PyObject = if run_async {
        let fut = match Python::with_gil(|py| {
            let b = res.bind(py).clone();
            pyo3_async_runtimes::tokio::into_future(b)
        }) {
            Ok(f) => f,
            Err(e) => {
                return send_python_error(&protocol, &method, &path, e).await;
            }
        };
        match fut.await {
            Ok(x) => x,
            Err(e) => {
                return send_python_error(&protocol, &method, &path, e).await;
            }
        }
    } else {
        res
    };
    let mapped = match Python::with_gil(|py| map_handler_return(py, &handler_out)) {
        Ok(m) => m,
        Err(e) => {
            return send_python_error(&protocol, &method, &path, e).await;
        }
    };
    let mapped = match Python::with_gil(|py| {
        merge_config_response_headers(py, &security_cfg, scope.bind(py).clone(), mapped, true)
    }) {
        Ok(m) => m,
        Err(e) => {
            return send_python_error(&protocol, &method, &path, e).await;
        }
    };
    let mapped = match Python::with_gil(|py| {
        merge_config_response_headers(py, &cors_cfg, scope.bind(py).clone(), mapped, false)
    }) {
        Ok(m) => m,
        Err(e) => {
            return send_python_error(&protocol, &method, &path, e).await;
        }
    };
    send_handler_map(&protocol, is_head, mapped).await
}

/// Return value of a user handler, mapped to an HTTP body and headers.
fn map_handler_return(py: Python<'_>, out: &Py<PyAny>) -> PyResult<HandlerMap> {
    let b = out.bind(py);
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
            body: s.into_bytes(),
            content_type: "text/plain; charset=utf-8".to_string(),
        });
    }
    if let Ok(s) = b.extract::<&str>() {
        return Ok(HandlerMap::Simple {
            status: 200,
            body: s.as_bytes().to_vec(),
            content_type: "text/plain; charset=utf-8".to_string(),
        });
    }
    if let Ok(buf) = b.extract::<Vec<u8>>() {
        return Ok(HandlerMap::Simple {
            status: 200,
            body: buf,
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
                    body: bstr.to_string().into_bytes(),
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
        body: s.into_bytes(),
        content_type: "application/json; charset=utf-8".to_string(),
    })
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
        body: Vec<u8>,
        content_type: String,
    },
}

async fn send_handler_map(
    protocol: &Py<PyAny>,
    is_head: bool,
    mapped: HandlerMap,
) -> PyResult<PyObject> {
    if matches!(mapped, HandlerMap::AlreadySent) {
        return Ok(Python::with_gil(|py| py.None()));
    }
    if is_head {
        match mapped {
            HandlerMap::AlreadySent => Ok(Python::with_gil(|py| py.None())),
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            } => response::send_head_with_headers(protocol, status, &body, headers).await,
            HandlerMap::Simple {
                status,
                body,
                content_type,
            } => response::send_head_simple(protocol, status, body.len(), &content_type).await,
        }
    } else {
        match mapped {
            HandlerMap::AlreadySent => Ok(Python::with_gil(|py| py.None())),
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            } => response::send_with_headers(protocol, status, &body, headers).await,
            HandlerMap::Simple {
                status,
                body,
                content_type,
            } => response::send_bytes(protocol, status, &body, &content_type).await,
        }
    }
}

/// `if_absent`: only add a header if no same-name (case-insensitive) header is already present
/// (``security`` preset). `false` replaces/merges like CORS (``replace`` / duplicate header names).
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
    let pairs: Vec<(String, String)> = c
        .call_method1(py, "response_header_pairs", (&scope,))?
        .extract(py)?;
    if pairs.is_empty() {
        return Ok(mapped);
    }
    if if_absent {
        Ok(merge_header_pairs_if_absent(mapped, &pairs))
    } else {
        Ok(merge_header_pairs_replace(mapped, &pairs))
    }
}

fn merge_header_pairs_replace(mapped: HandlerMap, extra: &[(String, String)]) -> HandlerMap {
    if extra.is_empty() {
        return mapped;
    }
    match mapped {
        HandlerMap::AlreadySent => HandlerMap::AlreadySent,
        HandlerMap::WithHeaders {
            status,
            body,
            mut headers,
        } => {
            for (a, b) in extra {
                headers.retain(|(k, _)| !k.eq_ignore_ascii_case(a));
                headers.push((a.clone(), b.clone()));
            }
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            }
        }
        HandlerMap::Simple {
            status,
            body,
            content_type,
        } => {
            let mut headers = vec![("content-type".to_string(), content_type)];
            for (a, b) in extra {
                headers.retain(|(k, _)| !k.eq_ignore_ascii_case(a));
                headers.push((a.clone(), b.clone()));
            }
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            }
        }
    }
}

fn merge_header_pairs_if_absent(mapped: HandlerMap, extra: &[(String, String)]) -> HandlerMap {
    if extra.is_empty() {
        return mapped;
    }
    match mapped {
        HandlerMap::AlreadySent => HandlerMap::AlreadySent,
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
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            }
        }
        HandlerMap::Simple {
            status,
            body,
            content_type,
        } => {
            let mut headers = vec![("content-type".to_string(), content_type)];
            for (a, b) in extra {
                if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(a)) {
                    headers.push((a.clone(), b.clone()));
                }
            }
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            }
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
    ensure_compiled_snapshot(&state);
    let route_match = {
        let st = state.read();
        match_ws_route(&st, &path)
    };
    let Some((route_idx, param_map)) = route_match else {
        // No route → polite close. ``close`` is sync on RSGIWebsocketProtocol.
        let _ = Python::with_gil(|py| -> PyResult<()> {
            let p = protocol.bind(py);
            let _ = p.call_method1("close", (1000i32,));
            Ok(())
        });
        return Ok(Python::with_gil(|py| py.None()));
    };
    let (handler, is_async) = Python::with_gil(|_py| -> PyResult<(Py<PyAny>, bool)> {
        let st = state.read();
        let e = st
            .websocket_routes
            .get(route_idx)
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("ws route index"))?;
        Ok((e.handler.clone(), e.is_async))
    })?;
    let call_result = Python::with_gil(|py| -> PyResult<(PyObject, bool)> {
        let ws = WebSocket::new(protocol.clone_ref(py), scope.clone_ref(py), param_map);
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
