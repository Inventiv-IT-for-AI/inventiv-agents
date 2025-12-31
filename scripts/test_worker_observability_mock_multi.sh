#!/usr/bin/env bash
set -euo pipefail

# Multi-instance E2E (mock): create -> attach mock runtimes -> validate observability + OpenAI proxy -> terminate.
# Runs both serial and parallel phases.
#
# Env:
#   PORT_OFFSET=0
#   N_SERIAL=2
#   N_PARALLEL=2
#   WINDOW_S=600

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

DEV_ENV_FILE="${DEV_ENV_FILE:-env/dev.env}"

PORT_OFFSET="${PORT_OFFSET:-0}"
N_SERIAL="${N_SERIAL:-2}"
N_PARALLEL="${N_PARALLEL:-2}"
WINDOW_S="${WINDOW_S:-600}"
BOOTSTRAP_UPDATE_ADMIN_PASSWORD="${BOOTSTRAP_UPDATE_ADMIN_PASSWORD:-1}"

API_HOST_PORT="$((8003 + PORT_OFFSET))"
API_BASE_URL="http://127.0.0.1:${API_HOST_PORT}"

CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME:-"$(basename "$(pwd)")_default"}"
export CONTROLPLANE_NETWORK_NAME

WORKER_AUTH_TOKEN="${WORKER_AUTH_TOKEN:-dev-worker-token}"
export WORKER_AUTH_TOKEN

read_env_kv() {
  local key="$1"
  local file="$2"
  if [ ! -f "${file}" ]; then
    return 1
  fi
  # shellcheck disable=SC2002
  cat "${file}" | grep -E "^[[:space:]]*${key}=" | tail -n 1 | sed -E "s/^[[:space:]]*${key}=//" | sed -E "s/^\"(.*)\"$/\\1/" | sed -E "s/^'(.*)'$/\\1/"
}

SECRETS_DIR="${SECRETS_DIR:-}"
if [ -z "${SECRETS_DIR}" ]; then
  SECRETS_DIR="$(read_env_kv SECRETS_DIR "${DEV_ENV_FILE}" || true)"
fi
SECRETS_DIR="${SECRETS_DIR:-./deploy/secrets}"

# Login expects an email (see scripts/test_worker_observability_mock.sh).
ADMIN_EMAIL="${DEFAULT_ADMIN_EMAIL:-}"
if [ -z "${ADMIN_EMAIL}" ]; then
  ADMIN_EMAIL="$(read_env_kv DEFAULT_ADMIN_EMAIL "${DEV_ENV_FILE}" || true)"
fi
if [ -z "${ADMIN_EMAIL}" ]; then
  # Backward-compat: some env files use DEFAULT_ADMIN_USERNAME to store an email.
  ADMIN_EMAIL="${DEFAULT_ADMIN_USERNAME:-}"
fi
if [ -z "${ADMIN_EMAIL}" ]; then
  ADMIN_EMAIL="$(read_env_kv DEFAULT_ADMIN_USERNAME "${DEV_ENV_FILE}" || true)"
fi
ADMIN_EMAIL="${ADMIN_EMAIL:-admin@inventiv.local}"
ADMIN_PASS_FILE="${SECRETS_DIR}/default_admin_password"

COOKIE_JAR="/tmp/inventiv_cookies_multi.txt"
rm -f "${COOKIE_JAR}" || true

created_ids=()

cleanup() {
  set +e
  echo ""
  echo "🧹 cleanup: stopping runtimes + terminating instances (best effort)"
  # Stop runtimes for any created instance ids (Option A uses per-instance compose projects).
  if [ "${#created_ids[@]}" -gt 0 ]; then
    for id in "${created_ids[@]}"; do
      INSTANCE_ID="${id}" CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME}" ./scripts/mock_runtime_down.sh >/dev/null 2>&1 || true
    done
    for id in "${created_ids[@]}"; do
      curl -fsS -X DELETE -b "${COOKIE_JAR}" "${API_BASE_URL}/instances/${id}" >/dev/null 2>&1 || true
    done
  fi
}
trap cleanup EXIT

require() { command -v "$1" >/dev/null 2>&1 || { echo "❌ missing dependency: $1" >&2; exit 2; }; }
require curl
require python3
require docker

echo "== 0) Bring up control-plane (dev) =="
BOOTSTRAP_UPDATE_ADMIN_PASSWORD="${BOOTSTRAP_UPDATE_ADMIN_PASSWORD}" PORT_OFFSET="${PORT_OFFSET}" make up >/dev/null
PORT_OFFSET="${PORT_OFFSET}" make api-unexpose >/dev/null 2>&1 || true
PORT_OFFSET="${PORT_OFFSET}" make api-expose >/dev/null

echo "== 1) Wait for API ready =="
for i in {1..60}; do
  if curl -fsS "${API_BASE_URL}/" >/dev/null 2>&1; then
    echo "✅ API reachable at ${API_BASE_URL}"
    break
  fi
  sleep 1
done

echo "== 2) Login (cookie session) =="
ADMIN_PASS="$(cat "${ADMIN_PASS_FILE}" | tr -d '\n' || true)"
if [ -z "${ADMIN_PASS}" ]; then
  echo "❌ admin password missing (file=${ADMIN_PASS_FILE})" >&2
  exit 1
fi
curl -fsS -c "${COOKIE_JAR}" -X POST "${API_BASE_URL}/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"${ADMIN_EMAIL}\",\"password\":\"${ADMIN_PASS}\"}" >/dev/null
echo "✅ logged in as ${ADMIN_EMAIL}"

echo "== 3) Resolve a model_id for deployments =="
MODELS_JSON="$(curl -fsS -b "${COOKIE_JAR}" "${API_BASE_URL}/models" || true)"
export MODELS_JSON
MODEL_ID_UUID="$(python3 -c 'import json,os; raw=os.environ.get("MODELS_JSON","").strip(); j=json.loads(raw) if raw else []; rows=j if isinstance(j,list) else (j.get("models") or []); print((rows[0].get("id","") if rows else ""))' || true)"
if [ -z "${MODEL_ID_UUID}" ]; then
  echo "❌ Could not resolve a model_id from /models (seed missing?)" >&2
  exit 1
fi
echo "model_id=${MODEL_ID_UUID}"

create_instance() {
  local instance_type="$1"
  local create_json
  create_json="$(curl -fsS -b "${COOKIE_JAR}" -X POST "${API_BASE_URL}/deployments" \
    -H "Content-Type: application/json" \
    -d "{\"provider_code\":\"mock\",\"zone\":\"mock-eu-1\",\"instance_type\":\"${instance_type}\",\"model_id\":\"${MODEL_ID_UUID}\"}")"
  echo "${create_json}" | python3 -c 'import json,sys; j=json.load(sys.stdin); print(j["instance_id"])'
}

wait_for_worker_fields() {
  local id="$1"
  for i in {1..90}; do
    local j
    j="$(curl -fsS -b "${COOKIE_JAR}" "${API_BASE_URL}/instances/${id}")"
    if echo "${j}" | python3 -c 'import json,sys; j=json.load(sys.stdin); ok=bool(j.get("worker_last_heartbeat")) and bool(j.get("ip_address")) and bool(j.get("worker_health_port")) and bool(j.get("worker_vllm_port")); raise SystemExit(0 if ok else 1)' >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_ready() {
  local id="$1"
  for i in {1..120}; do
    local st
    st="$(curl -fsS -b "${COOKIE_JAR}" "${API_BASE_URL}/instances/${id}" | python3 -c 'import json,sys; j=json.load(sys.stdin); print((j.get("status") or "").lower())')"
    if [ "${st}" = "ready" ]; then
      return 0
    fi
    if [ "${st}" = "startup_failed" ]; then
      echo "⚠️ instance ${id} is startup_failed (will continue waiting briefly)" >&2
    fi
    sleep 1
  done
  return 1
}

wait_for_worker_status_ready() {
  local id="$1"
  for i in {1..90}; do
    local ws
    ws="$(curl -fsS -b "${COOKIE_JAR}" "${API_BASE_URL}/instances/${id}" | python3 -c 'import json,sys; j=json.load(sys.stdin); print((j.get("worker_status") or "").lower())')"
    if [ "${ws}" = "ready" ] || [ -z "${ws}" ]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_samples() {
  local id="$1"
  for i in {1..60}; do
    local sys_ok gpu_ok
    sys_ok=0
    gpu_ok=0
    curl -fsS -b "${COOKIE_JAR}" "${API_BASE_URL}/system/activity?window_s=${WINDOW_S}&instance_id=${id}&granularity=second" | \
      python3 -c 'import json,sys; j=json.load(sys.stdin); inst=(j.get("instances") or []); iid=sys.argv[1]; ok=any((s.get("instance_id")==iid and (s.get("samples") or [])) for s in inst); raise SystemExit(0 if ok else 1)' "${id}" \
      >/dev/null 2>&1 && sys_ok=1 || true

    curl -fsS -b "${COOKIE_JAR}" "${API_BASE_URL}/gpu/activity?window_s=${WINDOW_S}&instance_id=${id}&granularity=second" | \
      python3 -c 'import json,sys; j=json.load(sys.stdin); iid=sys.argv[1]; inst=(j.get("instances") or []); \
ok=any((str(s.get("instance_id"))==iid and any((g.get("samples") or []) for g in (s.get("gpus") or []))) for s in inst); \
raise SystemExit(0 if ok else 1)' "${id}" \
      >/dev/null 2>&1 && gpu_ok=1 || true

    if [ $sys_ok -eq 1 ] && [ $gpu_ok -eq 1 ]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

validate_openai_proxy() {
  local model="$1"
  # /v1/models
  curl -fsS -b "${COOKIE_JAR}" "${API_BASE_URL}/v1/models" | \
    python3 -c 'import json,sys; j=json.load(sys.stdin); ids=[m.get("id") for m in (j.get("data") or [])]; want=sys.argv[1]; \
      (want in ids) or (_ for _ in ()).throw(SystemExit(f"model_not_in_v1_models: {want} (count={len(ids)})"))' "${model}" \
      >/dev/null
  # /v1/chat/completions (echo)
  curl -fsS -b "${COOKIE_JAR}" -X POST "${API_BASE_URL}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"${model}\",\"messages\":[{\"role\":\"user\",\"content\":\"ping\"}]}" | \
    python3 -c 'import json,sys; j=json.load(sys.stdin); choices=j.get("choices") or []; \
      (choices and (((choices[0] or {}).get("message") or {}).get("content") or "")) or (_ for _ in ()).throw(SystemExit("missing choices")); \
      msg=(((choices[0] or {}).get("message") or {}).get("content") or ""); \
      ("mock-vllm ok" in msg) or (_ for _ in ()).throw(SystemExit(f"unexpected content: {msg[:200]}"))' \
    >/dev/null
}

terminate_and_wait() {
  local id="$1"
  curl -fsS -b "${COOKIE_JAR}" -X DELETE "${API_BASE_URL}/instances/${id}" >/dev/null || true
  for i in {1..120}; do
    local st
    st="$(curl -fsS -b "${COOKIE_JAR}" "${API_BASE_URL}/instances/${id}" | python3 -c 'import json,sys; j=json.load(sys.stdin); print((j.get("status") or "").lower())')"
    if [ "${st}" = "terminated" ]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

attach_runtime() {
  local id="$1"
  local id12
  id12="$(echo "${id}" | tr -d '-' | cut -c1-12)"
  local mid="demo-model-${id12}"
  INSTANCE_ID="${id}" MOCK_VLLM_MODEL_ID="${mid}" CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME}" \
    ./scripts/mock_runtime_up.sh >/dev/null
  echo "${mid}"
}

echo "== 4) SERIAL phase (N=${N_SERIAL}) =="
for n in $(seq 1 "${N_SERIAL}"); do
  echo "-- serial create #${n}"
  id="$(create_instance "MOCK-GPU-S")"
  echo "instance_id=${id}"
  created_ids+=("${id}")

  model="$(attach_runtime "${id}")"
  echo "runtime model=${model}"

  echo "wait: worker fields"
  wait_for_worker_fields "${id}" || { echo "❌ worker fields not persisted for ${id}" >&2; exit 1; }

  echo "wait: instance ready"
  wait_for_ready "${id}" || { echo "❌ instance not ready for ${id}" >&2; exit 1; }

  echo "wait: worker_status=ready (for OpenAI routing)"
  wait_for_worker_status_ready "${id}" || { echo "❌ worker_status not ready for ${id}" >&2; exit 1; }

  echo "wait: samples"
  wait_for_samples "${id}" || { echo "❌ missing samples for ${id}" >&2; exit 1; }

  echo "validate: OpenAI proxy"
  validate_openai_proxy "${model}" || { echo "❌ OpenAI proxy validation failed for ${id}" >&2; exit 1; }
done

echo "== 5) PARALLEL phase (N=${N_PARALLEL}) =="
parallel_ids=()
for n in $(seq 1 "${N_PARALLEL}"); do
  echo "-- parallel create #${n}"
  id="$(create_instance "MOCK-GPU-S")"
  echo "instance_id=${id}"
  created_ids+=("${id}")
  parallel_ids+=("${id}")
done

echo "-- parallel attach runtimes"
pids=()
for id in "${parallel_ids[@]}"; do
  (
    attach_runtime "${id}" >/tmp/mock_model_"${id}".txt
  ) &
  pids+=("$!")
done
for p in "${pids[@]}"; do wait "${p}"; done

echo "-- parallel validate"
pids=()
for id in "${parallel_ids[@]}"; do
  (
    wait_for_worker_fields "${id}"
    wait_for_ready "${id}"
    wait_for_worker_status_ready "${id}"
    wait_for_samples "${id}"
    model="$(cat /tmp/mock_model_"${id}".txt)"
    validate_openai_proxy "${model}"
  ) &
  pids+=("$!")
done
for p in "${pids[@]}"; do wait "${p}"; done

echo "== 6) Terminate ALL created instances (parallel) =="
pids=()
for id in "${created_ids[@]}"; do
  (
    terminate_and_wait "${id}"
  ) &
  pids+=("$!")
done
for p in "${pids[@]}"; do wait "${p}"; done

echo "== 7) Stop ALL runtimes (parallel) =="
pids=()
for id in "${created_ids[@]}"; do
  (
    INSTANCE_ID="${id}" CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME}" ./scripts/mock_runtime_down.sh >/dev/null 2>&1 || true
  ) &
  pids+=("$!")
done
for p in "${pids[@]}"; do wait "${p}"; done

echo "✅ multi-instance mock E2E OK (serial=${N_SERIAL}, parallel=${N_PARALLEL})"


