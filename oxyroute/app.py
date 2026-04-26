from __future__ import annotations

from typing import Any, Callable, List, Optional, Tuple, TypeVar, Union

from . import _oxyroute
from .asgi import build_asgi_caller

F = TypeVar("F", bound=Callable[..., Any])
Dep = Union[Callable[..., Any], _oxyroute.PyDepends]


def _unwrap_dep(f: Dep) -> Any:
    if isinstance(f, _oxyroute.PyDepends):
        return f.dependency()
    return f


def _norm_dependencies(
    deps: Optional[List[Tuple[str, Dep]]],
) -> Optional[List[Tuple[str, Any]]]:
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
    """

    def __init__(self, title: str = "OxyRoute", *, include_openapi: bool = True) -> None:
        self._app = _oxyroute.App(include_openapi=include_openapi)
        self._app.set_openapi_title(title)
        self.title = title
        self._asgi3: Callable[..., Any] = build_asgi_caller(self)

    def freeze(self) -> None:
        """After ``freeze()``, no more route registration (matches Rust app state)."""
        self._app.freeze()

    def set_openapi_served(self, enabled: bool) -> None:
        """Enable or disable the built-in ``GET /openapi.json`` route."""
        self._app.set_openapi_served(enabled)

    def get(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: Optional[str] = None,
        algorithms: Optional[List[str]] = None,
        dependencies: Optional[List[Tuple[str, Dep]]] = None,
    ) -> Callable[[F], F]:
        return self._route(
            "GET", path, require_jwt, jwt_secret, algorithms, read_json_body=False, dependencies=dependencies
        )

    def post(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: Optional[str] = None,
        algorithms: Optional[List[str]] = None,
        read_json_body: bool = True,
        dependencies: Optional[List[Tuple[str, Dep]]] = None,
    ) -> Callable[[F], F]:
        return self._route(
            "POST",
            path,
            require_jwt,
            jwt_secret,
            algorithms,
            read_json_body,
            dependencies=dependencies,
        )

    def put(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: Optional[str] = None,
        algorithms: Optional[List[str]] = None,
        dependencies: Optional[List[Tuple[str, Dep]]] = None,
    ) -> Callable[[F], F]:
        return self._route(
            "PUT", path, require_jwt, jwt_secret, algorithms, read_json_body=True, dependencies=dependencies
        )

    def delete(
        self,
        path: str,
        *,
        require_jwt: bool = False,
        jwt_secret: Optional[str] = None,
        algorithms: Optional[List[str]] = None,
        dependencies: Optional[List[Tuple[str, Dep]]] = None,
    ) -> Callable[[F], F]:
        return self._route(
            "DELETE", path, require_jwt, jwt_secret, algorithms, read_json_body=False, dependencies=dependencies
        )

    def _route(
        self,
        method: str,
        path: str,
        require_jwt: bool,
        jwt_secret: Optional[str],
        algorithms: Optional[List[str]],
        read_json_body: bool,
        dependencies: Optional[List[Tuple[str, Dep]]],
    ) -> Callable[[F], F]:
        dlist = _norm_dependencies(dependencies)

        def wrap(handler: F) -> F:
            self._app.add_route(
                method,
                path,
                handler,
                require_jwt,
                jwt_secret,
                algorithms,
                read_json_body,
                dlist,
            )
            return handler

        return wrap

    async def __rsgi_init__(self, *args: Any, **kwargs: Any) -> None:  # noqa: D401
        """Lifespan hook (no-op). Granian may pass extra positional args; accept **kwargs."""
        return None

    async def __rsgi_del__(self, *args: Any, **kwargs: Any) -> None:  # noqa: D401
        """Lifespan teardown (no-op). Accept extra args for Granian compatibility."""
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
