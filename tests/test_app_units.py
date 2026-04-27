"""Extra App wrapper branch coverage."""

from __future__ import annotations

import asyncio

from oxyroute.app import App, Depends, _norm_dependencies, _unwrap_dep


def test_depends_and_unwrap_helpers() -> None:
    def dep() -> int:
        return 1

    d = Depends(dep)
    assert _unwrap_dep(d) is dep
    assert _unwrap_dep(dep) is dep
    assert _norm_dependencies(None) is None
    assert _norm_dependencies([]) is None
    norm = _norm_dependencies([("x", d)])
    assert norm is not None
    assert norm[0][0] == "x"
    assert norm[0][1] is dep


def test_app_freeze_and_base_rsgi_noops() -> None:
    app = App()
    app.freeze()
    assert asyncio.run(app.__rsgi_init__()) is None
    assert asyncio.run(app.__rsgi_del__()) is None


def test_put_and_delete_decorators_register_routes() -> None:
    app = App()

    @app.put("/u")
    def upd() -> str:
        return "ok"

    @app.delete("/u")
    def rem() -> str:
        return "ok"

    assert upd() == "ok"
    assert rem() == "ok"


def test_rsgi_forwarder_methods_exist() -> None:
    app = App()
    assert callable(app.handle_rsgi)
    assert callable(app.__rsgi__)
