from __future__ import annotations

import inspect
import json
from collections.abc import Callable, Mapping
from types import SimpleNamespace
from typing import Any, TypeVar

from . import _oxyroute
from .asgi import build_asgi_caller
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
    ``granian app:app --interface rsgi`` or for ASGI ``granian app:app --interface asgi``.

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
        self._asgi3: Callable[..., Any] = build_asgi_caller(self)

    def freeze(self) -> None:
        """After ``freeze()``, no more route registration (matches Rust app state)."""
        self._app.freeze()

    def set_openapi_served(self, enabled: bool) -> None:
        """Enable or disable the built-in ``GET /openapi.json`` route."""
        self._app.set_openapi_served(enabled)

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

    def set_middleware(self, handler: Callable[..., Any] | None) -> None:
        """
        One optional pre-route callback ``(scope, protocol)`` — return ``None`` to pass through.

        For any other return value, the same rules apply as for route handlers
        (e.g. :class:`oxyroute.Response` or a ``dict`` with ``status`` / ``body`` / ``headers``);
        the response is sent and routing / body read is skipped. Runs **before** the request
        body is read (e.g. for CORS preflight).
        """
        self._app.set_middleware(handler)

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
        """RSGI worker teardown (no-op in the base class)."""
        return None

    async def __rsgi__(self, scope: Any, protocol: Any) -> None:
        return await self._app.handle_rsgi(scope, protocol)

    def handle_rsgi(self, scope: Any, protocol: Any) -> Any:
        """Forward to the native app (used by the ASGI bridge)."""
        return self._app.handle_rsgi(scope, protocol)

    async def __call__(self, scope: Any, receive: Any, send: Any) -> None:
        return await self._asgi3(scope, receive, send)

    def openapi_json(self) -> str:
        return self._app.openapi_json()
