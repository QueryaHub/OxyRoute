use std::collections::HashSet;
use std::sync::Arc;

use matchit::Router;
use parking_lot::Mutex;
use pyo3::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MethodMask(pub u8);

impl MethodMask {
    pub const GET: u8 = 1 << 0;
    pub const HEAD: u8 = 1 << 1;
    pub const POST: u8 = 1 << 2;
    pub const PUT: u8 = 1 << 3;
    pub const PATCH: u8 = 1 << 4;
    pub const DELETE: u8 = 1 << 5;
    pub const OPTIONS: u8 = 1 << 6;

    pub fn from_method(method: &str) -> Self {
        match method {
            "GET" => Self(Self::GET | Self::HEAD),
            "HEAD" => Self(Self::HEAD),
            "POST" => Self(Self::POST),
            "PUT" => Self(Self::PUT),
            "PATCH" => Self(Self::PATCH),
            "DELETE" => Self(Self::DELETE),
            "OPTIONS" => Self(Self::OPTIONS),
            _ => Self(0),
        }
    }

    pub fn insert_method(&mut self, method: &str) {
        self.0 |= Self::from_method(method).0;
    }

    pub fn to_vec(self) -> Vec<String> {
        const ORDER: [(&str, u8); 7] = [
            ("GET", MethodMask::GET),
            ("HEAD", MethodMask::HEAD),
            ("POST", MethodMask::POST),
            ("PUT", MethodMask::PUT),
            ("PATCH", MethodMask::PATCH),
            ("DELETE", MethodMask::DELETE),
            ("OPTIONS", MethodMask::OPTIONS),
        ];
        let mut out = Vec::with_capacity(7);
        for (name, flag) in ORDER {
            if (self.0 & flag) != 0 {
                out.push(name.to_string());
            }
        }
        out
    }
}

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
    pub all_paths: Router<MethodMask>,
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

/// A single route dependency definition.
#[derive(Clone)]
pub struct DependencyEntry {
    pub name: String,
    pub factory: Py<PyAny>,
    pub is_async: bool,
    pub wants_request: bool,
    pub factory_params: HashSet<String>,
    pub factory_varkw: bool,
}

/// Auxiliary / cold metadata for a route.
#[derive(Clone)]
pub struct RouteExtra {
    pub path_template: String,
    pub jwt_cookie: Option<String>,
    pub jwt_decoding_key: Option<Arc<jsonwebtoken::DecodingKey>>,
    pub jwt_validation: Option<Arc<jsonwebtoken::Validation>>,
    pub dependencies: Arc<[DependencyEntry]>,
    pub handler_param_names: Arc<HashSet<String>>,
    pub body_param_name: String,
}

/// Compact 32-byte route entry fitting comfortably inside a single 64-byte L1D cache line.
#[derive(Clone)]
#[repr(C)]
pub struct RouteEntry {
    pub handler: Py<PyAny>,
    pub body_model: Option<Py<PyAny>>,
    pub extra: Arc<RouteExtra>,
    pub is_async: bool,
    pub require_jwt: bool,
    pub read_json_body: bool,
    pub read_form_body: bool,
    pub handler_varkw: bool,
    pub trivial_sync: bool,
}

const _: () = assert!(std::mem::size_of::<RouteEntry>() <= 64);

/// True when the route can be served by [`try_rsgi_sync_short_circuit`](crate::dispatch::try_rsgi_sync_short_circuit)
/// without body read, JWT, or dependency resolution.
pub fn route_is_trivial_sync(entry: &RouteEntry) -> bool {
    entry.trivial_sync
}

pub type ExceptionHandlerList = Arc<Vec<(Py<pyo3::types::PyType>, Py<PyAny>, bool)>>;

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
    pub openapi: Mutex<(serde_json::Value, Option<Arc<String>>)>,
    /// When `Some`, route matching uses these tables without taking per-router mutexes
    /// (populated in [`App::freeze`](crate::App::freeze)).
    pub compiled: Option<Arc<CompiledRouters>>,
    /// When true, `add_route` fails (DAG / routes frozen for DI).
    pub frozen: bool,
    /// Serve `GET /openapi.json` from the built document without a user route.
    pub include_openapi: bool,
    /// Stack of `(scope, protocol) -> None | Response | dict` request hooks. Return ``None`` to continue to routing.
    pub request_middleware: Arc<Vec<Py<PyAny>>>,
    /// Stack of `(scope, response_dict) -> Response | dict` response hooks. Runs before CORS/Security headers.
    pub response_middleware: Arc<Vec<Py<PyAny>>>,
    pub exception_handlers: ExceptionHandlerList,
    /// Optional Python CORS config (e.g. :class:`oxyroute.cors.CORSConfig`) for response headers.
    pub cors: Option<Py<PyAny>>,
    /// Optional :class:`oxyroute.security_headers.SecurityHeadersConfig` (or compatible
    /// ``response_header_pairs``) merged if those header names are not already set.
    pub security_headers: Option<Py<PyAny>>,
    /// Global connection pool for the Postgres database.
    pub db_pool: Option<sqlx::PgPool>,
    /// Bitmask of allowed HTTP methods per registered path template.
    pub path_method_masks: Mutex<std::collections::HashMap<String, MethodMask>>,
}

impl AppState {
    pub fn new() -> Self {
        let openapi = serde_json::json!({
            "openapi": "3.0.0",
            "info": { "title": "OxyRoute", "version": "0.5.0" },
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
            openapi: Mutex::new((openapi, None)),
            compiled: None,
            frozen: false,
            include_openapi: true,
            request_middleware: Arc::new(Vec::new()),
            response_middleware: Arc::new(Vec::new()),
            exception_handlers: Arc::new(Vec::new()),
            cors: None,
            security_headers: None,
            db_pool: None,
            path_method_masks: Mutex::new(std::collections::HashMap::new()),
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
            request_middleware: Arc::clone(&self.request_middleware),
            response_middleware: Arc::clone(&self.response_middleware),
            exception_handlers: Arc::clone(&self.exception_handlers),
            include_openapi: self.include_openapi,
            db_pool: self.db_pool.clone(),
        }
    }

    /// Clone current mutex-protected [`Router`]s into a snapshot (used at freeze / tests).
    pub fn snapshot_routers(&self) -> CompiledRouters {
        let mut all_paths = Router::new();
        for (path, mask) in self.path_method_masks.lock().iter() {
            let _ = all_paths.insert(path, *mask);
        }
        CompiledRouters {
            get: self.get.lock().clone(),
            post: self.post.lock().clone(),
            put: self.put.lock().clone(),
            patch: self.patch.lock().clone(),
            delete: self.delete.lock().clone(),
            options: self.options.lock().clone(),
            websocket: self.websocket.lock().clone(),
            all_paths,
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
    pub request_middleware: Arc<Vec<Py<PyAny>>>,
    pub response_middleware: Arc<Vec<Py<PyAny>>>,
    pub exception_handlers: ExceptionHandlerList,
    pub include_openapi: bool,
    pub db_pool: Option<sqlx::PgPool>,
}

/// Lookup a WebSocket route in a precomputed [`CompiledRouters`] (lock-free).
pub fn match_ws_route_compiled(
    compiled: &CompiledRouters,
    path: &str,
) -> Option<(usize, Vec<(String, String)>)> {
    compiled.websocket.at(path).ok().map(|m| {
        let mut pmap = Vec::new();
        for (k, v) in m.params.iter() {
            pmap.push((k.to_string(), v.to_string()));
        }
        (*m.value, pmap)
    })
}

/// Lookup an HTTP route in a precomputed [`CompiledRouters`] (lock-free).
///
/// Returns ``None`` for unsupported method, ``Some(None)`` for no match, ``Some(Some(...))`` on hit.
#[allow(clippy::type_complexity)]
pub fn match_route_compiled(
    compiled: &CompiledRouters,
    method: &str,
    path: &str,
) -> Option<Option<(usize, Vec<(String, String)>)>> {
    let g = router_for_compiled(compiled, method)?;
    Some(g.at(path).ok().map(|m| {
        let mut pmap = Vec::new();
        for (k, v) in m.params.iter() {
            pmap.push((k.to_string(), v.to_string()));
        }
        (*m.value, pmap)
    }))
}

/// All HTTP methods that match `path` in a precomputed [`CompiledRouters`] (lock-free 405 list).
pub fn methods_matching_path_compiled(compiled: &CompiledRouters, path: &str) -> Vec<String> {
    if let Ok(m) = compiled.all_paths.at(path) {
        let v = m.value.to_vec();
        if !v.is_empty() {
            return v;
        }
    }
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
    if let Some(c) = &state.compiled {
        methods_matching_path_compiled(c, path)
    } else {
        let compiled = state.snapshot_routers();
        methods_matching_path_compiled(&compiled, path)
    }
}

/// Returns route index and path params, or `None` if the method is unsupported; `Some(None)` if
/// method is valid but path did not match.
#[cfg(test)]
fn match_route(
    state: &AppState,
    method: &str,
    path: &str,
) -> Option<Option<(usize, Vec<(String, String)>)>> {
    if let Some(c) = &state.compiled {
        return match_route_compiled(c, method, path);
    }
    let g = map_method_router(state, method)?;
    Some(g.at(path).ok().map(|m| {
        let mut pmap = Vec::new();
        for (k, v) in m.params.iter() {
            pmap.push((k.to_string(), v.to_string()));
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
        assert_eq!(
            inner
                .1
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.as_str()),
            Some("5")
        );
    }

    #[test]
    fn methods_matching_path_uses_compiled() {
        let mut s = AppState::new();
        s.post.lock().insert("/x", 0usize).unwrap();
        s.path_method_masks
            .lock()
            .entry("/x".to_string())
            .or_default()
            .insert_method("POST");
        s.compiled = Some(Arc::new(s.snapshot_routers()));
        let m = methods_matching_path(&s, "/x");
        assert_eq!(m, vec!["POST".to_string()]);
    }

    #[test]
    fn test_route_entry_cacheline_packing() {
        let size = std::mem::size_of::<RouteEntry>();
        assert!(
            size <= 64,
            "RouteEntry size must be <= 64 bytes for L1D cacheline packing, got {size}"
        );
    }
}
