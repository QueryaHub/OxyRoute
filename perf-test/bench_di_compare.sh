#!/usr/bin/env bash
# bench_di_compare.sh – measure DI/DX overhead across dependency depths.
#
# Routes in app_di_compare.py:
#   /nodep        baseline, zero deps
#   /dep1         one sync Depends
#   /dep3         three independent sync Depends
#   /depchain     chain of 3 sync Depends (a→b→c)
#   /depasync     one async Depends
#   /dep1json     one sync Depends + JSON body read
#
# Usage:
#   ./perf-test/bench_di_compare.sh
#
# Env knobs (same as other bench scripts):
#   OXYROUTE_BENCH_DURATION     default: 5s
#   OXYROUTE_BENCH_THREADS      default: 2
#   OXYROUTE_BENCH_CONNECTIONS  default: 32
#   OXYROUTE_BENCH_WORKERS      default: 1
#
# Requirements: granian, wrk, editable oxyroute install.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ -n "${PYTHON:-}" ]]; then
  :
elif [[ -x "${ROOT}/.venv/bin/python" ]]; then
  PYTHON="${ROOT}/.venv/bin/python"
else
  PYTHON="python3"
fi

DURATION="${OXYROUTE_BENCH_DURATION:-5s}"
THREADS="${OXYROUTE_BENCH_THREADS:-2}"
CONN="${OXYROUTE_BENCH_CONNECTIONS:-32}"
WORKERS="${OXYROUTE_BENCH_WORKERS:-1}"

if ! command -v wrk >/dev/null 2>&1; then
  echo "wrk not found – falling back to bench_di_compare.py (httpx-based)"
  exec "${PYTHON}" "$(dirname "${BASH_SOURCE[0]}")/bench_di_compare.py" \
    --duration "${DURATION%s}" \
    --concurrency "${CONN}" \
    --workers "${WORKERS}"
fi

# ── helpers ────────────────────────────────────────────────────────────────────

_free_port() {
  "${PYTHON}" -c "import socket; s=socket.socket(); s.bind(('127.0.0.1',0)); print(s.getsockname()[1]); s.close()"
}

_wait_http() {
  local port="$1" path="${2:-/nodep}"
  local deadline=$((SECONDS + 30))
  while (( SECONDS < deadline )); do
    if "${PYTHON}" -c \
      "import urllib.request; urllib.request.urlopen('http://127.0.0.1:${port}${path}', timeout=0.5).read()" \
      2>/dev/null; then
      return 0
    fi
    sleep 0.05
  done
  return 1
}

_run_wrk() {
  local url="$1"; shift
  wrk -t"${THREADS}" -c"${CONN}" -d"${DURATION}" "$@" "${url}" 2>&1 \
    | awk '/Requests\/sec:/{gsub(/^[ \t]+/,"",$2); print $2; exit}'
}

_start_server() {
  local port="$1"
  (
    export PYTHONPATH="${ROOT}"
    cd "${ROOT}/perf-test"
    exec "${PYTHON}" -m granian "app_di_compare:app" \
      --host 127.0.0.1 --port "${port}" --interface rsgi --workers "${WORKERS}"
  ) >/dev/null 2>&1 &
  echo $!
}

# ── single benchmark pass ───────────────────────────────────────────────────────

declare -A RESULTS   # name → rps

_bench_route() {
  local label="$1" path="$2" pid="$3" port="$4"
  shift 4
  local rps
  rps=$(_run_wrk "http://127.0.0.1:${port}${path}" "$@")
  RESULTS["${label}"]="${rps}"
}

# ── JSON lua snippet ────────────────────────────────────────────────────────────

_lua_json() {
  cat >"$1" <<'LUA'
wrk.method = "POST"
wrk.body   = '{"x":1,"y":"hello"}'
wrk.headers["Content-Type"] = "application/json"
LUA
}

# ── pretty print table ─────────────────────────────────────────────────────────

_print_table() {
  local baseline="${RESULTS[nodep]:-0}"

  printf '\n%-18s %12s %10s %10s\n' "scenario" "req/s" "vs baseline" "overhead%"
  printf '%s\n' "------------------------------------------------------------"

  local order=(nodep dep1 dep3 depchain depasync dep1json)
  for key in "${order[@]}"; do
    local rps="${RESULTS[$key]:-n/a}"
    if [[ "${rps}" == "n/a" ]]; then
      printf '%-18s %12s %10s %10s\n' "${key}" "n/a" "-" "-"
      continue
    fi
    local ratio overhead
    ratio=$(  "${PYTHON}" -c "b=float('${baseline}'); r=float('${rps}'); print(f'{r/b:.3f}x' if b>0 else 'n/a')")
    overhead=$("${PYTHON}" -c "b=float('${baseline}'); r=float('${rps}'); print(f'{(r/b-1)*100:+.1f}%' if b>0 else 'n/a')")
    printf '%-18s %12s %10s %10s\n' "${key}" "${rps}" "${ratio}" "${overhead}"
  done
  printf '\n'
}

# ── main ────────────────────────────────────────────────────────────────────────

main() {
  echo "bench_di_compare: OxyRoute DI/DX overhead (RSGI via Granian)"
  echo "  duration=${DURATION} threads=${THREADS} connections=${CONN} workers=${WORKERS}"
  echo ""

  local port pid
  port="$(_free_port)"
  pid="$(_start_server "${port}")"

  # Make sure server is up before benching any route.
  if ! _wait_http "${port}" "/nodep"; then
    kill "${pid}" 2>/dev/null || true
    wait "${pid}"  2>/dev/null || true
    echo "error: server did not become ready" >&2
    exit 1
  fi

  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "${tmp}"; kill "${pid}" 2>/dev/null || true; wait "${pid}" 2>/dev/null || true' EXIT

  echo "  [1/6] nodep       GET /nodep"
  _bench_route "nodep"    "/nodep"    "${pid}" "${port}"

  echo "  [2/6] dep1        GET /dep1"
  _bench_route "dep1"     "/dep1"     "${pid}" "${port}"

  echo "  [3/6] dep3        GET /dep3"
  _bench_route "dep3"     "/dep3"     "${pid}" "${port}"

  echo "  [4/6] depchain    GET /depchain"
  _bench_route "depchain" "/depchain" "${pid}" "${port}"

  echo "  [5/6] depasync    GET /depasync"
  _bench_route "depasync" "/depasync" "${pid}" "${port}"

  echo "  [6/6] dep1json    POST /dep1json (JSON body)"
  _lua_json "${tmp}/json.lua"
  _bench_route "dep1json" "/dep1json" "${pid}" "${port}" -s "${tmp}/json.lua"

  _print_table
}

main "$@"
