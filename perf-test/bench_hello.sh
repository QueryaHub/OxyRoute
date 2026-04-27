#!/usr/bin/env bash
# Compare OxyRoute (RSGI) vs FastAPI (ASGI) on a plain "hello" GET / using wrk.
# Requirements: granian, wrk, optional: `uv pip install -e ".[bench]"` for FastAPI.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DURATION="${OXYROUTE_BENCH_DURATION:-5s}"
THREADS="${OXYROUTE_BENCH_THREADS:-2}"
CONN="${OXYROUTE_BENCH_CONNECTIONS:-32}"
WORKERS="${OXYROUTE_BENCH_WORKERS:-1}"

if ! command -v wrk >/dev/null 2>&1; then
  echo "error: wrk not found (install wrk and retry)" >&2
  exit 1
fi

_free_port() {
  python3 -c "import socket; s=socket.socket(); s.bind(('127.0.0.1',0)); print(s.getsockname()[1]); s.close()"
}

_run_wrk() {
  local url="$1"
  wrk -t"${THREADS}" -c"${CONN}" -d"${DURATION}" "${url}" 2>&1 \
    | awk '/Requests\/sec:/{gsub(/^[ \t]+/,"",$2); print $2; exit}'
}

_wait_http() {
  local port="$1"
  local deadline=$((SECONDS + 30))
  while (( SECONDS < deadline )); do
    if python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:${port}/', timeout=0.5).read()" 2>/dev/null; then
      return 0
    fi
    sleep 0.05
  done
  return 1
}

_bench() {
  local name="$1" module_path="$2" iface="$3" port
  port="$(_free_port)"
  local -a cmd=(python3 -m granian "${module_path}" --host 127.0.0.1 --port "${port}" --interface "${iface}" --workers "${WORKERS}")
  # Silence server logs so command substitution only captures the numeric RPS line.
  (
    export PYTHONPATH="${ROOT}"
    cd "${ROOT}/perf-test"
    exec "${cmd[@]}"
  ) >/dev/null 2>&1 &
  local pid=$!
  if ! _wait_http "${port}"; then
    kill "${pid}" 2>/dev/null || true
    wait "${pid}" 2>/dev/null || true
    echo "error: server did not become ready (${name})" >&2
    exit 1
  fi
  local rps
  rps=$(_run_wrk "http://127.0.0.1:${port}/")
  kill "${pid}" 2>/dev/null || true
  wait "${pid}" 2>/dev/null || true
  echo "${rps}"
}

main() {
  echo "bench_hello: OxyRoute (rsgi) vs FastAPI (asgi)"
  echo "  duration=${DURATION} threads=${THREADS} connections=${CONN} workers=${WORKERS}"
  echo ""

  local oxy
  oxy=$(_bench "oxyroute" "app_oxyroute:app" "rsgi")
  local fa
  if ! python3 -c "import fastapi" 2>/dev/null; then
    echo "FastAPI not installed. From the repo root: uv sync --extra bench  (or uv pip install -e \".[bench]\")" >&2
    echo "OxyRoute Requests/sec: ${oxy}"
    exit 0
  fi
  fa=$(_bench "fastapi" "app_fastapi:app" "asgi")

  local pct
  pct=$(python3 -c "o=float('${oxy}'); f=float('${fa}');
if f <= 0:
  print('n/a')
else:
  print(f'{(o/f-1.0)*100.0:+.2f}')")

  echo "OxyRoute  RSGI  Requests/sec: ${oxy}"
  echo "FastAPI   ASGI  Requests/sec: ${fa}"
  echo "Delta (OxyRoute vs FastAPI): ${pct}%"
}

main "$@"
