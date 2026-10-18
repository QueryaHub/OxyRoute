import pytest
from oxyroute import App


@pytest.mark.anyio
async def test_database_pool_failure():
    app = App()

    # Attempting to connect to a non-existent database should fail fast
    with pytest.raises(RuntimeError, match="DB connect error:"):
        await app.setup_database(
            "postgresql://foo:bar@127.0.0.1:12345/baz", acquire_timeout_secs=0.5
        )


@pytest.mark.anyio
@pytest.mark.parametrize(
    "option", ["acquire_timeout_secs", "idle_timeout_secs", "max_lifetime_secs"]
)
@pytest.mark.parametrize("bad", [-1.0, float("nan"), float("inf")])
async def test_database_pool_rejects_bad_durations(option, bad):
    app = App()

    # Negative or non-finite durations must raise a ValueError, not panic in Rust
    with pytest.raises(ValueError, match=option):
        await app.setup_database("postgresql://foo:bar@127.0.0.1:12345/baz", **{option: bad})


@pytest.mark.anyio
async def test_database_pool_teardown_no_op():
    app = App()

    # Calling close without setup should be a no-op and not raise any errors
    await app.close_database()

    # __rsgi_del__ should also gracefully do nothing if no db was set up
    await app.__rsgi_del__()


@pytest.mark.anyio
async def test_database_pool_acquire_timeout_is_applied():
    import time

    app = App()
    start = time.monotonic()
    with pytest.raises(RuntimeError, match="DB connect error:"):
        await app.setup_database(
            "postgresql://foo:bar@127.0.0.1:12345/baz", acquire_timeout_secs=0.2
        )
    # The default is 3.0s, so finishing well under that shows the option applied.
    assert time.monotonic() - start < 2.0
