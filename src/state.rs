use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use matchit::Router;
use parking_lot::Mutex;
use pyo3::prelude::*;

/// Immutable route tables built at [`AppState::freeze`](AppState) time so the request
/// path can be matched without per-method `Mutex` locks (issue #4).
pub struct CompiledRouters {
    pub get: Router<usize>,
    pub post: Router<usize>,
    pub put: Router<usize>,
    pub patch: Router<usize>,
    pub delete: Router<usize>,
    pub options: Router<usize>,
    pub websocket: Router<usize>,
}

fn router_for_compiled<'a>(c: &'a CompiledRouters, method: &str) -> Option<&'a Router<usize>> {
    match method {
        "GET" | "HEAD" => Some(&c.get),
        "POST" => Some(&c.post),
        "PUT" => Some(&c.put),
        "PATCH" => Some(&c.patch),
        "DELETE" => Some(&c.delete),
        "OPTIONS" => Some(&c.options),
        _ => None,
    }
}

/// One WebSocket route: just a handler + async flag (no JWT / deps / body).
#[derive(Clone)]
pub struct WebsocketRoute {
    pub handler: Py<PyAny>,
    pub is_async: bool,
}

#[derive(Clone)]
pub struct RouteEntry {
    pub handler: Py<PyAny>,
    pub is_async: bool,
    pub require_jwt: bool,
    pub jwt_secret: Option<String>,
    pub algs: Arc<[jsonwebtoken::Algorithm]>,
    /// `None` in Python → no issuer check; else `set_issuer` in jsonwebtoken.
    pub jwt_issuer: Option<String>,
    /// `None` in Python → `validate_aud` disabled for this route.
    pub jwt_audience: Option<String>,
    /// Clock skew (seconds); Python `None` uses default 60 (jsonwebtoken default).
    pub jwt_leeway: u64,
    /// If set, read JWT from the `Cookie` header when `Authorization: Bearer` is missing.
    pub jwt_cookie: Option<String>,
    pub read_json_body: bool,
    /// When set, body is parsed as form data (``application/x-www-form-urlencoded`` or ``multipart/form-data``), not JSON.
    pub read_form_body: bool,
    /// Dependency `name` -> factory callable (linear order; resolved in order, then user handler).
    pub dep_names: Arc<[String]>,
    pub dep_factories: Arc<[Py<PyAny>]>,
    pub dep_is_async: Arc<[bool]>,
    /// Per factory: pass a `request` context dict (see `build_request_context` in dispatch).
    pub dep_wants_request: Arc<[bool]>,
    /// From `inspect.signature(handler)`: which parameter names the handler accepts (excluding
    /// `*args` / only `*`-only); used to forward only matching dependency results.
    pub handler_param_names: Arc<HashSet<String>>,
    /// Handler has `**kwargs` (pass all dependency kwargs).
    pub handler_varkw: bool,
    /// Sync ``call0()`` route with no body/JWT/deps/kwargs — eligible for RSGI sync fast path.
    pub trivial_sync: bool,
}

/// True when the route can be served by [`try_rsgi_sync_short_circuit`](crate::dispatch::try_rsgi_sync_short_circuit)
/// without body read, JWT, or dependency resolution.
pub fn route_is_trivial_sync(entry: &RouteEntry) -> bool {
    entry.trivial_sync
}

pub struct AppState {
    /// Wrapped in `Arc<Vec<…>>` so the hot path can clone a cheap pointer **once** per request and
    /// release [`AppState`]'s `RwLock` immediately. Mutation goes through [`Arc::make_mut`].
    pub routes: Arc<Vec<RouteEntry>>,
    /// Same Arc-snapshot trick as [`routes`](Self::routes); registration uses [`Arc::make_mut`].
    pub websocket_routes: Arc<Vec<WebsocketRoute>>,
    pub get: Mutex<Router<usize>>,
    pub post: Mutex<Router<usize>>,
    pub put: Mutex<Router<usize>>,
    pub patch: Mutex<Router<usize>>,
    pub delete: Mutex<Router<usize>>,
    pub options: Mutex<Router<usize>>,
    pub websocket: Mutex<Router<usize>>,
    pub openapi: Mutex<serde_json::Value>,
    /// When `Some`, route matching uses these tables without taking per-router mutexes
    /// (populated in [`App::freeze`](crate::App::freeze)).
    pub compiled: Option<Arc<CompiledRouters>>,
    /// When true, `add_route` fails (DAG / routes frozen for DI).
    pub frozen: bool,
    /// Serve `GET /openapi.json` from the built document without a user route.
    pub include_openapi: bool,
    /// Optional `(scope, protocol) ->` hook; return ``None`` to continue to routing (see `docs/handlers.md`).
    pub middleware: Option<Py<PyAny>>,
    /// Optional Python CORS config (e.g. :class:`oxyroute.cors.CORSConfig`) for response headers.
    pub cors: Option<Py<PyAny>>,
    /// Optional :class:`oxyroute.security_headers.SecurityHeadersConfig` (or compatible
    /// ``response_header_pairs``) merged if those header names are not already set.
    pub security_headers: Option<Py<PyAny>>,
    /// Global connection pool for the Postgres database.
    pub db_pool: Option<sqlx::SqlitePool>,
}

impl AppState {
    pub fn new() -> Self {
        let openapi = serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "OxyRoute", "version": "0.3.0" },
            "paths": {}
        });
        Self {
            routes: Arc::new(Vec::new()),
            websocket_routes: Arc::new(Vec::new()),
            get: Mutex::new(Router::new()),
            post: Mutex::new(Router::new()),
            put: Mutex::new(Router::new()),
            patch: Mutex::new(Router::new()),
            delete: Mutex::new(Router::new()),
            options: Mutex::new(Router::new()),
            websocket: Mutex::new(Router::new()),
            openapi: Mutex::new(openapi),
            compiled: None,
            frozen: false,
            include_openapi: true,
            middleware: None,
            cors: None,
            security_headers: None,
            db_pool: None,
        }
    }

    /// Cheap read-side snapshot of the fields the request hot path touches: the
    /// returned [`HotSnapshot`] is built **inside one** `state.read()` so the request
    /// dispatch can release the `RwLock` immediately and avoid further reads.
    ///
    /// Cheap because every cloned field is `Arc::clone` / `Option<Py<PyAny>>::clone`
    /// (both refcount bumps), not deep clones.
    pub fn hot_snapshot(&self) -> HotSnapshot {
        HotSnapshot {
            routes: Arc::clone(&self.routes),
            websocket_routes: Arc::clone(&self.websocket_routes),
            compiled: self.compiled.as_ref().map(Arc::clone),
            cors: self.cors.clone(),
            security_headers: self.security_headers.clone(),
            middleware: self.middleware.clone(),
            include_openapi: self.include_openapi,
            db_pool: self.db_pool.clone(),
        }
    }

    /// Clone current mutex-protected [`Router`]s into a snapshot (used at freeze / tests).
    pub fn snapshot_routers(&self) -> CompiledRouters {
        CompiledRouters {
            get: self.get.lock().clone(),
            post: self.post.lock().clone(),
            put: self.put.lock().clone(),
            patch: self.patch.lock().clone(),
            delete: self.delete.lock().clone(),
            options: self.options.lock().clone(),
            websocket: self.websocket.lock().clone(),
        }
    }
}

/// One-shot read-side view of [`AppState`] for [`run_rsgi`]. All fields are cheap to clone
/// (`Arc`/`Option<Py<PyAny>>` refcount bumps) so the hot path can drop the `RwLock` after a
/// single `read()`. See [`AppState::hot_snapshot`].
pub struct HotSnapshot {
    pub routes: Arc<Vec<RouteEntry>>,
    pub websocket_routes: Arc<Vec<WebsocketRoute>>,
    pub compiled: Option<Arc<CompiledRouters>>,
    pub cors: Option<Py<PyAny>>,
    pub security_headers: Option<Py<PyAny>>,
    pub middleware: Option<Py<PyAny>>,
    pub include_openapi: bool,
    pub db_pool: Option<sqlx::SqlitePool>,
}

/// Lookup a WebSocket route in a precomputed [`CompiledRouters`] (lock-free).
pub fn match_ws_route_compiled(
    compiled: &CompiledRouters,
    path: &str,
) -> Option<(usize, HashMap<String, String>)> {
    compiled.websocket.at(path).ok().map(|m| {
        let mut pmap = HashMap::new();
        for (k, v) in m.params.iter() {
            pmap.insert(k.to_string(), v.to_string());
        }
        (*m.value, pmap)
    })
}

/// Lookup an HTTP route in a precomputed [`CompiledRouters`] (lock-free).
///
/// Returns ``None`` for unsupported method, ``Some(None)`` for no match, ``Some(Some(...))`` on hit.
pub fn match_route_compiled(
    compiled: &CompiledRouters,
    method: &str,
    path: &str,
) -> Option<Option<(usize, HashMap<String, String>)>> {
    let g = router_for_compiled(compiled, method)?;
    Some(g.at(path).ok().map(|m| {
        let mut pmap = HashMap::new();
        for (k, v) in m.params.iter() {
            pmap.insert(k.to_string(), v.to_string());
        }
        (*m.value, pmap)
    }))
}

/// All HTTP methods that match `path` in a precomputed [`CompiledRouters`] (lock-free 405 list).
pub fn methods_matching_path_compiled(compiled: &CompiledRouters, path: &str) -> Vec<String> {
    const ORDER: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];
    let mut have = [false; 7];
    if compiled.get.at(path).is_ok() {
        have[0] = true;
        have[1] = true;
    }
    if compiled.post.at(path).is_ok() {
        have[2] = true;
    }
    if compiled.put.at(path).is_ok() {
        have[3] = true;
    }
    if compiled.patch.at(path).is_ok() {
        have[4] = true;
    }
    if compiled.delete.at(path).is_ok() {
        have[5] = true;
    }
    if compiled.options.at(path).is_ok() {
        have[6] = true;
    }
    ORDER
        .iter()
        .zip(have)
        .filter(|(_, ok)| *ok)
        .map(|(m, _)| (*m).to_string())
        .collect()
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
#[cfg(test)]
fn methods_matching_path(state: &AppState, path: &str) -> Vec<String> {
    const ORDER: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];
    let mut have = [false; 7];
    if let Some(c) = &state.compiled {
        if c.get.at(path).is_ok() {
            have[0] = true;
            have[1] = true;
        }
        if c.post.at(path).is_ok() {
            have[2] = true;
        }
        if c.put.at(path).is_ok() {
            have[3] = true;
        }
        if c.patch.at(path).is_ok() {
            have[4] = true;
        }
        if c.delete.at(path).is_ok() {
            have[5] = true;
        }
        if c.options.at(path).is_ok() {
            have[6] = true;
        }
    } else {
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
    }
    ORDER
        .iter()
        .zip(have)
        .filter(|(_, ok)| *ok)
        .map(|(m, _)| (*m).to_string())
        .collect()
}

/// Returns route index and path params, or `None` if the method is unsupported; `Some(None)` if
/// no match; `Some(Some)` on success. Uses [`CompiledRouters`] when set (lock-free).
#[cfg(test)]
fn match_route(
    state: &AppState,
    method: &str,
    path: &str,
) -> Option<Option<(usize, HashMap<String, String>)>> {
    if let Some(c) = &state.compiled {
        let g = router_for_compiled(c, method)?;
        return Some(g.at(path).ok().map(|m| {
            let mut pmap = HashMap::new();
            for (k, v) in m.params.iter() {
                pmap.insert(k.to_string(), v.to_string());
            }
            (*m.value, pmap)
        }));
    }
    let g = map_method_router(state, method)?;
    Some(g.at(path).ok().map(|m| {
        let mut pmap = HashMap::new();
        for (k, v) in m.params.iter() {
            pmap.insert(k.to_string(), v.to_string());
        }
        (*m.value, pmap)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_route_same_with_and_without_compiled() {
        let mut s = AppState::new();
        s.get.lock().insert("/a/:id", 7usize).unwrap();
        let pre = match_route(&s, "GET", "/a/5").expect("method");

        s.compiled = Some(Arc::new(s.snapshot_routers()));
        let post = match_route(&s, "GET", "/a/5").expect("method");

        assert_eq!(pre, post);
        let inner = pre.expect("match");
        assert_eq!(inner.0, 7);
        assert_eq!(inner.1.get("id").map(String::as_str), Some("5"));
    }

    #[test]
    fn methods_matching_path_uses_compiled() {
        let mut s = AppState::new();
        s.post.lock().insert("/x", 0usize).unwrap();
        s.compiled = Some(Arc::new(s.snapshot_routers()));
        let m = methods_matching_path(&s, "/x");
        assert_eq!(m, vec!["POST".to_string()]);
    }
}
