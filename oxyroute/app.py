from __future__ import annotations

import inspect
import json
from collections.abc import Callable, Mapping
from types import SimpleNamespace
from typing import Any, TypeVar

from . import _oxyroute
from .docs_ui import docs_html, normalize_docs_ui
from .response import Response
from .router import APIRouter, join_path

F = TypeVar("F", bound=Callable[..., Any])
Dep = Callable[..., Any] | _oxyroute.PyDepends


def _unwrap_dep(f: Dep) -> Any:
    if isinstance(f, _oxyroute.PyDepends):
        return f.dependency()
    return f


class _ProtocolWrapper:
    __slots__ = ("__oxyroute_path_template__", "_inner", "status")

    def __init__(self, inner: Any) -> None:
        self._inner = inner
        self.status: int = 500
        self.__oxyroute_path_template__: str = ""

    def __oxyroute_set_path_template__(self, template: str) -> None:
        self.__oxyroute_path_template__ = template

    def response_empty(self, status: int, headers: list[tuple[str, str]]) -> None:
        self.status = status
        self._inner.response_empty(status, headers)

    def response_str(self, status: int, headers: list[tuple[str, str]], body: str) -> None:
        self.status = status
        self._inner.response_str(status, headers, body)

    def response_bytes(self, status: int, headers: list[tuple[str, str]], body: bytes) -> None:
        self.status = status
        self._inner.response_bytes(status, headers, body)

    def response_file(self, status: int, headers: list[tuple[str, str]], file_path: str) -> None:
        self.status = status
        self._inner.response_file(status, headers, file_path)

    def response_stream(self, status: int, headers: list[tuple[str, str]]) -> Any:
        self.status = status
        return self._inner.response_stream(status, headers)


def _norm_dependencies(
    deps: list[tuple[str, Dep]] | None,
) -> list[tuple[str, Any]] | None:
    if not deps:
        return None
    return [(n, _unwrap_dep(c)) for n, c in deps]


def Depends(call: Callable[..., Any]) -> _oxyroute.PyDepends:
    """Marker for a dependency factory; used with ``dependencies=[("name", Depends(fn)), ...]``."""
    return _oxyroute.PyDepends(call)


class App:
    """
    Application object: pass a module instance to Granian, e.g.
    ``granian app:app --interface rsgi``. RSGI is the only supported transport;
    the legacy ASGI bridge was removed in v0.3.0.

    ``state`` is an empty ``types.SimpleNamespace`` for per-process data; set fields in
    ``on_startup`` / ``__rsgi_init__`` or a factory, or on a subclass. In-memory data is
    not shared across Granian worker processes.
    """

    def __init__(
        self,
        title: str = "OxyRoute",
        *,
        include_openapi: bool = True,
        docs_ui: str | None = None,
        openapi_description: str | None = None,
        openapi_contact: Mapping[str, Any] | None = None,
        openapi_servers: list[Mapping[str, Any]] | None = None,
        access_log_hook: Callable[[Any, int, float, str], None] | None = None,
    ) -> None:
        self._app = _oxyroute.App(include_openapi=include_openapi)
        self._app.set_openapi_title(title)
        self.title = title
        self.access_log_hook = access_log_hook
        # Per-process mutable bag for ``on_startup`` / factory setup (DB pool, clients, …).
        self.state: SimpleNamespace = SimpleNamespace()
        self._docs_ui: str | None = normalize_docs_ui(docs_ui)
        self._docs_mounted: bool = False
        if (
            openapi_description is not None
            or openapi_contact is not None
            or openapi_servers is not None
        ):
            self.set_openapi_info(
                description=openapi_description,
                contact=openapi_contact,
                servers=openapi_servers,
            )
        if self._docs_ui is not None:
            self.mount_docs("/docs", ui=self._docs_ui)

    def freeze(self) -> None:
        """After ``freeze()``, no more route registration (matches Rust app state)."""
        self._app.freeze()

    def set_openapi_served(self, enabled: bool) -> None:
        """Enable or disable the built-in ``GET /openapi.json`` route."""
        self._app.set_openapi_served(enabled)

    def set_openapi_info(
        self,
        *,
        description: str | None = None,
        contact: Mapping[str, Any] | None = None,
        servers: list[Mapping[str, Any]] | None = None,
    ) -> None:
        """
        Enrich the OpenAPI document ``info`` and optional ``servers`` list.

        ``contact`` is an OpenAPI contact object (e.g. ``{"name": "…", "email": "…"}``).
        ``servers`` is a list of ``{"url": "…", "description": "…"}`` objects.
        """
        contact_json = json.dumps(dict(contact)) if contact is not None else None
        servers_json = json.dumps([dict(s) for s in servers]) if servers is not None else None
        self._app.set_openapi_info(description, contact_json, servers_json)

    def mount_docs(
        self,
        path: str = "/docs",
        *,
        ui: str = "scalar",
        openapi_url: str = "/openapi.json",
    ) -> None:
        """
        Register ``GET path`` serving Scalar or Swagger UI against ``openapi_url``.

        UI assets load from a public CDN; set CSP ``script-src`` / ``style-src`` accordingly
        (or disable security-header presets that block CDN scripts on the docs route).
        """
        ui_n = normalize_docs_ui(ui)
        if ui_n is None:
            raise ValueError("ui is required")
        path = path.rstrip("/") or "/docs"
        html = docs_html(ui=ui_n, title=self.title, openapi_url=openapi_url)
        headers = {"content-type": "text/html; charset=utf-8"}

        def _docs() -> Response:
            return Response(body=html, status=200, headers=headers)

        self.get(path)(_docs)
        self._docs_ui = ui_n
        self._docs_mounted = True

    async def setup_database(self, url: str, max_connections: int = 10) -> None:
        """
        Connect to a PostgreSQL database and store the pool in the Rust hot path.
        Must be awaited (e.g. inside ``on_startup``).
        """
        await self._app.setup_database(url, max_connections)

    async def close_database(self) -> None:
        """
        Close the global PostgreSQL connection pool.
        Must be awaited (e.g. inside ``on_shutdown``).
        """
        await self._app.close_database()

    def include_router(
        self,
        router: APIRouter,
        prefix: str = "",
        **defaults: Any,
    ) -> None:
        """
        Mount routes from an :class:`APIRouter` on this app with an optional path prefix.
        Per-route options win over ``**defaults`` (same keys as the ``get`` / ``post`` / … decorators).
        """
        regmap: dict[str, Callable[..., Any]] = {
            "GET": App.get,
            "POST": App.post,
            "PUT": App.put,
            "PATCH": App.patch,
            "DELETE": App.delete,
            "OPTIONS": App.options,
        }
        for method, rel, handler, opts in router._routes:
            merged: dict[str, Any] = {**defaults, **opts}
            full = join_path(prefix, rel)
            reg = regmap[method]
            allowed = {p for p in inspect.signature(reg).parameters if p not in ("self", "path")}
            kw: dict[str, Any] = {k: v for k, v in merged.items() if k in allowed}
            reg(self, full, **kw)(handler)

    def add_exception_handler(
        self, exc_type: type[BaseException], handler: Callable[..., Any]
    ) -> None:
        """
        Register a global exception handler for a specific exception type.
        """
        self._app.add_exception_handler(exc_type, handler)

    def set_middleware(self, handler: Callable[..., Any] | None) -> None:
        """
        One optional pre-route callback ``(scope, protocol)`` — return ``None`` to pass through.

        For any other return value, the same rules apply as for route handlers
        (e.g. :class:`oxyroute.Response` or a ``dict`` with ``status`` / ``body`` / ``headers``);
        the response is sent and routing / body read is skipped. Runs **before** the request
        body is read (e.g. for CORS preflight).
        """
        self._app.set_middleware(handler)

    def set_cors(self, config: Any | None) -> None:
        """
        Optional CORS object (e.g. :class:`oxyroute.cors.CORSConfig`) for merging response
        headers. Used together with :func:`oxyroute.cors.apply_cors` or a custom
        :meth:`set_middleware` for preflight. Pass ``None`` to disable.
        """
        self._app.set_cors(config)

    def set_security_headers(self, config: Any | None) -> None:
        """
        Optional preset (e.g. :class:`oxyroute.security_headers.SecurityHeadersConfig`) — merged
        into responses when a header of the same name is not already set. Pass ``None`` to disable.
        """
        self._app.set_security_headers(config)

    def mount(self, path: str, app: Any) -> None:
        """Mount another application or handler at a specific path prefix."""
        path = path.rstrip("/")
        # Mount the exact prefix
        self.get(path)(app)
        self.get(path + "/")(app)
        # Mount all subpaths
        self.get(path + "/*path")(app)

    def get(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: str | None = None,
        algorithms: list[str] | None = None,
        jwt_issuer: str | None = None,
        jwt_audience: str | None = None,
        jwt_leeway: int | None = None,
        jwt_cookie: str | None = None,
        dependencies: list[tuple[str, Dep]] | None = None,
        tags: list[str] | None = None,
    ) -> Callable[[F], F]:
        return self._route(
            "GET",
            path,
            require_jwt,
            jwt_secret,
            algorithms,
            read_json_body=False,
            dependencies=dependencies,
            read_form_body=False,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            tags=tags,
        )

    def post(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: str | None = None,
        algorithms: list[str] | None = None,
        read_json_body: bool = True,
        read_form_body: bool = False,
        jwt_issuer: str | None = None,
        jwt_audience: str | None = None,
        jwt_leeway: int | None = None,
        jwt_cookie: str | None = None,
        body_model: Any | None = None,
        body_schema: Mapping[str, Any] | None = None,
        dependencies: list[tuple[str, Dep]] | None = None,
        tags: list[str] | None = None,
    ) -> Callable[[F], F]:
        return self._route(
            "POST",
            path,
            require_jwt,
            jwt_secret,
            algorithms,
            read_json_body,
            dependencies=dependencies,
            read_form_body=read_form_body,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            body_model=body_model,
            body_schema=body_schema,
            tags=tags,
        )

    def put(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: str | None = None,
        algorithms: list[str] | None = None,
        read_json_body: bool = True,
        read_form_body: bool = False,
        jwt_issuer: str | None = None,
        jwt_audience: str | None = None,
        jwt_leeway: int | None = None,
        jwt_cookie: str | None = None,
        body_model: Any | None = None,
        body_schema: Mapping[str, Any] | None = None,
        dependencies: list[tuple[str, Dep]] | None = None,
        tags: list[str] | None = None,
    ) -> Callable[[F], F]:
        return self._route(
            "PUT",
            path,
            require_jwt,
            jwt_secret,
            algorithms,
            read_json_body,
            dependencies=dependencies,
            read_form_body=read_form_body,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            body_model=body_model,
            body_schema=body_schema,
            tags=tags,
        )

    def patch(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: str | None = None,
        algorithms: list[str] | None = None,
        read_json_body: bool = True,
        read_form_body: bool = False,
        jwt_issuer: str | None = None,
        jwt_audience: str | None = None,
        jwt_leeway: int | None = None,
        jwt_cookie: str | None = None,
        body_model: Any | None = None,
        body_schema: Mapping[str, Any] | None = None,
        dependencies: list[tuple[str, Dep]] | None = None,
        tags: list[str] | None = None,
    ) -> Callable[[F], F]:
        return self._route(
            "PATCH",
            path,
            require_jwt,
            jwt_secret,
            algorithms,
            read_json_body,
            dependencies=dependencies,
            read_form_body=read_form_body,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            body_model=body_model,
            body_schema=body_schema,
            tags=tags,
        )

    def delete(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: str | None = None,
        algorithms: list[str] | None = None,
        jwt_issuer: str | None = None,
        jwt_audience: str | None = None,
        jwt_leeway: int | None = None,
        jwt_cookie: str | None = None,
        dependencies: list[tuple[str, Dep]] | None = None,
        tags: list[str] | None = None,
    ) -> Callable[[F], F]:
        return self._route(
            "DELETE",
            path,
            require_jwt,
            jwt_secret,
            algorithms,
            read_json_body=False,
            dependencies=dependencies,
            read_form_body=False,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            tags=tags,
        )

    def websocket(self, path: str) -> Callable[[F], F]:
        """
        Register a native RSGI WebSocket handler.

        ``path`` uses the same ``matchit`` syntax as HTTP routes (e.g. ``/ws/:room``).
        The handler receives a single :class:`oxyroute.WebSocket` argument; ``await``
        :meth:`oxyroute.WebSocket.accept` once before sending or receiving frames.

        Sync handlers run inline (the Rust dispatcher does not bridge them through Tokio
        twice); async handlers are awaited on Granian's loop.
        """

        def wrap(handler: F) -> F:
            self._app.add_websocket_route(path, handler)
            return handler

        return wrap

    def options(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: str | None = None,
        algorithms: list[str] | None = None,
        jwt_issuer: str | None = None,
        jwt_audience: str | None = None,
        jwt_leeway: int | None = None,
        jwt_cookie: str | None = None,
        dependencies: list[tuple[str, Dep]] | None = None,
        tags: list[str] | None = None,
    ) -> Callable[[F], F]:
        return self._route(
            "OPTIONS",
            path,
            require_jwt,
            jwt_secret,
            algorithms,
            read_json_body=False,
            dependencies=dependencies,
            read_form_body=False,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            tags=tags,
        )

    def _route(
        self,
        method: str,
        path: str,
        require_jwt: bool,
        jwt_secret: str | None,
        algorithms: list[str] | None,
        read_json_body: bool,
        dependencies: list[tuple[str, Dep]] | None,
        *,
        read_form_body: bool = False,
        jwt_issuer: str | None = None,
        jwt_audience: str | None = None,
        jwt_leeway: int | None = None,
        jwt_cookie: str | None = None,
        body_model: Any | None = None,
        body_schema: Mapping[str, Any] | None = None,
        tags: list[str] | None = None,
    ) -> Callable[[F], F]:
        dlist = _norm_dependencies(dependencies)

        def wrap(handler: F) -> F:
            if body_model is not None and body_schema is not None:
                raise TypeError("use only one of body_model and body_schema")
            rj = read_json_body
            if read_form_body:
                rj = False
            body_schema_json: str | None = None
            if body_model is not None:
                body_schema_json = json.dumps(body_model.model_json_schema())
            elif body_schema is not None:
                body_schema_json = json.dumps(body_schema)
            self._app.add_route(
                method,
                path,
                handler,
                require_jwt,
                jwt_secret,
                algorithms,
                rj,
                read_form_body,
                dlist,
                jwt_issuer,
                jwt_audience,
                jwt_leeway,
                jwt_cookie,
                body_schema_json,
                body_model,
                tags,
            )
            return handler

        return wrap

    @staticmethod
    def _run_lifespan(coro: Any, loop: Any | None) -> Any:
        """
        Granian calls sync ``__rsgi_init__(loop)`` / ``__rsgi_del__(loop)`` with a
        **non-running** event loop — use ``run_until_complete``. TestClient and
        ``await app.__rsgi_init__()`` pass no loop and receive the coroutine.
        """
        if loop is not None and hasattr(loop, "run_until_complete"):
            try:
                running = bool(loop.is_running())
            except Exception:
                running = False
            if not running:
                return loop.run_until_complete(coro)
        return coro

    async def on_startup(self) -> None:
        """
        Per-worker async startup. Override in a subclass; the base class runs this from
        sync :meth:`__rsgi_init__` under Granian.
        """
        return None

    async def on_shutdown(self) -> None:
        """Per-worker async teardown. Closes the global connection pool if it exists."""
        await self.close_database()

    def __rsgi_init__(self, loop: Any | None = None, *args: Any, **kwargs: Any) -> Any:
        """
        RSGI worker startup (Granian-compatible).

        Granian invokes this as a **sync** method with the worker ``loop`` (not running)
        and expects ``loop.run_until_complete(...)``. Prefer overriding :meth:`on_startup`
        instead of this method. When called with no ``loop`` (tests), returns the
        ``on_startup`` coroutine for the caller to await.
        """
        return self._run_lifespan(self.on_startup(), loop)

    def __rsgi_del__(self, loop: Any | None = None, *args: Any, **kwargs: Any) -> Any:
        """RSGI worker teardown; see :meth:`__rsgi_init__` / :meth:`on_shutdown`."""
        return self._run_lifespan(self.on_shutdown(), loop)

    async def __rsgi__(self, scope: Any, protocol: Any) -> Any:
        """
        Granian awaits this coroutine. Native ``handle_rsgi`` may return ``None`` immediately
        (sync short-circuit for openapi / 404 / 405) or an awaitable (full async path).
        """
        if self.access_log_hook:
            import time

            start = time.perf_counter_ns()
            p = _ProtocolWrapper(protocol)
            r = self._app.handle_rsgi(scope, p)
            if r is not None and inspect.isawaitable(r):
                await r
            dur = (time.perf_counter_ns() - start) / 1000000.0
            self.access_log_hook(scope, p.status, dur, p.__oxyroute_path_template__)
            return r

        r = self._app.handle_rsgi(scope, protocol)
        if r is None or not inspect.isawaitable(r):
            return r
        return await r

    def handle_rsgi(self, scope: Any, protocol: Any) -> Any:
        """Forward to the native ``handle_rsgi`` coroutine."""
        return self._app.handle_rsgi(scope, protocol)

    def openapi_json(self) -> str:
        return self._app.openapi_json()
