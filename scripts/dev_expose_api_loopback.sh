#!/usr/bin/env bash
set -euo pipefail

# Expose the API container port 8003 to the host loopback (127.0.0.1) without modifying docker-compose.yml.
#
# Why:
# - cloudflared runs on the host and needs a host-reachable address (localhost:PORT).
# - we don't want to publish api:8003 directly in compose (avoid collisions / keep UI-only default).
#
# How:
# - run a tiny socat container on the docker-compose project network
# - publish 127.0.0.1:${API_HOST_PORT} -> socat:8003 -> api:8003
#
# Usage:
#   PORT_OFFSET=10000 ./scripts/dev_expose_api_loopback.sh
#   API_HOST_PORT=18003 ./scripts/dev_expose_api_loopback.sh

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PORT_OFFSET="${PORT_OFFSET:-0}"
API_HOST_PORT="${API_HOST_PORT:-}"

if [ -z "${API_HOST_PORT}" ]; then
  # Default: base 8003 + offset
  if ! [[ "${PORT_OFFSET}" =~ ^[0-9]+$ ]]; then
    echo "❌ PORT_OFFSET must be a non-negative integer (got: '${PORT_OFFSET}')" >&2
    exit 2
  fi
  API_HOST_PORT="$((8003 + PORT_OFFSET))"
fi

if ! [[ "${API_HOST_PORT}" =~ ^[0-9]+$ ]]; then
  echo "❌ API_HOST_PORT must be a port number (got: '${API_HOST_PORT}')" >&2
  exit 2
fi

API_CID="$(docker compose ps -q api || true)"
if [ -z "${API_CID}" ]; then
  echo "❌ Could not find running 'api' container. Start the stack first: make up" >&2
  exit 2
fi

# Get the first attached docker network for the api container (compose default network).
NETWORK="$(docker inspect -f '{{range $k, $v := .NetworkSettings.Networks}}{{println $k}}{{end}}' "${API_CID}" | head -n 1 | tr -d '\r' || true)"
if [ -z "${NETWORK}" ]; then
  echo "❌ Could not resolve docker network for api container ${API_CID}" >&2
  exit 2
fi

NAME="inventiv-api-loopback-${API_HOST_PORT}"

echo "🔌 Exposing API on http://127.0.0.1:${API_HOST_PORT} (docker network: ${NETWORK})"
echo "   - container name: ${NAME}"
echo "   - target: api:8003"

# If already running, replace it.
docker rm -f "${NAME}" >/dev/null 2>&1 || true

docker run -d --restart unless-stopped \
  --name "${NAME}" \
  --network "${NETWORK}" \
  -p "127.0.0.1:${API_HOST_PORT}:8003" \
  alpine:3.20 \
  sh -lc "apk add --no-cache socat >/dev/null && socat -d -d TCP-LISTEN:8003,fork,reuseaddr TCP:api:8003" >/dev/null

echo "✅ API loopback proxy up. Test:"
echo "   curl -fsS http://127.0.0.1:${API_HOST_PORT}/ | head"


