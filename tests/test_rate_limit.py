import asyncio

import httpx
from oxyroute import APIRouter, App
from oxyroute.testing import asgi_test_app


def test_native_token_bucket_rate_limiter() -> None:
    app = App()

    @app.get("/limited", rate_limit="2/minute")
    def handle_limited() -> dict[str, str]:
        return {"status": "ok"}

    @app.get("/custom-header", rate_limit="1/minute", rate_limit_key="header:x-api-key")
    def handle_custom_header() -> dict[str, str]:
        return {"auth": "keyed"}

    router = APIRouter()

    @router.get("/sub", rate_limit="2/minute")
    def handle_sub() -> dict[str, str]:
        return {"sub": "router"}

    app.include_router(router, prefix="/api")

    async def _run() -> None:
        transport = httpx.ASGITransport(app=asgi_test_app(app))
        async with httpx.AsyncClient(transport=transport, base_url="http://test") as c:
            # 1. First 2 requests succeed
            r1 = await c.get("/limited")
            assert r1.status_code == 200
            assert r1.json() == {"status": "ok"}

            r2 = await c.get("/limited")
            assert r2.status_code == 200
            assert r2.json() == {"status": "ok"}

            # 3rd request exceeds limit -> 429
            r3 = await c.get("/limited")
            assert r3.status_code == 429
            assert r3.json() == {"detail": "Too Many Requests"}
            assert r3.headers.get("ratelimit-limit") == "2"
            assert r3.headers.get("ratelimit-remaining") == "0"
            assert int(r3.headers.get("ratelimit-reset", "0")) >= 1
            assert int(r3.headers.get("retry-after", "0")) >= 1

            # Different IP (via x-forwarded-for) is not blocked
            r_other_ip = await c.get("/limited", headers={"x-forwarded-for": "198.51.100.1"})
            assert r_other_ip.status_code == 200

            # 2. Header-keyed rate limiting
            rh1 = await c.get("/custom-header", headers={"x-api-key": "client-A"})
            assert rh1.status_code == 200

            rh2 = await c.get("/custom-header", headers={"x-api-key": "client-A"})
            assert rh2.status_code == 429
            assert rh2.headers.get("ratelimit-limit") == "1"

            # client-B is separate
            rh_b = await c.get("/custom-header", headers={"x-api-key": "client-B"})
            assert rh_b.status_code == 200

            # 3. Router route rate limiting
            rr1 = await c.get("/api/sub")
            assert rr1.status_code == 200
            rr2 = await c.get("/api/sub")
            assert rr2.status_code == 200
            rr3 = await c.get("/api/sub")
            assert rr3.status_code == 429

    asyncio.run(_run())
