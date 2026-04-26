use std::collections::HashSet;

use matchit::Router;
use parking_lot::Mutex;
use pyo3::prelude::*;

pub struct RouteEntry {
    pub handler: Py<PyAny>,
    pub is_async: bool,
    pub require_jwt: bool,
    pub jwt_secret: Option<String>,
    pub algs: Vec<jsonwebtoken::Algorithm>,
    /// `None` in Python → no issuer check; else `set_issuer` in jsonwebtoken.
    pub jwt_issuer: Option<String>,
    /// `None` in Python → `validate_aud` disabled for this route.
    pub jwt_audience: Option<String>,
    /// Clock skew (seconds); Python `None` uses default 60 (jsonwebtoken default).
    pub jwt_leeway: u64,
    /// If set, read JWT from the `Cookie` header when `Authorization: Bearer` is missing.
    pub jwt_cookie: Option<String>,
    pub read_json_body: bool,
    /// Dependency `name` -> factory callable (linear order; resolved in order, then user handler).
    pub dep_names: Vec<String>,
    pub dep_factories: Vec<Py<PyAny>>,
    pub dep_is_async: Vec<bool>,
    /// Per factory: pass a `request` context dict (see `build_request_context` in dispatch).
    pub dep_wants_request: Vec<bool>,
    /// From `inspect.signature(handler)`: which parameter names the handler accepts (excluding
    /// `*args` / only `*`-only); used to forward only matching dependency results.
    pub handler_param_names: HashSet<String>,
    /// Handler has `**kwargs` (pass all dependency kwargs).
    pub handler_varkw: bool,
}

pub struct AppState {
    pub routes: Vec<RouteEntry>,
    pub get: Mutex<Router<usize>>,
    pub post: Mutex<Router<usize>>,
    pub put: Mutex<Router<usize>>,
    pub patch: Mutex<Router<usize>>,
    pub delete: Mutex<Router<usize>>,
    pub options: Mutex<Router<usize>>,
    pub openapi: Mutex<serde_json::Value>,
    /// When true, `add_route` fails (DAG / routes frozen for DI).
    pub frozen: bool,
    /// Serve `GET /openapi.json` from the built document without a user route.
    pub include_openapi: bool,
    /// Optional `(scope, protocol) ->` hook; return ``None`` to continue to routing (see `docs/handlers.md`).
    pub middleware: Option<Py<PyAny>>,
}

impl AppState {
    pub fn new() -> Self {
        let openapi = serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "OxyRoute", "version": "0.1.0" },
            "paths": {}
        });
        Self {
            routes: Vec::new(),
            get: Mutex::new(Router::new()),
            post: Mutex::new(Router::new()),
            put: Mutex::new(Router::new()),
            patch: Mutex::new(Router::new()),
            delete: Mutex::new(Router::new()),
            options: Mutex::new(Router::new()),
            openapi: Mutex::new(openapi),
            frozen: false,
            include_openapi: true,
            middleware: None,
        }
    }
}

pub fn map_method_router<'a>(
    state: &'a AppState,
    method: &str,
) -> Option<parking_lot::MutexGuard<'a, matchit::Router<usize>>> {
    match method {
        // RFC 9110: HEAD shares URI with GET; same handler, no body in the response.
        "GET" | "HEAD" => Some(state.get.lock()),
        "POST" => Some(state.post.lock()),
        "PUT" => Some(state.put.lock()),
        "PATCH" => Some(state.patch.lock()),
        "DELETE" => Some(state.delete.lock()),
        "OPTIONS" => Some(state.options.lock()),
        _ => None,
    }
}

/// All HTTP methods for which `path` matches a registered route. Used to respond with **405** and
/// an [`Allow`][1] header when the request method’s router had no match but another would.
///
/// [1]: https://www.rfc-editor.org/rfc/rfc9110#name-405-method-not-allowed
pub fn methods_matching_path(state: &AppState, path: &str) -> Vec<String> {
    const ORDER: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];
    let mut have = [false; 7];
    {
        let g = state.get.lock();
        if g.at(path).is_ok() {
            have[0] = true;
            have[1] = true;
        }
    }
    {
        let r = state.post.lock();
        if r.at(path).is_ok() {
            have[2] = true;
        }
    }
    {
        let r = state.put.lock();
        if r.at(path).is_ok() {
            have[3] = true;
        }
    }
    {
        let r = state.patch.lock();
        if r.at(path).is_ok() {
            have[4] = true;
        }
    }
    {
        let r = state.delete.lock();
        if r.at(path).is_ok() {
            have[5] = true;
        }
    }
    {
        let r = state.options.lock();
        if r.at(path).is_ok() {
            have[6] = true;
        }
    }
    ORDER
        .iter()
        .zip(have)
        .filter(|(_, ok)| *ok)
        .map(|(m, _)| (*m).to_string())
        .collect()
}
