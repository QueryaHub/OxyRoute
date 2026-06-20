#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

HOST="127.0.0.1"
WRK_THREADS="${OXYROUTE_BENCH_THREADS:-4}"
WRK_CONN="${OXYROUTE_BENCH_CONNECTIONS:-128}"
WRK_DUR="${OXYROUTE_BENCH_DURATION:-10s}"
RUNS="${OXYROUTE_BENCH_RUNS:-3}"
WORKERS="${OXYROUTE_BENCH_WORKERS:-2}"
RUNTIME_MODE="${OXYROUTE_BENCH_RUNTIME_MODE:-mt}"
RUNTIME_THREADS="${OXYROUTE_BENCH_RUNTIME_THREADS:-1}"
SERVER_FLAGS=(--workers "${WORKERS}" --runtime-mode "${RUNTIME_MODE}" --runtime-threads "${RUNTIME_THREADS}")
BENCH_TMP="$(mktemp -d)"

if [[ -n "${PYTHON:-}" ]]; then
  :
elif [[ -x "${ROOT_DIR}/.venv/bin/python" ]]; then
  PYTHON="${ROOT_DIR}/.venv/bin/python"
else
  PYTHON="python3"
fi

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill -- "-${SERVER_PID}" 2>/dev/null || kill "${SERVER_PID}" 2>/dev/null || true
    SERVER_PID=""
  fi
}

final_cleanup() {
  cleanup
  rm -rf "${BENCH_TMP}"
}
trap final_cleanup EXIT

free_port() {
  "${PYTHON}" -c "import socket; s=socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1]); s.close()"
}

wait_http() {
  local port="$1"
  local path="$2"
  local deadline=$((SECONDS + 15))
  while (( SECONDS < deadline )); do
    if "${PYTHON}" -c "import urllib.request; urllib.request.urlopen('http://${HOST}:${port}${path}', timeout=0.5).read()" 2>/dev/null; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}

extract_rps() {
  awk '/Requests\/sec:/ {print $2}' "$1"
}

extract_latency() {
  awk '/Latency/ && $2 ~ /ms|us|s/ {print $2}' "$1" | head -n1
}

run_suite() {
  local name="$1"
  local module_path="$2"
  local interface="$3"
  local path="$4"
  local out_prefix="$5"
  
  local port
  port="$(free_port)"
  local -a cmd=(
    "${PYTHON}" -m granian "${module_path}"
    --interface "${interface}"
    --host "${HOST}"
    --port "${port}"
    "${SERVER_FLAGS[@]}"
  )

  echo "=== ${name} ==="
  setsid "${cmd[@]}" >"${BENCH_TMP}/${out_prefix}_server.log" 2>&1 &
  SERVER_PID=$!
  if ! wait_http "${port}" "${path}"; then
    echo "error: server did not become ready (${name})" >&2
    echo "--- server log ---" >&2
    cat "${BENCH_TMP}/${out_prefix}_server.log" >&2 || true
    exit 1
  fi

  local rps_values=()
  for i in $(seq 1 "${RUNS}"); do
    local out_file="${BENCH_TMP}/${out_prefix}_wrk_${i}.txt"
    wrk -t"${WRK_THREADS}" -c"${WRK_CONN}" -d"${WRK_DUR}" "http://${HOST}:${port}${path}" | tee "${out_file}" >/dev/null
    local rps
    rps="$(extract_rps "${out_file}")"
    local lat
    lat="$(extract_latency "${out_file}")"
    echo "run${i}: rps=${rps} latency_avg=${lat}"
    rps_values+=("${rps}")
  done

  cleanup

  "${PYTHON}" - "$name" "${rps_values[@]}" <<'PY'
import statistics
import sys
name = sys.argv[1]
vals = [float(x) for x in sys.argv[2:]]
print(f"{name} avg_rps={sum(vals)/len(vals):.2f} median_rps={statistics.median(vals):.2f}")
PY
}

run_suite \
  "OxyRoute Rust sqlx /test_db" \
  "perf-test.test_sqlx:app" \
  "rsgi" \
  "/test_db" \
  "oxyroute_sqlx"

run_suite \
  "FastAPI asyncpg /test_db" \
  "perf-test.fastapi_app:app" \
  "asgi" \
  "/test_db" \
  "fastapi_asyncpg"

"${PYTHON}" - "${BENCH_TMP}" <<'PY'
import glob
import statistics
import sys

tmp = sys.argv[1]

def read_rps(prefix):
    vals = []
    for p in sorted(glob.glob(f"{tmp}/{prefix}_wrk_*.txt")):
        with open(p, "r", encoding="utf-8") as f:
            for line in f:
                if "Requests/sec:" in line:
                    vals.append(float(line.split()[-1]))
                    break
    return vals

oxy = read_rps("oxyroute_sqlx")
fa = read_rps("fastapi_asyncpg")
if not oxy or not fa:
    raise SystemExit(f"missing wrk results: oxyroute={oxy!r}, fastapi={fa!r}")
oxy_avg = sum(oxy)/len(oxy)
fa_avg = sum(fa)/len(fa)
ratio = oxy_avg / fa_avg
uplift = (ratio - 1.0) * 100.0
print("=== Summary ===")
print(f"OxyRoute (Rust sqlx) avg={oxy_avg:.2f} median={statistics.median(oxy):.2f}")
print(f"FastAPI (asyncpg) avg={fa_avg:.2f} median={statistics.median(fa):.2f}")
print(f"Ratio (OxyRoute / FastAPI): {ratio:.2f}x")
print(f"Relative uplift vs FastAPI: {uplift:+.2f}%")
PY
