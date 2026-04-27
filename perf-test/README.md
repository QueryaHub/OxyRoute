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

## Run

From repository root:

```bash
bash perf-test/bench.sh
```

The script prints per-run metrics plus average/median RPS and relative delta.
