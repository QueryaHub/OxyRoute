import httpx
import pytest
from oxyroute import App, DBQuery, Depends
from oxyroute.testing import asgi_test_app


def test_db_query_constructor_and_attributes():
    # Safe positional parameter binding via tuple
    q1 = DBQuery("SELECT id, name FROM users WHERE id = $1", (42,))
    assert q1.query == "SELECT id, name FROM users WHERE id = $1"
    assert q1.args == (42,)

    # Safe positional parameter binding via list
    q2 = DBQuery("SELECT * FROM items WHERE price > $1 AND active = $2", [19.99, True])
    assert q2.query == "SELECT * FROM items WHERE price > $1 AND active = $2"
    assert q2.args == (19.99, True)

    # Empty args default
    q3 = DBQuery("SELECT 1")
    assert q3.args == ()
    assert q3.chunk_size is None

    # Chunk size configuration
    q4 = DBQuery("SELECT 1", chunk_size=100)
    assert q4.chunk_size == 100

    # Type error on invalid args
    with pytest.raises(TypeError, match="args must be a list or tuple"):
        DBQuery("SELECT 1", 123)  # type: ignore

    # Value error on non-positive chunk_size
    with pytest.raises(ValueError, match="chunk_size must be greater than 0"):
        DBQuery("SELECT 1", chunk_size=0)


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
