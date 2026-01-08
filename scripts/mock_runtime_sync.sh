#!/usr/bin/env bash
set -euo pipefail

# Ensure a mock runtime exists for each active Mock instance (Option A).
# This script is intentionally "external" to the orchestrator: it inspects the DB and starts per-instance runtimes.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME:-}"
if [ -z "${CONTROLPLANE_NETWORK_NAME}" ]; then
  # Default docker-compose project network name.
  CONTROLPLANE_NETWORK_NAME="$(basename "$(pwd)")_default"
fi
export CONTROLPLANE_NETWORK_NAME

echo "🔎 mock runtime sync: using CONTROLPLANE_NETWORK_NAME=${CONTROLPLANE_NETWORK_NAME}"

# Select active mock instances (not archived, not terminated).
IDS="$(docker compose exec -T db psql -U postgres -d llminfra -t -A -c "
  SELECT i.id::text
  FROM instances i
  JOIN providers p ON p.id = i.provider_id
  WHERE p.code = 'mock'
    AND COALESCE(i.is_archived, false) = false
    AND i.status IN ('provisioning','booting','ready','draining','terminating')
  ORDER BY i.created_at DESC;
")"

if [ -z "${IDS}" ]; then
  echo "ℹ️  no active mock instances found"
  exit 0
fi

count=0
while IFS= read -r id; do
  [ -z "${id}" ] && continue
  count=$((count+1))
  echo "→ ensuring runtime for instance_id=${id}"
  INSTANCE_ID="${id}" CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME}" ./scripts/mock_runtime_up.sh
done <<< "${IDS}"

echo "✅ mock runtime sync done (${count} instance(s))"


