use std::sync::Mutex;

use matchit::Router;
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
    pub read_json_body: bool,
    /// Dependency `name` -> factory callable (linear order; resolved in order, then user handler).
    pub dep_names: Vec<String>,
    pub dep_factories: Vec<Py<PyAny>>,
    pub dep_is_async: Vec<bool>,
}

pub struct AppState {
    pub routes: Vec<RouteEntry>,
    pub get: Mutex<Router<usize>>,
    pub post: Mutex<Router<usize>>,
    pub put: Mutex<Router<usize>>,
    pub patch: Mutex<Router<usize>>,
    pub delete: Mutex<Router<usize>>,
    pub openapi: Mutex<serde_json::Value>,
    /// When true, `add_route` fails (DAG / routes frozen for DI).
    pub frozen: bool,
    /// Serve `GET /openapi.json` from the built document without a user route.
    pub include_openapi: bool,
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
            openapi: Mutex::new(openapi),
            frozen: false,
            include_openapi: true,
        }
    }
}

pub fn map_method_router<'a>(
    state: &'a AppState,
    method: &str,
) -> Option<std::sync::MutexGuard<'a, matchit::Router<usize>>> {
    match method {
        "GET" => state.get.lock().ok(),
        "POST" => state.post.lock().ok(),
        "PUT" => state.put.lock().ok(),
        "PATCH" => state.patch.lock().ok(),
        "DELETE" => state.delete.lock().ok(),
        _ => None,
    }
}
