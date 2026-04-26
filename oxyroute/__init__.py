# Native extension first — prevents circular import with `app` importing `_oxyroute`.
import oxyroute._oxyroute  # noqa: F401
from oxyroute._oxyroute import decode_jwt_hs
from oxyroute.app import App, Depends
from oxyroute.cors import CORSConfig, apply_cors
from oxyroute.exceptions import HTTPException
from oxyroute.response import Response
from oxyroute.router import APIRouter

__all__ = [
    "APIRouter",
    "App",
    "CORSConfig",
    "Depends",
    "HTTPException",
    "apply_cors",
    "Response",
    "__version__",
    "decode_jwt_hs",
]
__version__ = "0.1.0"
