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

# Stable, short compose project name (avoid collisions by using 12 hex chars)
ID12="$(echo "${INSTANCE_ID}" | tr -d '-' | cut -c1-12)"
PROJECT_NAME="mockrt-${ID12}"

MOCK_VLLM_MODEL_ID="${MOCK_VLLM_MODEL_ID:-demo-model-${ID12}}"
export MOCK_VLLM_MODEL_ID

# Determine which compose file to use based on MOCK_USE_REAL_VLLM
USE_REAL_VLLM="${MOCK_USE_REAL_VLLM:-0}"
if [ "${USE_REAL_VLLM}" = "1" ]; then
  COMPOSE_FILE="docker-compose.mock-runtime-real.yml"
  echo "🚀 mock runtime up (REAL vLLM): INSTANCE_ID=${INSTANCE_ID} project=${PROJECT_NAME} model=${MOCK_VLLM_MODEL_ID}"
else
  COMPOSE_FILE="docker-compose.mock-runtime.yml"
  echo "🚀 mock runtime up (SYNTHETIC): INSTANCE_ID=${INSTANCE_ID} project=${PROJECT_NAME} model=${MOCK_VLLM_MODEL_ID}"
fi

CONTROLPLANE_NETWORK_NAME="${CONTROLPLANE_NETWORK_NAME}" \
INSTANCE_ID="${INSTANCE_ID}" \
MOCK_VLLM_MODEL_ID="${MOCK_VLLM_MODEL_ID}" \
docker compose -f "${COMPOSE_FILE}" -p "${PROJECT_NAME}" up -d --build --remove-orphans

echo "✅ mock runtime started (${PROJECT_NAME})"


