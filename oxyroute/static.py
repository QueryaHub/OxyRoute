import mimetypes
import os
from pathlib import Path
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
        directory: str | os.PathLike[str],
        html: bool = False,
        max_age: int | None = None,
    ) -> None:
        self.directory = Path(directory).resolve()
        if not self.directory.is_dir():
            raise RuntimeError(f"Directory {directory} does not exist")
        self.html = html
        self.max_age = max_age

    def __call__(self, protocol: Any, path: str = "") -> Any:
        if ".." in path.split("/"):
            raise HTTPException(status_code=403, detail="Forbidden")

        rel_path = path.lstrip("/")
        try:
            target_path = (self.directory / rel_path).resolve()
        except Exception:
            raise HTTPException(status_code=404, detail="Not Found") from None

        try:
            target_path.relative_to(self.directory)
        except ValueError:
            raise HTTPException(status_code=403, detail="Forbidden") from None

        if not target_path.exists() or not target_path.is_file():
            if self.html and (target_path / "index.html").is_file():
                target_path = target_path / "index.html"
            else:
                raise HTTPException(status_code=404, detail="Not Found")

        file_path = str(target_path)
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
