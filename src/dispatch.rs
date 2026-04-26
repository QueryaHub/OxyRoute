use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, DecodingKey, Validation};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use serde_json::Value as JsonValue;

use crate::params::{build_request_context, header_get_lax, parse_query, value_for_path_param};
use crate::response;
use crate::schema::json_to_py;
use crate::state::{map_method_router, methods_matching_path, AppState};
use crate::token::extract_bearer;

fn oxyroute_debug() -> bool {
    std::env::var("OXYROUTE_DEBUG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
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

pub async fn run_rsgi(
    state: Arc<Mutex<AppState>>,
    scope: Py<PyAny>,
    protocol: Py<PyAny>,
) -> PyResult<PyObject> {
    let proto: String = Python::with_gil(|py| scope.bind(py).getattr("proto")?.extract())?;
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
    if (method == "GET" || method == "HEAD") && path == "/openapi.json" {
        let (inc, doc) = {
            let st = state.lock().map_err(crate::lock_err)?;
            if !st.include_openapi {
                (false, String::new())
            } else {
                let oa = st.openapi.lock().map_err(crate::lock_err)?;
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
    let read_fut = Python::with_gil(|py| {
        let p = protocol.bind(py);
        let aw: Bound<PyAny> = p.call0()?;
        pyo3_asyncio_0_21::tokio::into_future(aw)
    })?;
    let body_obj: PyObject = read_fut.await?;
    let body_bytes: Vec<u8> = Python::with_gil(|py| -> PyResult<Vec<u8>> {
        let b = body_obj.bind(py);
        if let Ok(x) = b.extract::<Vec<u8>>() {
            return Ok(x);
        }
        if let Ok(s) = b.str() {
            return Ok(s.to_string().into_bytes());
        }
        Ok(Vec::new())
    })?;
    let auth = Python::with_gil(|py| -> PyResult<Option<String>> {
        let s = scope.bind(py);
        let headers = s.getattr("headers")?;
        Ok(header_get_lax(&headers, "authorization"))
    })?;
    let route_out: Option<(usize, HashMap<String, String>)> = (|| -> PyResult<_> {
        let st = state.lock().map_err(crate::lock_err)?;
        let g = map_method_router(&st, &method)
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("method"))?;
        Ok(g.at(&path).ok().map(|m| {
            let mut pmap = HashMap::new();
            for (k, v) in m.params.iter() {
                pmap.insert(k.to_string(), v.to_string());
            }
            (*m.value, pmap)
        }))
    })()?;
    let (route_idx, param_map) = match route_out {
        Some(x) => x,
        None => {
            let m = {
                let st = state.lock().map_err(crate::lock_err)?;
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
        read_json_body,
        dep_names,
        dep_factories,
        dep_is_async,
        dep_wants_request,
        handler_param_names,
        handler_varkw,
    ) = {
        let st = state.lock().map_err(crate::lock_err)?;
        let e = st
            .routes
            .get(route_idx)
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("route index"))?;
        (
            e.handler.clone(),
            e.is_async,
            e.require_jwt,
            e.jwt_secret.clone(),
            e.algs.clone(),
            e.jwt_issuer.clone(),
            e.jwt_audience.clone(),
            e.jwt_leeway,
            e.read_json_body,
            e.dep_names.clone(),
            e.dep_factories.clone(),
            e.dep_is_async.clone(),
            e.dep_wants_request.clone(),
            e.handler_param_names.clone(),
            e.handler_varkw,
        )
    };
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
        let token = match extract_bearer(auth.as_deref()) {
            Some(t) => t,
            None => {
                return response::send_text(
                    &protocol,
                    401,
                    "Unauthorized",
                    "text/plain; charset=utf-8",
                )
                .await
            }
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
        val.algorithms = algs;
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
        let dk = DecodingKey::from_secret(key.as_bytes());
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
    let need_req_ctx = dep_wants_request.iter().any(|&x| x);
    let request_ctx: Option<Py<PyAny>> = if need_req_ctx {
        match Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            let s = scope.bind(py);
            let d = build_request_context(py, s, &method, &path, &query_string)?;
            Ok(d.unbind().into())
        }) {
            Ok(o) => Some(o),
            Err(e) => {
                return send_internal_error(&protocol, &method, &path, e).await;
            }
        }
    } else {
        None
    };
    let mut dep_out: Vec<PyObject> = Vec::with_capacity(dep_factories.len());
    for (i, fact) in dep_factories.iter().enumerate() {
        if dep_is_async.get(i) == Some(&true) {
            let r = match Python::with_gil(|py| -> PyResult<PyObject> {
                let kw = PyDict::new_bound(py);
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
                    return send_internal_error(&protocol, &method, &path, e).await;
                }
            };
            let fut = match Python::with_gil(|py| {
                let b = r.bind(py).clone();
                pyo3_asyncio_0_21::tokio::into_future(b)
            }) {
                Ok(f) => f,
                Err(e) => {
                    return send_internal_error(&protocol, &method, &path, e).await;
                }
            };
            let o = match fut.await {
                Ok(x) => x,
                Err(e) => {
                    return send_internal_error(&protocol, &method, &path, e).await;
                }
            };
            dep_out.push(o);
        } else {
            let o = match Python::with_gil(|py| -> PyResult<PyObject> {
                let kw = PyDict::new_bound(py);
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
                    return send_internal_error(&protocol, &method, &path, e).await;
                }
            };
            dep_out.push(o);
        }
    }
    let (res, run_async) = match Python::with_gil(|py| -> PyResult<(PyObject, bool)> {
        let kwargs = PyDict::new_bound(py);
        for (k, v) in param_map {
            let vpy = value_for_path_param(py, &v);
            kwargs.set_item(k, vpy)?;
        }
        if !query_map.is_empty() {
            let qd = PyDict::new_bound(py);
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
        if !body_bytes.is_empty() && body_json.is_none() {
            kwargs.set_item("body", PyBytes::new_bound(py, &body_bytes))?;
        }
        let res = handler.bind(py).call((), Some(&kwargs))?.unbind();
        Ok((res, is_async))
    }) {
        Ok(x) => x,
        Err(e) => {
            return send_internal_error(&protocol, &method, &path, e).await;
        }
    };
    let handler_out: PyObject = if run_async {
        let fut = match Python::with_gil(|py| {
            let b = res.bind(py).clone();
            pyo3_asyncio_0_21::tokio::into_future(b)
        }) {
            Ok(f) => f,
            Err(e) => {
                return send_internal_error(&protocol, &method, &path, e).await;
            }
        };
        match fut.await {
            Ok(x) => x,
            Err(e) => {
                return send_internal_error(&protocol, &method, &path, e).await;
            }
        }
    } else {
        res
    };
    let mapped = match Python::with_gil(|py| map_handler_return(py, &handler_out)) {
        Ok(m) => m,
        Err(e) => {
            return send_internal_error(&protocol, &method, &path, e).await;
        }
    };
    if is_head {
        match mapped {
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            } => response::send_head_with_headers(&protocol, status, &body, headers).await,
            HandlerMap::Simple {
                status,
                body,
                content_type,
            } => response::send_head_simple(&protocol, status, body.len(), &content_type).await,
        }
    } else {
        match mapped {
            HandlerMap::WithHeaders {
                status,
                body,
                headers,
            } => response::send_with_headers(&protocol, status, &body, headers).await,
            HandlerMap::Simple {
                status,
                body,
                content_type,
            } => response::send_bytes(&protocol, status, &body, &content_type).await,
        }
    }
}

/// Return value of a user handler, mapped to an HTTP body and headers.
fn map_handler_return(py: Python<'_>, out: &Py<PyAny>) -> PyResult<HandlerMap> {
    let b = out.bind(py);
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
    let jmod = py.import_bound("json")?;
    let dumped = jmod.call_method1("dumps", (b.clone().unbind(),))?;
    let s: String = dumped.extract()?;
    Ok(HandlerMap::Simple {
        status: 200,
        body: s.into_bytes(),
        content_type: "application/json; charset=utf-8".to_string(),
    })
}

enum HandlerMap {
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
    let jmod = py.import_bound("json")?;
    let dumped = jmod.call_method1("dumps", (b.clone().unbind(),))?;
    let s: String = dumped.extract()?;
    Ok((
        s.into_bytes(),
        "application/json; charset=utf-8".to_string(),
    ))
}
