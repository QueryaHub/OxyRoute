"""RSGI lifespan hooks: ``on_startup`` / Granian-compatible sync init (issue #18 / #130)."""

from __future__ import annotations

import asyncio
from types import SimpleNamespace

from oxyroute import App


def test_app_state_is_simple_namespace() -> None:
    a = App()
    assert isinstance(a.state, SimpleNamespace)
    a.state.pool = "mock"
    assert a.state.pool == "mock"


def test_base_rsgi_init_and_del_are_noop() -> None:
    async def _go() -> None:
        a = App()
        # No-arg: returns coroutine (TestClient / await style).
        await a.__rsgi_init__()
        await a.__rsgi_del__()

    asyncio.run(_go())


def test_on_startup_sets_state() -> None:
    class WorkerApp(App):
        async def on_startup(self) -> None:
            self.marker = 7

    async def _go() -> None:
        a = WorkerApp()
        assert not hasattr(a, "marker")
        await a.__rsgi_init__()
        assert a.marker == 7

    asyncio.run(_go())


def test_granian_style_sync_init_with_non_running_loop() -> None:
    class WorkerApp(App):
        async def on_startup(self) -> None:
            self.state.ready = True

    a = WorkerApp()
    loop = asyncio.new_event_loop()
    try:
        # Granian: sync call with a non-running loop.
        result = a.__rsgi_init__(loop)
        assert result is None
        assert a.state.ready is True
        a.__rsgi_del__(loop)
    finally:
        loop.close()


def test_subclass_legacy_async_rsgi_init_still_awaitable() -> None:
    class WorkerApp(App):
        async def __rsgi_init__(self, *args, **kwargs) -> None:
            self.marker = 7
            return None

    async def _go() -> None:
        a = WorkerApp()
        await a.__rsgi_init__()
        assert a.marker == 7

    asyncio.run(_go())
