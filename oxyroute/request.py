from collections.abc import Iterator, Mapping
from typing import Any


class Request(Mapping[str, Any]):
    """
    A typed Request context object providing lazy access to headers and other scope properties.
    Preserves dictionary-like access (`request["headers"]`) for backwards compatibility.
    """

    def __init__(self, scope: Any, method: str, path: str, query_string: str) -> None:
        self.scope = scope
        self.method = method
        self.path = path
        self.query_string = query_string
        self._headers: dict[str, str] | None = None

    @property
    def headers(self) -> dict[str, str]:
        if self._headers is None:
            if isinstance(self.scope, dict):
                h = self.scope.get("headers", [])
                if isinstance(h, dict):
                    self._headers = {str(k): str(v) for k, v in h.items()}
                else:
                    self._headers = {k.decode("latin-1").lower(): v.decode("latin-1") for k, v in h}
            else:
                h = getattr(self.scope, "headers", {})
                if hasattr(h, "_d"):
                    h = h._d
                self._headers = dict(h)
        return self._headers

    @property
    def client(self) -> str | None:
        if isinstance(self.scope, dict):
            client = self.scope.get("client")
            if client:
                return f"{client[0]}:{client[1]}"
        else:
            return getattr(self.scope, "client", None)
        return None

    @property
    def cookies(self) -> dict[str, str]:
        # Minimal cookie parsing from headers
        cookie_header = self.headers.get("cookie")
        if not cookie_header:
            return {}
        cookies = {}
        for chunk in cookie_header.split(";"):
            if "=" in chunk:
                k, v = chunk.split("=", 1)
                cookies[k.strip()] = v.strip()
        return cookies

    def __getitem__(self, key: str) -> Any:
        if key == "headers":
            return self.headers
        if key == "method":
            return self.method
        if key == "path":
            return self.path
        if key == "query_string":
            return self.query_string
        raise KeyError(key)

    def __iter__(self) -> Iterator[str]:
        return iter(["method", "path", "query_string", "headers"])

    def __len__(self) -> int:
        return 4
