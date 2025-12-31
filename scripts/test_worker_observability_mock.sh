#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PORT_OFFSET="${PORT_OFFSET:-0}"
API_HOST_PORT="$((8003 + PORT_OFFSET))"
API_BASE_URL="http://127.0.0.1:${API_HOST_PORT}"

echo "== Test: worker observability chain (mock) =="
echo "API_BASE_URL=$API_BASE_URL"

read_env_kv() {
  local key="$1"
  local file="$2"
  [ -f "$file" ] || return 1
  # Support "KEY=value" (no export), ignore comments/empty lines.
  local line
  line="$(grep -E "^[[:space:]]*${key}=" "$file" | tail -n 1 || true)"
  [ -n "$line" ] || return 1
  echo "${line#*=}"
  return 0
}

# Ensure the compose stack uses the same secrets dir as the script (for deterministic admin login).
# Also, if the admin already exists in a persisted DB volume, force password refresh to avoid 401.
DEV_ENV_FILE="${DEV_ENV_FILE:-${ROOT_DIR}/env/dev.env}"
SECRETS_DIR="${SECRETS_DIR:-}"
if [ -z "${SECRETS_DIR}" ]; then
  SECRETS_DIR="$(read_env_kv SECRETS_DIR "$DEV_ENV_FILE" || true)"
fi
SECRETS_DIR="${SECRETS_DIR:-${ROOT_DIR}/deploy/secrets}"
if [[ "${SECRETS_DIR}" != /* ]]; then
  SECRETS_DIR="${ROOT_DIR}/${SECRETS_DIR}"
fi
export SECRETS_DIR
export BOOTSTRAP_UPDATE_ADMIN_PASSWORD="${BOOTSTRAP_UPDATE_ADMIN_PASSWORD:-1}"
export WORKER_AUTH_TOKEN="${WORKER_AUTH_TOKEN:-dev-worker-token}"
export ORCHESTRATOR_FEATURES="${ORCHESTRATOR_FEATURES:-provider-mock}"

RESET_VOLUMES="${RESET_VOLUMES:-0}"
if [ "$RESET_VOLUMES" = "1" ]; then
  echo "== Cleanup: docker compose down -v =="
  docker compose down -v --remove-orphans >/dev/null 2>&1 || true
  docker compose --profile worker-local down -v --remove-orphans >/dev/null 2>&1 || true
else
  echo "== Cleanup: docker compose down (keep volumes) =="
  docker compose down --remove-orphans >/dev/null 2>&1 || true
  docker compose --profile worker-local down --remove-orphans >/dev/null 2>&1 || true
fi

echo "== 1) Start core stack (db/redis/orchestrator/api) =="
docker compose up -d db redis orchestrator api

echo "== 1b) Ensure DB is ready (pg_isready) =="
db_ok=0
for i in {1..45}; do
  if docker compose exec -T db pg_isready -U postgres -d llminfra >/dev/null 2>&1; then
    db_ok=1
    echo "DB is ready"
    break
  fi
  sleep 1
done
if [ "$db_ok" != "1" ]; then
  echo "❌ DB is not ready in time. Diagnostics:" >&2
  docker compose ps -a >&2 || true
  echo "---- db logs (tail) ----" >&2
  docker compose logs --tail=200 db >&2 || true
  if docker compose logs --tail=200 db 2>/dev/null | grep -qi "no space left on device"; then
    cat >&2 <<'TXT'

🧯 It looks like Docker/TimescaleDB ran out of disk space while initializing Postgres (pg_wal).
Fix options (pick one):

1) Free Docker disk space (safe-ish but can delete unused images/containers):
   - docker system df
   - docker system prune -af
   - docker volume prune

2) Docker Desktop: increase the VM disk image size (Settings → Resources → Disk image size),
   then retry.

3) If you can afford losing local DB state for this repo:
   - docker compose down -v

Note: pruning volumes/images may impact other projects on your machine.
TXT
  fi
  exit 1
fi

echo "== 2) Wait for API / to be ready =="
api_ok=0
for i in {1..60}; do
  if docker compose exec -T api curl -fsS "http://localhost:8003/" >/dev/null 2>&1; then
    echo "API is up (inside container)"
    api_ok=1
    break
  fi
  sleep 1
done
if [ "$api_ok" != "1" ]; then
  echo "❌ API did not become ready in time. Diagnostics:" >&2
  docker compose ps -a >&2 || true
  echo "---- db logs (tail) ----" >&2
  docker compose logs --tail=120 db >&2 || true
  echo "---- api logs (tail) ----" >&2
  docker compose logs --tail=200 api >&2 || true
  echo "---- orchestrator logs (tail) ----" >&2
  docker compose logs --tail=200 orchestrator >&2 || true
  exit 1
fi

echo "== 3) Expose API on host loopback (make api-expose) =="
make api-unexpose PORT_OFFSET="$PORT_OFFSET" >/dev/null 2>&1 || true
make api-expose PORT_OFFSET="$PORT_OFFSET" >/dev/null

echo "== 3b) Wait for API via loopback (${API_BASE_URL}/) =="
loop_ok=0
for i in {1..30}; do
  if curl -fsS "${API_BASE_URL}/" >/dev/null 2>&1; then
    echo "API is up (loopback)"
    loop_ok=1
    break
  fi
  sleep 1
done
if [ "$loop_ok" != "1" ]; then
  echo "❌ API loopback proxy did not become ready. Diagnostics:" >&2
  docker ps --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}' >&2 || true
  NAME="inventiv-api-loopback-${API_HOST_PORT}"
  echo "---- loopback logs (tail) ${NAME} ----" >&2
  docker logs --tail 120 "${NAME}" >&2 || true
  echo "---- api logs (tail) ----" >&2
  docker compose logs --tail=200 api >&2 || true
  exit 1
fi

echo "== 3c) Wait for orchestrator reachability via API proxy =="
orch_ok=0
for i in {1..45}; do
  # If orchestrator is still compiling/booting, API will return 502 orchestrator_unreachable.
  code="$(curl -sS -o /tmp/inventiv_orch_probe.json -w '%{http_code}' \
    -X POST "${API_BASE_URL}/internal/worker/register" \
    -H "Content-Type: application/json" \
    -d '{"instance_id":"00000000-0000-0000-0000-000000000000"}' || true)"
  if [ "$code" != "502" ] && [ "$code" != "000" ]; then
    orch_ok=1
    echo "Orchestrator reachable (proxy status=${code})"
    break
  fi
  sleep 1
done
if [ "$orch_ok" != "1" ]; then
  echo "❌ Orchestrator not reachable via API proxy. Diagnostics:" >&2
  echo "-- last proxy response --" >&2
  cat /tmp/inventiv_orch_probe.json >&2 || true
  echo "---- api logs (tail) ----" >&2
  docker compose logs --tail=200 api >&2 || true
  echo "---- orchestrator logs (tail) ----" >&2
  docker compose logs --tail=200 orchestrator >&2 || true
  exit 1
fi

echo "== 4) Login (admin session cookie) =="
ADMIN_USERNAME="${DEFAULT_ADMIN_USERNAME:-}"
if [ -z "${ADMIN_USERNAME}" ]; then
  ADMIN_USERNAME="$(read_env_kv DEFAULT_ADMIN_USERNAME "$DEV_ENV_FILE" || true)"
fi
ADMIN_USERNAME="${ADMIN_USERNAME:-admin}"

PASSWORD_FILE="${DEFAULT_ADMIN_PASSWORD_FILE:-}"
if [ -z "${PASSWORD_FILE}" ]; then
  PASSWORD_FILE="$(read_env_kv DEFAULT_ADMIN_PASSWORD_FILE "$DEV_ENV_FILE" || true)"
fi
# DEFAULT_ADMIN_PASSWORD_FILE is a container path; for host reading, use SECRETS_DIR.
PASSWORD_FILE_HOST="${SECRETS_DIR}/default_admin_password"

ADMIN_PASSWORD="$(cat "${PASSWORD_FILE_HOST}" 2>/dev/null | tr -d '\n\r' || true)"
if [ -z "$ADMIN_PASSWORD" ]; then
  echo "❌ Missing admin password file: ${PASSWORD_FILE_HOST}" >&2
  echo "Tip: check SECRETS_DIR in ${DEV_ENV_FILE} or export SECRETS_DIR before running this script." >&2
  exit 2
fi

rm -f /tmp/inventiv_cookies.txt
curl -fsS -c /tmp/inventiv_cookies.txt \
  -X POST "${API_BASE_URL}/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"${ADMIN_USERNAME}\",\"password\":\"${ADMIN_PASSWORD}\"}" >/dev/null

echo "== 5) Create mock deployment (get instance_id) =="
echo "Fetching a model_id to deploy..."
MODELS_JSON="$(curl -fsS -b /tmp/inventiv_cookies.txt "${API_BASE_URL}/models" || true)"
export MODELS_JSON
MODEL_ID_UUID="$(python3 - <<'PY' || true
import json, os, sys
raw=os.environ.get("MODELS_JSON","").strip()
if not raw:
    print("")
    raise SystemExit(0)
try:
    j=json.loads(raw)
except Exception:
    print("")
    raise SystemExit(0)

def pick_id(item):
    if not isinstance(item, dict):
        return None
    mid=item.get("id")
    if isinstance(mid,str) and mid:
        # prefer active if present
        if item.get("is_active") is False:
            return None
        return mid
    return None

if isinstance(j, list):
    for it in j:
        v=pick_id(it)
        if v:
            print(v); raise SystemExit(0)
elif isinstance(j, dict):
    for key in ("data","models","items"):
        arr=j.get(key)
        if isinstance(arr,list):
            for it in arr:
                v=pick_id(it)
                if v:
                    print(v); raise SystemExit(0)
print("")
PY
)"

if [ -z "${MODEL_ID_UUID}" ]; then
  echo "No models found; creating a minimal demo model..."
  CREATE_MODEL_JSON="$(curl -fsS -b /tmp/inventiv_cookies.txt \
    -X POST "${API_BASE_URL}/models" \
    -H "Content-Type: application/json" \
    -d '{"name":"Demo model","model_id":"demo-model","required_vram_gb":1,"context_length":2048,"is_active":true}')"
  export CREATE_MODEL_JSON
  MODEL_ID_UUID="$(python3 - <<'PY'
import json, os
j=json.loads(os.environ["CREATE_MODEL_JSON"])
print(j.get("id",""))
PY
)"
fi

if [ -z "${MODEL_ID_UUID}" ]; then
  echo "❌ Could not resolve/create a model_id for deployment" >&2
  exit 1
fi
echo "model_id=$MODEL_ID_UUID"

CREATE_JSON="$(curl -fsS -b /tmp/inventiv_cookies.txt \
  -X POST "${API_BASE_URL}/deployments" \
  -H "Content-Type: application/json" \
  -d "{\"provider_code\":\"mock\",\"zone\":\"local\",\"instance_type\":\"mock-local-instance\",\"model_id\":\"${MODEL_ID_UUID}\"}")"

export CREATE_JSON
INSTANCE_ID="$(python3 - <<'PY'
import json, os
data=json.loads(os.environ["CREATE_JSON"])
print(data["instance_id"])
PY
)"
echo "instance_id=$INSTANCE_ID"

echo "== 6) Start per-instance mock runtime (Option A) =="
# Use a unique mock model id per run to avoid collisions when multiple instances exist in the DB.
MOCK_VLLM_MODEL_ID="demo-model-$(echo "$INSTANCE_ID" | tr -d '-' | cut -c1-12)"
export MOCK_VLLM_MODEL_ID
echo "MOCK_VLLM_MODEL_ID=$MOCK_VLLM_MODEL_ID"

export WORKER_AUTH_TOKEN="${WORKER_AUTH_TOKEN:-dev-worker-token}"
export INSTANCE_ID

# Attach the per-instance runtime to the control-plane docker network so it can reach http://api:8003.
CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME:-"$(basename "$(pwd)")_default"}"
export CONTROLPLANE_NETWORK_NAME
chmod +x ./scripts/mock_runtime_up.sh 2>/dev/null || true
./scripts/mock_runtime_up.sh

echo "== 7) Wait for worker heartbeats to be persisted (poll via API) =="
hb_ok=0
for i in {1..60}; do
  INST_JSON="$(curl -fsS -b /tmp/inventiv_cookies.txt "${API_BASE_URL}/instances/${INSTANCE_ID}" || true)"
  export INST_JSON
  if python3 - <<'PY' >/dev/null 2>&1
import json, os
j=json.loads(os.environ.get("INST_JSON","{}") or "{}")
ok = bool(j.get("worker_last_heartbeat")) and bool(j.get("ip_address")) and bool(j.get("worker_health_port")) and bool(j.get("worker_vllm_port"))
raise SystemExit(0 if ok else 1)
PY
  then
    echo "✅ worker heartbeat persisted"
    hb_ok=1
    break
  fi
  sleep 1
done
if [ "$hb_ok" != "1" ]; then
  echo "❌ worker heartbeat not persisted in time" >&2
  docker compose logs --tail=200 api >&2 || true
  docker compose logs --tail=200 orchestrator >&2 || true
  exit 1
fi

echo "== 8) Wait a bit for time-series samples =="
sleep 6

echo "== 9) Validate instance worker fields via API =="
INST_JSON="$(curl -fsS -b /tmp/inventiv_cookies.txt "${API_BASE_URL}/instances/${INSTANCE_ID}")"
export INST_JSON
set +e
python3 - <<'PY'
import json, os, sys
j=json.loads(os.environ["INST_JSON"])
def fail(msg):
    print("❌ " + msg)
    sys.exit(1)
status=str(j.get("status",""))
if status.lower() not in ("ready","booting","provisioning","terminating","terminated","draining"):
    fail(f"unexpected instance status={status}")
if not j.get("worker_last_heartbeat"):
    fail("worker_last_heartbeat missing")
ws=j.get("worker_status")
if not ws:
    fail("worker_status missing")
qd=j.get("worker_queue_depth")
if qd is None:
    fail("worker_queue_depth missing (expected from mock /metrics)")
print(f"✅ worker_status={ws} worker_queue_depth={qd} worker_last_heartbeat={j.get('worker_last_heartbeat')}")
PY
rc=$?
set -e
if [ $rc -ne 0 ]; then
  echo "---- instance JSON (debug) ----" >&2
  echo "${INST_JSON}" >&2
  echo "---- mock runtime logs (tail) ----" >&2
  # Best-effort: show any mockrt-* containers
  docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}' | grep -E 'mockrt-|NAME' >&2 || true
  echo "---- orchestrator logs (tail) ----" >&2
  docker compose logs --tail=200 orchestrator >&2 || true
  echo "---- api logs (tail) ----" >&2
  docker compose logs --tail=200 api >&2 || true
  echo "---- db instance row ----" >&2
  docker compose exec -T db psql -U postgres -d llminfra -c \
    "select id, status, worker_status, worker_last_heartbeat, worker_model_id, worker_queue_depth from instances where id='${INSTANCE_ID}';" >&2 || true
  exit 1
fi

echo "== 9b) Wait for instance status=ready (health-check convergence) =="
ready_ok=0
for i in {1..60}; do
  INST_JSON="$(curl -fsS -b /tmp/inventiv_cookies.txt "${API_BASE_URL}/instances/${INSTANCE_ID}" || true)"
  export INST_JSON
  if python3 - <<'PY'
import json, os, sys
j=json.loads(os.environ.get("INST_JSON") or "{}")
st=str(j.get("status","")).lower()
sys.exit(0 if st == "ready" else 1)
PY
  then
    echo "Instance is READY"
    ready_ok=1
    break
  fi
  sleep 1
done
if [ "$ready_ok" != "1" ]; then
  echo "❌ Instance did not reach READY in time." >&2
  echo "---- instance JSON (debug) ----" >&2
  echo "${INST_JSON}" >&2
  echo "---- orchestrator logs (tail) ----" >&2
  docker compose logs --tail=200 orchestrator >&2 || true
  echo "---- mock runtime logs (tail) ----" >&2
  docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}' | grep -E 'mockrt-|NAME' >&2 || true
  exit 1
fi

echo "== 10) Validate time series endpoints =="
curl -fsS -o /dev/null -b /tmp/inventiv_cookies.txt \
  "${API_BASE_URL}/gpu/activity?window_s=600&instance_id=${INSTANCE_ID}&granularity=second"

curl -fsS -o /dev/null -b /tmp/inventiv_cookies.txt \
  "${API_BASE_URL}/system/activity?window_s=600&instance_id=${INSTANCE_ID}&granularity=second"
echo "✅ gpu/activity + system/activity reachable"

echo "== 10b) OpenAI upstream routing =="
echo "✅ Using instance.ip_address + worker_vllm_port from worker heartbeats (no DB mutation)"

echo "== 11) Validate OpenAI proxy endpoints (models + chat) =="
echo "Creating a short-lived API key for OpenAI proxy auth..."
APIKEY_RESP_FILE="/tmp/inventiv_apikey_resp.json"
APIKEY_CODE="$(curl -sS -b /tmp/inventiv_cookies.txt \
  -o "${APIKEY_RESP_FILE}" \
  -w '%{http_code}' \
  -X POST "${API_BASE_URL}/api_keys" \
  -H "Content-Type: application/json" \
  -d '{"name":"e2e-worker-observability"}' || true)"
APIKEY_JSON="$(cat "${APIKEY_RESP_FILE}" 2>/dev/null || true)"
if [ "${APIKEY_CODE}" != "200" ]; then
  echo "❌ Failed to create API key (status=${APIKEY_CODE})" >&2
  echo "${APIKEY_JSON}" >&2
  exit 1
fi
export APIKEY_JSON
OPENAI_API_KEY="$(python3 - <<'PY'
import json, os
j=json.loads(os.environ["APIKEY_JSON"])
print(j.get("api_key",""))
PY
)"
if [ -z "${OPENAI_API_KEY}" ]; then
  echo "❌ Failed to create API key (missing api_key in response)" >&2
  echo "${APIKEY_JSON}" >&2
  exit 1
fi

MODELS_RESP_FILE="/tmp/inventiv_openai_models.json"
MODELS_CODE="$(curl -sS \
  -o "${MODELS_RESP_FILE}" \
  -w '%{http_code}' \
  -H "Authorization: Bearer ${OPENAI_API_KEY}" \
  "${API_BASE_URL}/v1/models" || true)"
if [ "${MODELS_CODE}" != "200" ]; then
  echo "❌ /v1/models failed (status=${MODELS_CODE})" >&2
  cat "${MODELS_RESP_FILE}" >&2 || true
  exit 1
fi
python3 - <<'PY'
import json,sys,os
with open("/tmp/inventiv_openai_models.json","r",encoding="utf-8") as f:
    j=json.load(f)
ids=[x.get("id") for x in (j.get("data") or []) if isinstance(x,dict)]
want = os.environ.get("MOCK_VLLM_MODEL_ID","").strip()
if not want:
    print("❌ MOCK_VLLM_MODEL_ID env is missing")
    sys.exit(1)
if want not in ids:
    print(f"❌ {want} not present in /v1/models: " + str(ids))
    sys.exit(1)
print(f"✅ /v1/models contains {want}")
PY

CHAT_RESP_FILE="/tmp/inventiv_openai_chat.json"
CHAT_CODE="$(curl -sS \
  -o "${CHAT_RESP_FILE}" \
  -w '%{http_code}' \
  -X POST "${API_BASE_URL}/v1/chat/completions" \
  -H "Authorization: Bearer ${OPENAI_API_KEY}" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"${MOCK_VLLM_MODEL_ID}\",\"messages\":[{\"role\":\"user\",\"content\":\"ping\"}],\"stream\":false}" || true)"
if [ "${CHAT_CODE}" != "200" ]; then
  echo "❌ /v1/chat/completions failed (status=${CHAT_CODE})" >&2
  cat "${CHAT_RESP_FILE}" >&2 || true
  exit 1
fi
python3 - <<'PY'
import json,sys
with open("/tmp/inventiv_openai_chat.json","r",encoding="utf-8") as f:
    j=json.load(f)
chs=j.get("choices") or []
ok=isinstance(chs,list) and len(chs)>0
if not ok:
    print("❌ invalid chat completion response")
    sys.exit(1)
print("✅ /v1/chat/completions end-to-end OK")
PY

echo "== 12) Execute 3 test requests on local worker =="
REQUESTS=(
  "What is 2+2?"
  "Say hello in French"
  "What is the capital of France?"
)
REQ_NUM=1
for REQ_CONTENT in "${REQUESTS[@]}"; do
  echo "Request ${REQ_NUM}/3: ${REQ_CONTENT}"
  CHAT_RESP_FILE="/tmp/inventiv_openai_chat_${REQ_NUM}.json"
  CHAT_CODE="$(curl -sS \
    -o "${CHAT_RESP_FILE}" \
    -w '%{http_code}' \
    -X POST "${API_BASE_URL}/v1/chat/completions" \
    -H "Authorization: Bearer ${OPENAI_API_KEY}" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"${MOCK_VLLM_MODEL_ID}\",\"messages\":[{\"role\":\"user\",\"content\":\"${REQ_CONTENT}\"}],\"stream\":false,\"max_tokens\":50}" || true)"
  if [ "${CHAT_CODE}" != "200" ]; then
    echo "❌ Request ${REQ_NUM} failed (status=${CHAT_CODE})" >&2
    cat "${CHAT_RESP_FILE}" >&2 || true
    exit 1
  fi
  export CHAT_RESP_FILE REQ_NUM REQ_CONTENT
  python3 - <<'PY'
import json,sys,os
resp_file=os.environ.get("CHAT_RESP_FILE","")
req_num=os.environ.get("REQ_NUM","")
req_content=os.environ.get("REQ_CONTENT","")
if not resp_file:
    print("❌ CHAT_RESP_FILE env missing")
    sys.exit(1)
with open(resp_file,"r",encoding="utf-8") as f:
    j=json.load(f)
chs=j.get("choices") or []
if not isinstance(chs,list) or len(chs)==0:
    print(f"❌ Request {req_num}: invalid response")
    sys.exit(1)
content=chs[0].get("message",{}).get("content","")
if not content:
    print(f"❌ Request {req_num}: empty response content")
    sys.exit(1)
print(f"  → Response: {content}")
print(f"✅ Request {req_num} OK")
PY
  REQ_NUM=$((REQ_NUM + 1))
  sleep 1
done
echo "✅ All 3 test requests completed successfully"

echo "== ✅ PASS: worker observability chain (mock) =="


