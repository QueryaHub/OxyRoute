#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

HOST="127.0.0.1"
PORT="8000"
WRK_THREADS="4"
WRK_CONN="128"
WRK_DUR="15s"
RUNS="3"
SERVER_FLAGS=(--workers 2 --runtime-mode mt --runtime-threads 1)

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
    SERVER_PID=""
  fi
}
trap cleanup EXIT

extract_rps() {
  awk '/Requests\/sec:/ {print $2}' "$1"
}

extract_latency() {
  awk '/Latency/ && $2 ~ /ms|us|s/ {print $2}' "$1" | head -n1
}

run_suite() {
  local name="$1"
  local start_cmd="$2"
  local out_prefix="$3"

  echo "=== ${name} ==="
  eval "${start_cmd}" >/tmp/"${out_prefix}"_server.log 2>&1 &
  SERVER_PID=$!
  sleep 2
  curl -fsS "http://${HOST}:${PORT}/" >/tmp/"${out_prefix}"_smoke.txt

  local rps_values=()
  for i in $(seq 1 "${RUNS}"); do
    local out_file="/tmp/${out_prefix}_wrk_${i}.txt"
    wrk -t"${WRK_THREADS}" -c"${WRK_CONN}" -d"${WRK_DUR}" "http://${HOST}:${PORT}/" | tee "${out_file}" >/dev/null
    local rps
    rps="$(extract_rps "${out_file}")"
    local lat
    lat="$(extract_latency "${out_file}")"
    echo "run${i}: rps=${rps} latency_avg=${lat}"
    rps_values+=("${rps}")
  done

  cleanup

  python3 - "$name" "${rps_values[@]}" <<'PY'
import statistics
import sys
name = sys.argv[1]
vals = [float(x) for x in sys.argv[2:]]
print(f"{name} avg_rps={sum(vals)/len(vals):.2f} median_rps={statistics.median(vals):.2f}")
PY
}

run_suite \
  "OxyRoute RSGI (tuned)" \
  "granian perf-test.app:app --interface rsgi --host ${HOST} --port ${PORT} ${SERVER_FLAGS[*]}" \
  "oxyroute"

run_suite \
  "FastAPI ASGI (tuned)" \
  "uv run --with fastapi granian perf-test.fastapi_app:app --interface asgi --host ${HOST} --port ${PORT} ${SERVER_FLAGS[*]}" \
  "fastapi"

python3 - <<'PY'
import glob
import statistics

def read_rps(prefix):
    vals = []
    for p in sorted(glob.glob(f"/tmp/{prefix}_wrk_*.txt")):
        with open(p, "r", encoding="utf-8") as f:
            for line in f:
                if line.startswith("Requests/sec:"):
                    vals.append(float(line.split()[-1]))
                    break
    return vals

oxy = read_rps("oxyroute")
fa = read_rps("fastapi")
oxy_avg = sum(oxy)/len(oxy)
fa_avg = sum(fa)/len(fa)
delta = (oxy_avg / fa_avg - 1.0) * 100.0
print("=== Summary ===")
print(f"OxyRoute avg={oxy_avg:.2f} median={statistics.median(oxy):.2f}")
print(f"FastAPI avg={fa_avg:.2f} median={statistics.median(fa):.2f}")
print(f"Delta (OxyRoute vs FastAPI): {delta:+.2f}%")
PY
