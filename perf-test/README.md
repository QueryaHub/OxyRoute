# perf-test

Reproducible load and micro-bench harness for OxyRoute (issue #110).

## Apps

| File | Purpose |
|------|---------|
| `app_oxyroute.py` | Minimal hello `GET /` (RSGI) for `bench_hello.sh` |
| `app_fastapi.py` | FastAPI hello for compare |
| `app_scenarios.py` | Multi-route app for `bench_scenarios.sh` (text, JSON, JWT, CORS, Depends) |
| `app.py` / `fastapi_app.py` | Older compare harness used by `bench.sh` |

## Prerequisites

- `wrk`
- `granian`
- Editable OxyRoute (`uv sync --extra dev --extra bench`)
- For FastAPI compare: FastAPI (bench extra)
- For JWT scenario token: PyJWT (bench extra)

## Criterion microbenchmarks (Rust)

Opt-in; **not** required in default CI. Measures hot-path primitives without wrk:

| Group | Cases |
|-------|--------|
| `match_route_compiled` | static `/hello`, param `/items/:id` |
| `map_handler_return` | Python `str` / `bytes` |
| `json_to_py` | small object, nested document |

```bash
# From repo root (needs a Python interpreter for PyO3 link)
cargo bench --bench hot_path
```

HTML reports land under `target/criterion/`. **Before opening a perf PR**, run the same bench on `dev` and on your branch and paste key numbers (or attach the report) so reviewers can see deltas.

## Hello-world RPS (OxyRoute vs FastAPI)

Minimal comparison on `GET /` returning plain text, both served by Granian:

- OxyRoute: `--interface rsgi` (`app_oxyroute.py`)
- FastAPI: `--interface asgi` (`app_fastapi.py`)

### Setup

```bash
cd /path/to/OxyRoute
uv sync --extra dev --extra bench
# wrk: sudo apt install wrk  /  brew install wrk
```

`bench_hello.sh` prefers `REPO/.venv/bin/python` when present.

### Run

```bash
./perf-test/bench_hello.sh
```

| Variable | Default | Meaning |
|----------|---------|---------|
| `OXYROUTE_BENCH_DURATION` | `5s` | `wrk -d` |
| `OXYROUTE_BENCH_THREADS` | `2` | `wrk -t` |
| `OXYROUTE_BENCH_CONNECTIONS` | `32` | `wrk -c` |
| `OXYROUTE_BENCH_WORKERS` | `1` | Granian `--workers` |

## Scenario suite (`bench_scenarios.sh`)

Hits routes on `app_scenarios.py`:

| Scenario | Path | Notes |
|----------|------|--------|
| `text` | `GET /` | Plain text |
| `json` | `POST /json` | JSON body + JSON response |
| `jwt` | `GET /jwt` | Bearer HS256 |
| `cors` | `GET /` | `Origin` header (CORS enabled on app) |
| `dep` | `GET /dep` | One `Depends` factory |

```bash
./perf-test/bench_scenarios.sh
# or one scenario:
OXYROUTE_BENCH_SCENARIO=json ./perf-test/bench_scenarios.sh
```

Same `OXYROUTE_BENCH_*` knobs as hello, plus `OXYROUTE_BENCH_SCENARIO` (`all` \| `text` \| `json` \| `jwt` \| `cors` \| `dep`).

## Optional pytest (short hello run)

```bash
OXYROUTE_BENCH=1 uv run pytest tests/test_perf_hello_bench.py -m bench -v
```

Skipped unless `OXYROUTE_BENCH=1` (not for default CI).

## Full compare (`bench.sh`)

Older multi-rep harness — see script header. Default profile uses higher connection counts than the hello script.

## Baseline checklist (perf PRs)

1. `git checkout dev && cargo bench --bench hot_path` (save summary)
2. Your branch: same command
3. Optionally `./perf-test/bench_scenarios.sh` with fixed `OXYROUTE_BENCH_DURATION` / connections
4. Paste before/after numbers in the PR
