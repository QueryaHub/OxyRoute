"""RSGI lifespan hooks: subclassing ``App`` (issue #18)."""

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
        await a.__rsgi_init__()
        await a.__rsgi_del__()

    asyncio.run(_go())


def test_subclass_rsgi_init_can_set_state() -> None:
    class WorkerApp(App):
        async def __rsgi_init__(self, *args, **kwargs) -> None:
            self.marker = 7
            return None

    async def _go() -> None:
        a = WorkerApp()
        assert not hasattr(a, "marker")
        await a.__rsgi_init__()
        assert a.marker == 7

    asyncio.run(_go())
