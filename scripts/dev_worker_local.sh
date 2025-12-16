#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "== 0. Local worker-ready test (mock provider) =="

export PROVIDER="${PROVIDER:-mock}"
# Leave empty by default to test per-worker bootstrap token flow.
export WORKER_AUTH_TOKEN="${WORKER_AUTH_TOKEN:-}"

echo "== Cleanup (idempotent): stop previous stack (keep volumes) =="
RESET_VOLUMES="${RESET_VOLUMES:-1}"
if [ "$RESET_VOLUMES" = "1" ]; then
  echo "Resetting volumes for deterministic migrations (RESET_VOLUMES=1)"
  docker compose down -v --remove-orphans >/dev/null 2>&1 || true
  docker compose --profile worker-local down -v --remove-orphans >/dev/null 2>&1 || true
else
  docker compose down --remove-orphans >/dev/null 2>&1 || true
  docker compose --profile worker-local down --remove-orphans >/dev/null 2>&1 || true
fi

pick_free_port() {
  local start="${1}"
  local end="${2}"
  python3 - <<PY
import socket
start=int("${start}"); end=int("${end}")
for port in range(start, end+1):
    s=socket.socket()
    try:
        s.bind(("127.0.0.1", port))
        s.close()
        print(port)
        raise SystemExit(0)
    except OSError:
        try: s.close()
        except: pass
print("")
raise SystemExit(1)
PY
}

echo "== 1) Start core stack (db/redis/orchestrator/api) =="
docker compose up -d db redis orchestrator api

echo "== 2) Wait for API to be ready (auto seed if providers empty) =="
for i in {1..60}; do
  if docker compose exec -T api curl -fsS "http://localhost:8003/" >/dev/null 2>&1; then
    echo "API is up"
    break
  fi
  sleep 1
done

echo "== 3) Create mock deployment (get instance_id) =="
CREATE_JSON="$(docker compose exec -T api curl -fsS -X POST "http://localhost:8003/deployments" \
  -H "Content-Type: application/json" \
  -d '{"provider_code":"mock","zone":"mock-eu-1","instance_type":"MOCK-GPU-S"}')"

export CREATE_JSON
INSTANCE_ID="$(python3 - <<'PY'
import json, os, sys
data=json.loads(os.environ["CREATE_JSON"])
print(data["instance_id"])
PY
)"

echo "instance_id=$INSTANCE_ID"
export INSTANCE_ID

export WORKER_AGENT_HOST_PORT="${WORKER_AGENT_HOST_PORT:-$(pick_free_port 18080 18099)}"
export MOCK_VLLM_HOST_PORT="${MOCK_VLLM_HOST_PORT:-$(pick_free_port 18000 18019)}"
echo "worker-agent host port: $WORKER_AGENT_HOST_PORT"
echo "mock-vllm host port: $MOCK_VLLM_HOST_PORT"

echo "== 4) Start worker-local profile (mock-vllm + worker-agent) =="
docker compose --profile worker-local up -d --build mock-vllm worker-agent

echo "== 5) Check worker endpoints and DB columns =="
echo "Waiting for /readyz on http://localhost:${WORKER_AGENT_HOST_PORT}/readyz ..."
for i in {1..30}; do
  if curl -fsS "http://localhost:${WORKER_AGENT_HOST_PORT}/readyz" >/dev/null 2>&1; then
    echo "Worker is READY"
    break
  fi
  sleep 1
done

echo "-- /readyz"
curl -i "http://localhost:${WORKER_AGENT_HOST_PORT}/readyz" || true

echo "-- /metrics (first 30 lines)"
curl -fsS "http://localhost:${WORKER_AGENT_HOST_PORT}/metrics" | head -n 30 || true

echo "Waiting a few seconds for heartbeats to reach orchestrator..."
sleep 6

echo "-- Worker logs"
docker compose --profile worker-local logs --tail=80 worker-agent || true

echo "-- Orchestrator logs (worker_register/heartbeat)"
docker compose logs --tail=80 orchestrator || true

echo "-- DB check (worker_status/heartbeat)"
docker compose exec -T db psql -U postgres -d llminfra -c \
  "select id, status, worker_status, worker_last_heartbeat, worker_model_id, worker_health_port from instances where id='${INSTANCE_ID}';" || true

echo "-- DB check (worker_auth_tokens)"
docker compose exec -T db psql -U postgres -d llminfra -c \
  "select instance_id, token_prefix, created_at, last_seen_at, revoked_at from worker_auth_tokens where instance_id='${INSTANCE_ID}';" || true

echo "== Done =="
echo "To stop: docker compose down -v"

