use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{DecodingKey, Validation, decode};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use serde_json::Value as JsonValue;

use crate::params::{header_get_lax, parse_query, value_for_path_param};
use crate::response;
use crate::schema::json_to_py;
use crate::state::{map_method_router, AppState};
use crate::token::extract_bearer;

pub async fn run_rsgi(
    state: Arc<Mutex<AppState>>,
    scope: Py<PyAny>,
    protocol: Py<PyAny>,
) -> PyResult<PyObject> {
    let proto: String = Python::with_gil(|py| scope.bind(py).getattr("proto")?.extract())?;
    if proto != "http" {
        return Ok(Python::with_gil(|py| py.None()));
    }
    let (method, path, query_string) = Python::with_gil(|py| -> PyResult<(String, String, String)> {
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
    if method == "GET" && path == "/openapi.json" {
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
            return response::send_str(
                &protocol,
                200,
                &doc,
                "application/json; charset=utf-8",
            )
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
        let g = map_method_router(&*st, &method)
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
            return response::send_text(
                &protocol,
                404,
                "Not Found",
                "text/plain; charset=utf-8",
            )
            .await
        }
    };
    let (
        handler,
        is_async,
        require_jwt,
        jwt_secret,
        algs,
        read_json_body,
        dep_names,
        dep_factories,
        dep_is_async,
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
            e.read_json_body,
            e.dep_names.clone(),
            e.dep_factories.clone(),
            e.dep_is_async.clone(),
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
                    .await
                }
                return response::send_text(
                    &protocol,
                    401,
                    "Unauthorized",
                    "text/plain; charset=utf-8",
                )
                .await
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
    let mut dep_out: Vec<PyObject> = Vec::with_capacity(dep_factories.len());
    for (i, fact) in dep_factories.iter().enumerate() {
        if dep_is_async.get(i) == Some(&true) {
            let r = Python::with_gil(|py| -> PyResult<PyObject> {
                Ok(fact.bind(py).call((), None)?.unbind())
            })?;
            let fut = Python::with_gil(|py| {
                let b = r.bind(py).clone();
                pyo3_asyncio_0_21::tokio::into_future(b)
            })?;
            let o = fut.await?;
            dep_out.push(o);
        } else {
            let o = Python::with_gil(|py| -> PyResult<PyObject> {
                Ok(fact.bind(py).call((), None)?.unbind())
            })?;
            dep_out.push(o);
        }
    }
    let (res, run_async) = Python::with_gil(|py| -> PyResult<(PyObject, bool)> {
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
                kwargs.set_item(name, oo.bind(py))?;
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
        let res = handler
            .bind(py)
            .call((), Some(&kwargs))?
            .unbind();
        Ok((res, is_async))
    })?;
    let handler_out: PyObject = if run_async {
        let fut = Python::with_gil(|py| {
            let b = res.bind(py).clone();
            pyo3_asyncio_0_21::tokio::into_future(b)
        })?;
        fut.await?
    } else {
        res
    };
    let (status, bytes, content_type) = Python::with_gil(|py| -> PyResult<(u16, Vec<u8>, String)> {
        let b = handler_out.bind(py);
        if let Ok(s) = b.extract::<String>() {
            return Ok((200, s.into_bytes(), "text/plain; charset=utf-8".to_string()));
        }
        if let Ok(s) = b.extract::<&str>() {
            return Ok((200, s.as_bytes().to_vec(), "text/plain; charset=utf-8".to_string()));
        }
        if let Ok(buf) = b.extract::<Vec<u8>>() {
            return Ok((200, buf, "application/octet-stream".to_string()));
        }
        if let Ok(d) = b.downcast::<PyDict>() {
            let st = d.get_item("status")?;
            let bd = d.get_item("body")?;
            if let (Some(sc), Some(body)) = (st, bd) {
                if let (Ok(code), Ok(bstr)) = (sc.extract::<u16>(), body.str()) {
                    return Ok((code, bstr.to_string().into_bytes(), "text/plain; charset=utf-8".to_string()));
                }
            }
        }
        let jmod = py.import_bound("json")?;
        let dumped = jmod.call_method1("dumps", (b.clone().unbind(),))?;
        let s: String = dumped.extract()?;
        Ok((200, s.into_bytes(), "application/json; charset=utf-8".to_string()))
    })?;
    response::send_bytes(&protocol, status, &bytes, &content_type).await
}
