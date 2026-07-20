"""
Composable sub-routers (issue #46): collect routes, then :meth:`oxyroute.app.App.include_router`.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from typing import Any, TypeVar

from . import _oxyroute

F = TypeVar("F", bound=Callable[..., Any])
Dep = Callable[..., Any] | _oxyroute.PyDepends


def join_path(prefix: str, path: str) -> str:
    """
    Join a URL path prefix and a route path. Both Granian and matchit use a leading ``/``.

    * ``"/v1"`` + ``"/a"`` → ``"/v1/a"``
    * ``""`` + ``"/a"`` → ``"/a"``
    * ``"/v1"`` + ``"/"`` → ``"/v1/"``
    """
    path = path.strip() if path else ""
    if not path:
        path = "/"
    elif not path.startswith("/"):
        path = "/" + path
    p = (prefix or "").strip()
    if not p:
        return path
    if not p.startswith("/"):
        p = "/" + p
    p = p.rstrip("/")
    if path == "/":
        return f"{p}/"
    return f"{p}{path}"


class APIRouter:
    """
    Register routes the same way as :class:`oxyroute.app.App`, then attach with
    ``app.include_router(router, prefix="/api/v1")``. Nested routers: ``router_a.include_router(router_b, prefix="…")``.
    """

    __slots__ = ("_routes",)

    def __init__(self) -> None:
        self._routes: list[tuple[str, str, Any, dict[str, Any]]] = []

    def _reg(self, method: str, path: str, **opts: Any) -> Callable[[F], F]:
        def dec(handler: F) -> F:
            # Drop ``None`` so ``include_router(..., tags=[...])`` defaults are not wiped.
            cleaned = {k: v for k, v in opts.items() if v is not None}
            self._routes.append((method, path, handler, cleaned))
            return handler

        return dec

    def include_router(
        self,
        router: APIRouter,
        prefix: str = "",
        **defaults: Any,
    ) -> None:
        """Copy routes from ``router`` into this router, prefixing paths."""
        for method, rel, handler, opts in router._routes:
            merged: dict[str, Any] = {**defaults, **opts}
            full = join_path(prefix, rel)
            self._routes.append((method, full, handler, merged))

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
        return self._reg(
            "GET",
            path,
            require_jwt=require_jwt,
            jwt_secret=jwt_secret,
            algorithms=algorithms,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            dependencies=dependencies,
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
        return self._reg(
            "POST",
            path,
            require_jwt=require_jwt,
            jwt_secret=jwt_secret,
            algorithms=algorithms,
            read_json_body=read_json_body,
            read_form_body=read_form_body,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            body_model=body_model,
            body_schema=body_schema,
            dependencies=dependencies,
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
        return self._reg(
            "PUT",
            path,
            require_jwt=require_jwt,
            jwt_secret=jwt_secret,
            algorithms=algorithms,
            read_json_body=read_json_body,
            read_form_body=read_form_body,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            body_model=body_model,
            body_schema=body_schema,
            dependencies=dependencies,
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
        return self._reg(
            "PATCH",
            path,
            require_jwt=require_jwt,
            jwt_secret=jwt_secret,
            algorithms=algorithms,
            read_json_body=read_json_body,
            read_form_body=read_form_body,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            body_model=body_model,
            body_schema=body_schema,
            dependencies=dependencies,
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
        return self._reg(
            "DELETE",
            path,
            require_jwt=require_jwt,
            jwt_secret=jwt_secret,
            algorithms=algorithms,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            dependencies=dependencies,
            tags=tags,
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
        tags: list[str] | None = None,
    ) -> Callable[[F], F]:
        return self._reg(
            "OPTIONS",
            path,
            require_jwt=require_jwt,
            jwt_secret=jwt_secret,
            algorithms=algorithms,
            jwt_issuer=jwt_issuer,
            jwt_audience=jwt_audience,
            jwt_leeway=jwt_leeway,
            jwt_cookie=jwt_cookie,
            dependencies=dependencies,
            tags=tags,
        )
