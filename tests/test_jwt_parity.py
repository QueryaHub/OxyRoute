"""Compare ``oxyroute.decode_jwt_hs`` (Rust) with ``oxyjwt.decode`` on the same token."""

from __future__ import annotations

import time

import pytest

from oxyroute import decode_jwt_hs

oxyjwt = pytest.importorskip("oxyjwt")


def test_jwt_oxyjwt_parity_hs256() -> None:
    secret = "parity-secret"
    now = int(time.time())
    payload = {
        "sub": "user-1",
        "iss": "test",
        "exp": now + 3600,
    }
    token = oxyjwt.encode(payload, secret, algorithm="HS256")
    a = oxyjwt.decode(
        token,
        secret,
        algorithms=["HS256"],
        issuer="test",
    )
    b = decode_jwt_hs(token, secret, ["HS256"])
    assert a["sub"] == b["sub"] == "user-1"
    assert a["iss"] == b["iss"] == "test"
