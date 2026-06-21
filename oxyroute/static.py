import mimetypes
import os
from typing import Any

from oxyroute.exceptions import HTTPException


class StaticFiles:
    """
    Serve static files from a directory.

    Can be mounted via:
        app.mount("/static", StaticFiles("static", html=True))
    """

    __name__ = "StaticFiles"

    def __init__(
        self,
        directory: str,
        html: bool = False,
        max_age: int | None = None,
    ) -> None:
        self.directory = os.path.abspath(directory)
        if not os.path.isdir(self.directory):
            raise RuntimeError(f"Directory {directory} does not exist")
        self.html = html
        self.max_age = max_age

    def __call__(self, protocol: Any, path: str = "") -> Any:
        if ".." in path.split("/"):
            raise HTTPException(status_code=403, detail="Forbidden")

        file_path = os.path.abspath(os.path.join(self.directory, path.lstrip("/")))

        if not file_path.startswith(self.directory):
            raise HTTPException(status_code=403, detail="Forbidden")

        if not os.path.exists(file_path) or not os.path.isfile(file_path):
            if self.html and os.path.isfile(os.path.join(file_path, "index.html")):
                file_path = os.path.join(file_path, "index.html")
            else:
                raise HTTPException(status_code=404, detail="Not Found")

        content_type, _ = mimetypes.guess_type(file_path)
        if content_type is None:
            content_type = "application/octet-stream"

        headers = [("content-type", content_type)]
        if self.max_age is not None:
            headers.append(("cache-control", f"public, max-age={self.max_age}"))

        if hasattr(protocol, "response_file"):
            # Rust fast path via tokio-fs
            protocol.response_file(200, headers, file_path)
            from oxyroute.streaming import stream_done

            return stream_done()
        else:
            # Fallback for Python testing transports without response_file
            with open(file_path, "rb") as f:
                body = f.read()
            protocol.response_bytes(200, headers, body)
            from oxyroute.streaming import stream_done

            return stream_done()
