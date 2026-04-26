"""Sanity: example from oxyjwt-docs/getting-started decodes (library installed)."""

import time

import pytest

oxyjwt = pytest.importorskip("oxyjwt")


def test_getting_started_decode() -> None:
    secret = "change-me"
    payload = {
        "sub": "user-123",
        "role": "admin",
        "aud": "api",
        "iss": "auth-service",
        "exp": int(time.time()) + 3600,
    }
    token = oxyjwt.encode(payload, secret, algorithm="HS256")
    claims = oxyjwt.decode(
        token,
        secret,
        algorithms=["HS256"],
        audience="api",
        issuer="auth-service",
    )
    assert claims["sub"] == "user-123"
