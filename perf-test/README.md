# perf-test

Reproducible micro-bench harness for OxyRoute vs FastAPI.

## Apps

- `app.py` -> OxyRoute hello endpoint (`GET /`)
- `fastapi_app.py` -> FastAPI hello endpoint (`GET /`)

Both return plain text `hello world` to keep payloads equivalent.

## Prerequisites

- `wrk` installed
- `granian` installed
- For FastAPI runs: `uv` (uses temporary dependency install via `--with fastapi`)

## Default benchmark profile

- Server tuning: `--workers 2 --runtime-mode mt --runtime-threads 1`
- Load profile: `wrk -t4 -c128 -d15s`
- Repetitions: `3`

## Run (full compare)

From repository root:

```bash
bash perf-test/bench.sh
```

The script prints per-run metrics plus average/median RPS and relative delta.

## Hello-world RPS (OxyRoute vs FastAPI)

Minimal comparison on `GET /` returning plain text, both served by
[Granian](https://github.com/emmett-framework/granian):

- OxyRoute: `--interface rsgi`
- FastAPI: `--interface asgi`

```bash
cd /path/to/OxyRoute
uv sync --all-extras
./perf-test/bench_hello.sh
```

Optional environment knobs for `bench_hello.sh`:

| Variable | Default | Meaning |
|----------|---------|---------|
| `OXYROUTE_BENCH_DURATION` | `5s` | `wrk -d` |
| `OXYROUTE_BENCH_THREADS` | `2` | `wrk -t` |
| `OXYROUTE_BENCH_CONNECTIONS` | `32` | `wrk -c` |
| `OXYROUTE_BENCH_WORKERS` | `1` | Granian `--workers` |

## Optional pytest (short run)

With `wrk` and `fastapi` available:

```bash
OXYROUTE_BENCH=1 uv run pytest tests/test_perf_hello_bench.py -m bench -v
```

By default the bench test is skipped (no load on normal `pytest`).
