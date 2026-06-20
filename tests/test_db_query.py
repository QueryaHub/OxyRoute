import httpx
import pytest
from oxyroute import App, DBQuery, Depends
from tests._rsgi_test_transport import asgi_test_app


@pytest.mark.anyio
async def test_db_query_dependency_no_pool():
    app = App()

    def get_query() -> DBQuery:
        return DBQuery("SELECT 1 as num", [])

    @app.get("/query", dependencies=[("res", Depends(get_query))])
    def handle_query(res):
        return {"result": res}

    tr = httpx.ASGITransport(app=asgi_test_app(app))
    async with httpx.AsyncClient(transport=tr, base_url="http://test") as c:
        r = await c.get("/query")

    assert r.status_code == 500
