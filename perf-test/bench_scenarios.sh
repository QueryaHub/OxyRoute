#!/usr/bin/env bash
# Run wrk against OxyRoute scenario routes (issue #110).
# Requirements: granian, wrk, editable oxyroute (and PyJWT for the JWT scenario token).
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
SCENARIO="${OXYROUTE_BENCH_SCENARIO:-all}"

if ! command -v wrk >/dev/null 2>&1; then
  echo "error: wrk not found (install wrk and retry)" >&2
  exit 1
fi

_free_port() {
  "${PYTHON}" -c "import socket; s=socket.socket(); s.bind(('127.0.0.1',0)); print(s.getsockname()[1]); s.close()"
}

_wait_http() {
  local port="$1"
  local deadline=$((SECONDS + 30))
  while (( SECONDS < deadline )); do
    if "${PYTHON}" -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:${port}/', timeout=0.5).read()" 2>/dev/null; then
      return 0
    fi
    sleep 0.05
  done
  return 1
}

_jwt_token() {
  "${PYTHON}" - <<'PY'
import time
try:
    import oxyjwt as jwt
except ImportError as e:
    raise SystemExit("oxyjwt required for JWT scenario: uv sync --extra bench") from e
print(jwt.encode(
    {"sub": "bench", "exp": int(time.time()) + 3600},
    "bench-secret-key-do-not-use-in-prod",
    algorithm="HS256",
))
PY
}

_run_wrk() {
  local url="$1"
  shift
  wrk -t"${THREADS}" -c"${CONN}" -d"${DURATION}" "$@" "${url}" 2>&1 \
    | awk '/Requests\/sec:/{gsub(/^[ \t]+/,"",$2); print $2; exit}'
}

_start_server() {
  local port="$1"
  (
    export PYTHONPATH="${ROOT}"
    cd "${ROOT}/perf-test"
    exec "${PYTHON}" -m granian "app_scenarios:app" \
      --host 127.0.0.1 --port "${port}" --interface rsgi --workers "${WORKERS}"
  ) >/dev/null 2>&1 &
  echo $!
}

_bench_one() {
  local name="$1" path="$2"
  shift 2
  local port pid rps
  port="$(_free_port)"
  pid="$(_start_server "${port}")"
  if ! _wait_http "${port}"; then
    kill "${pid}" 2>/dev/null || true
    wait "${pid}" 2>/dev/null || true
    echo "error: server did not become ready (${name})" >&2
    exit 1
  fi
  rps=$(_run_wrk "http://127.0.0.1:${port}${path}" "$@")
  kill "${pid}" 2>/dev/null || true
  wait "${pid}" 2>/dev/null || true
  printf '%-12s %s\n' "${name}" "${rps}"
}

_lua_json() {
  cat >"$1" <<'LUA'
wrk.method = "POST"
wrk.body   = '{"a":1,"b":"x"}'
wrk.headers["Content-Type"] = "application/json"
LUA
}

_lua_jwt() {
  local token="$2"
  cat >"$1" <<LUA
wrk.headers["Authorization"] = "Bearer ${token}"
LUA
}

_lua_cors() {
  cat >"$1" <<'LUA'
wrk.headers["Origin"] = "https://bench.example"
LUA
}

main() {
  echo "bench_scenarios: OxyRoute RSGI"
  echo "  duration=${DURATION} threads=${THREADS} connections=${CONN} workers=${WORKERS} scenario=${SCENARIO}"
  echo ""

  local tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "${tmp}"' EXIT

  local run_all=0
  [[ "${SCENARIO}" == "all" ]] && run_all=1

  if (( run_all )) || [[ "${SCENARIO}" == "text" ]]; then
    _bench_one "text_get" "/"
  fi

  if (( run_all )) || [[ "${SCENARIO}" == "json" ]]; then
    _lua_json "${tmp}/json.lua"
    _bench_one "json_post" "/json" -s "${tmp}/json.lua"
  fi

  if (( run_all )) || [[ "${SCENARIO}" == "jwt" ]]; then
    local token
    token="$(_jwt_token)"
    _lua_jwt "${tmp}/jwt.lua" "${token}"
    _bench_one "jwt_get" "/jwt" -s "${tmp}/jwt.lua"
  fi

  if (( run_all )) || [[ "${SCENARIO}" == "cors" ]]; then
    _lua_cors "${tmp}/cors.lua"
    _bench_one "cors_get" "/" -s "${tmp}/cors.lua"
  fi

  if (( run_all )) || [[ "${SCENARIO}" == "dep" ]]; then
    _bench_one "dep_get" "/dep"
  fi
}

main "$@"
