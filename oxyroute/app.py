from __future__ import annotations

import inspect
import json
from collections.abc import Callable, Mapping
from types import SimpleNamespace
from typing import Any, TypeVar

from . import _oxyroute
from .router import APIRouter, join_path

F = TypeVar("F", bound=Callable[..., Any])
Dep = Callable[..., Any] | _oxyroute.PyDepends


def _unwrap_dep(f: Dep) -> Any:
    if isinstance(f, _oxyroute.PyDepends):
        return f.dependency()
    return f


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
    ``__rsgi_init__`` or a factory, or on a subclass. In-memory data is not shared across
    Granian worker processes.
    """

    def __init__(self, title: str = "OxyRoute", *, include_openapi: bool = True) -> None:
        self._app = _oxyroute.App(include_openapi=include_openapi)
        self._app.set_openapi_title(title)
        self.title = title
        # Per-process mutable bag for ``__rsgi_init__`` / factory setup (DB pool, clients, …).
        self.state: SimpleNamespace = SimpleNamespace()

    def freeze(self) -> None:
        """After ``freeze()``, no more route registration (matches Rust app state)."""
        self._app.freeze()

    def set_openapi_served(self, enabled: bool) -> None:
        """Enable or disable the built-in ``GET /openapi.json`` route."""
        self._app.set_openapi_served(enabled)

    async def setup_database(self, url: str, max_connections: int = 10) -> None:
        """
        Connect to a PostgreSQL database and store the pool in the Rust hot path.
        Must be awaited (e.g. inside ``__rsgi_init__``).
        """
        await self._app.setup_database(url, max_connections)

    async def close_database(self) -> None:
        """
        Close the global PostgreSQL connection pool.
        Must be awaited (e.g. inside ``__rsgi_del__``).
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

    
    def add_exception_handler(self, exc_type: type[BaseException], handler: Callable[..., Any]) -> None:
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
            )
            return handler

        return wrap

    async def __rsgi_init__(self, *args: Any, **kwargs: Any) -> None:
        """
        RSGI worker startup (no-op in the base class). Subclass to open pools/clients; see
        ``docs/rsgi.md`` (Lifespan) and ``examples/rsgi_lifespan_app.py``.
        """
        return None

    async def __rsgi_del__(self, *args: Any, **kwargs: Any) -> None:
        """RSGI worker teardown. Closes the global connection pool if it exists."""
        await self.close_database()

    async def __rsgi__(self, scope: Any, protocol: Any) -> Any:
        """
        Granian awaits this coroutine. Native ``handle_rsgi`` may return ``None`` immediately
        (sync short-circuit for openapi / 404 / 405) or an awaitable (full async path).
        """
        r = self._app.handle_rsgi(scope, protocol)
        if r is None or not inspect.isawaitable(r):
            return r
        return await r

    def handle_rsgi(self, scope: Any, protocol: Any) -> Any:
        """Forward to the native ``handle_rsgi`` coroutine."""
        return self._app.handle_rsgi(scope, protocol)

    def openapi_json(self) -> str:
        return self._app.openapi_json()
