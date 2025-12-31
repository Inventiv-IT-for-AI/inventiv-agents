#!/usr/bin/env bash
set -euo pipefail

# External mock runtime manager (Option A)
# - watches DB for mock instances
# - starts per-instance runtime when needed (ip/heartbeat missing)
# - stops per-instance runtime when instance is archived/terminated
#
# This script is intentionally NOT part of the orchestrator.
#
# Env:
#   CONTROLPLANE_NETWORK_NAME (default: <pwd>_default)
#   WATCH_INTERVAL_S (default: 5)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME:-"$(basename "$(pwd)")_default"}"
WATCH_INTERVAL_S="${WATCH_INTERVAL_S:-5}"

if ! [[ "${WATCH_INTERVAL_S}" =~ ^[0-9]+$ ]]; then
  echo "❌ WATCH_INTERVAL_S must be an integer (got: ${WATCH_INTERVAL_S})" >&2
  exit 2
fi

echo "👀 mock-runtime-watch started"
echo "   CONTROLPLANE_NETWORK_NAME=${CONTROLPLANE_NETWORK_NAME}"
echo "   WATCH_INTERVAL_S=${WATCH_INTERVAL_S}"

while true; do
  # If db is not running yet, just wait.
  if ! docker compose ps -q db >/dev/null 2>&1; then
    sleep "${WATCH_INTERVAL_S}"
    continue
  fi

  # Instances that should have a runtime running (need worker heartbeat to advertise ip+ports).
  WANT_UP_IDS="$(docker compose exec -T db psql -U postgres -d llminfra -t -A -c "
    SELECT i.id::text
    FROM instances i
    JOIN providers p ON p.id = i.provider_id
    WHERE p.code = 'mock'
      AND COALESCE(i.is_archived, false) = false
      AND i.status IN ('provisioning','booting','ready','draining','terminating')
      AND (
        i.ip_address IS NULL
        OR i.worker_last_heartbeat IS NULL
        OR i.worker_health_port IS NULL
        OR i.worker_vllm_port IS NULL
      )
    ORDER BY i.created_at DESC;
  " 2>/dev/null || true)"

  if [ -n "${WANT_UP_IDS}" ]; then
    while IFS= read -r id; do
      [ -z "${id}" ] && continue
      INSTANCE_ID="${id}" CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME}" ./scripts/mock_runtime_up.sh >/dev/null 2>&1 || true
    done <<< "${WANT_UP_IDS}"
  fi

  # Instances that should have their runtime stopped (best-effort cleanup).
  WANT_DOWN_IDS="$(docker compose exec -T db psql -U postgres -d llminfra -t -A -c "
    SELECT i.id::text
    FROM instances i
    JOIN providers p ON p.id = i.provider_id
    WHERE p.code = 'mock'
      AND (
        COALESCE(i.is_archived, false) = true
        OR i.status IN ('terminated','archived')
      )
    ORDER BY i.created_at DESC
    LIMIT 200;
  " 2>/dev/null || true)"

  if [ -n "${WANT_DOWN_IDS}" ]; then
    while IFS= read -r id; do
      [ -z "${id}" ] && continue
      INSTANCE_ID="${id}" CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME}" ./scripts/mock_runtime_down.sh >/dev/null 2>&1 || true
    done <<< "${WANT_DOWN_IDS}"
  fi

  sleep "${WATCH_INTERVAL_S}"
done


