# Native extension first — prevents circular import with `app` importing `_oxyroute`.
import oxyroute._oxyroute  # noqa: F401
from oxyroute._oxyroute import DBQuery, WebSocket, decode_jwt_hs
from oxyroute.app import App, Depends
from oxyroute.cors import CORSConfig, apply_cors
from oxyroute.csrf import CSRFConfig, apply_csrf, csrf_layer
from oxyroute.exceptions import HTTPException
from oxyroute.request import Request
from oxyroute.response import Response
from oxyroute.router import APIRouter
from oxyroute.security_headers import SecurityHeadersConfig
from oxyroute.sse import SSEEvent, send_sse, sse_done

__all__ = [
    "APIRouter",
    "App",
    "CORSConfig",
    "CSRFConfig",
    "DBQuery",
    "Depends",
    "HTTPException",
    "Request",
    "Response",
    "SSEEvent",
    "SecurityHeadersConfig",
    "WebSocket",
    "__version__",
    "apply_cors",
    "apply_csrf",
    "csrf_layer",
    "decode_jwt_hs",
    "send_sse",
    "sse_done",
]
__version__ = "0.4.0"
