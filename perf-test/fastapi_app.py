import contextlib
from typing import AsyncGenerator

from fastapi import FastAPI
from fastapi.responses import JSONResponse
import asyncpg

DB_URI = "postgresql://postgres:postgres@127.0.0.1:5433/postgres"


class AppState:
    def __init__(self):
        self.pool = None


state = AppState()


@contextlib.asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncGenerator[None, None]:
    state.pool = await asyncpg.create_pool(DB_URI, min_size=10, max_size=10)
    yield
    await state.pool.close()


app = FastAPI(title="Perf Test FastAPI DB", lifespan=lifespan)


@app.get("/test_db")
async def hello():
    async with state.pool.acquire() as conn:
        val = await conn.fetchval("SELECT 1 as num")
        return {"num": val}
