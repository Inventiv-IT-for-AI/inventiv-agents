#!/usr/bin/env bash
set -euo pipefail

INSTANCE_ID="${INSTANCE_ID:-}"
if [ -z "${INSTANCE_ID}" ]; then
  echo "❌ INSTANCE_ID is required" >&2
  exit 2
fi

CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME:-}"
if [ -z "${CONTROLPLANE_NETWORK_NAME}" ]; then
  echo "❌ CONTROLPLANE_NETWORK_NAME is required (e.g. inventiv-agents-worker-fixes_default)" >&2
  exit 2
fi

ID12="$(echo "${INSTANCE_ID}" | tr -d '-' | cut -c1-12)"
PROJECT_NAME="mockrt-${ID12}"

echo "🛑 mock runtime down: INSTANCE_ID=${INSTANCE_ID} project=${PROJECT_NAME}"

CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME}" \
docker compose -f docker-compose.mock-runtime.yml -p "${PROJECT_NAME}" down --remove-orphans

echo "✅ mock runtime stopped (${PROJECT_NAME})"


