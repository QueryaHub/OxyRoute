"""DI/DX performance comparison app for ``bench_di_compare.sh``.

Routes
------
GET /nodep          – no Depends at all (baseline)
GET /dep1           – one sync Depends factory
GET /dep3           – three independent sync Depends factories
GET /depchain       – chain: c depends on b depends on a (depth 3)
GET /depasync       – one async Depends factory
GET /dep1json       – one sync Depends + read JSON body
"""

from __future__ import annotations

import asyncio

from oxyroute import App, Depends

app = App(include_openapi=False)


# ---------- plain baseline ----------

@app.get("/nodep")
def nodep() -> str:
    return "ok"


# ---------- single sync dep ----------

def _make_int() -> int:
    return 7


@app.get("/dep1", dependencies=[("n", Depends(_make_int))])
def dep1(n: int) -> str:
    return f"n={n}"


# ---------- three independent sync deps ----------

def _a() -> int:
    return 1


def _b() -> int:
    return 2


def _c() -> int:
    return 3


@app.get(
    "/dep3",
    dependencies=[("a", Depends(_a)), ("b", Depends(_b)), ("c", Depends(_c))],
)
def dep3(a: int, b: int, c: int) -> str:
    return f"{a+b+c}"


# ---------- chained sync deps (depth 3) ----------

def _base() -> int:
    return 10


def _mid(base: int) -> int:
    return base * 2


def _top(mid: int) -> int:
    return mid + 1


@app.get(
    "/depchain",
    dependencies=[
        ("base", Depends(_base)),
        ("mid", Depends(_mid)),
        ("top", Depends(_top)),
    ],
)
def depchain(top: int) -> str:
    return f"{top}"


# ---------- async dep ----------

async def _async_val() -> str:
    await asyncio.sleep(0)
    return "async"


@app.get("/depasync", dependencies=[("v", Depends(_async_val))])
async def dep_async(v: str) -> str:
    return v


# ---------- dep + JSON body ----------

@app.post("/dep1json", dependencies=[("n", Depends(_make_int))], read_json_body=True)
def dep1json(n: int, json: dict) -> dict:
    return {"n": n, "keys": list(json.keys())}
