#!/usr/bin/env bash
set -euo pipefail

# Destroy (delete) the Scaleway control-plane VM for an environment.
# Designed for ephemeral environments: you can safely re-run it.
#
# Behavior:
# - Finds the server by name (SCW_SERVER_NAME) in SCW_ZONE and project.
# - Detaches the configured Flexible IP (REMOTE_HOST) if attached (keeps the IP reserved).
# - Deletes the server.
#
# Auth:
# - reads SCW_SECRET_KEY / SCALEWAY_SECRET_KEY from env file or repo-root .env
#
# Usage:
#   ./scripts/scw_instance_destroy.sh env/staging.env staging

ENV_FILE="${1:-}"
ENV_NAME="${2:-}"

if [[ -z "${ENV_FILE}" || -z "${ENV_NAME}" ]]; then
  echo "Usage: $0 <env_file> <staging|prod>" >&2
  exit 2
fi
if [[ ! -f "${ENV_FILE}" ]]; then
  echo "Env file not found: ${ENV_FILE}" >&2
  exit 2
fi

set -a
# shellcheck disable=SC1090
source "${ENV_FILE}"
set +a

SCW_ZONE="${SCW_ZONE:-fr-par-2}"
SCW_SERVER_NAME="${SCW_SERVER_NAME:-inventiv-agents-${ENV_NAME}-control-plane}"
FLEX_IP_ADDR="${REMOTE_HOST:-}"
PROJECT_ID="${SCALEWAY_PROJECT_ID:?SCALEWAY_PROJECT_ID must be set}"

SCW_SECRET_KEY="${SCW_SECRET_KEY:-${SCALEWAY_SECRET_KEY:-}}"
if [[ -z "${SCW_SECRET_KEY}" && -f ".env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source ".env" || true
  set +a
  SCW_SECRET_KEY="${SCW_SECRET_KEY:-${SCALEWAY_SECRET_KEY:-}}"
fi
if [[ -z "${SCW_SECRET_KEY}" ]]; then
  echo "Missing Scaleway secret key in env file or .env (SCW_SECRET_KEY or SCALEWAY_SECRET_KEY)" >&2
  exit 2
fi

api() {
  local method="$1"; shift
  local url="$1"; shift
  local resp status body
  resp="$(curl -sS -X "${method}" \
    -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
    -H "Content-Type: application/json" \
    -w "\n__HTTP_STATUS__:%{http_code}\n" \
    "${url}" "$@")"
  status="$(printf '%s' "${resp}" | tail -n 1 | sed -n 's/^__HTTP_STATUS__:\([0-9]\{3\}\)$/\1/p')"
  body="$(printf '%s' "${resp}" | sed '$d')"
  if [[ -z "${status}" ]]; then
    echo "HTTP request failed (no status) for ${method} ${url}" >&2
    exit 2
  fi
  # allow 2xx and 404 for idempotency
  if [[ "${status}" == "404" ]]; then
    printf '%s' "${body}"
    return 0
  fi
  if [[ "${status}" != 2* ]]; then
    echo "HTTP ${status} from ${method} ${url}" >&2
    echo "${body}" | head -c 800 >&2 || true
    echo "" >&2
    exit 2
  fi
  printf '%s' "${body}"
}

echo "==> Locating server '${SCW_SERVER_NAME}' (zone=${SCW_ZONE})"
SERVERS_JSON="$(api GET "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers")"
SERVER_ID="$(printf '%s' "${SERVERS_JSON}" | python3 -c '
import json,sys
name, project = sys.argv[1], sys.argv[2]
data = json.load(sys.stdin)
for s in data.get("servers", []):
    if s.get("name") == name and s.get("project") == project:
        print(s.get("id",""))
        raise SystemExit(0)
raise SystemExit(0)
' "${SCW_SERVER_NAME}" "${PROJECT_ID}")"

if [[ -z "${SERVER_ID}" ]]; then
  echo "[ok] no server found (already deleted)"
  exit 0
fi

echo "==> Found server id: ${SERVER_ID}"

if [[ -n "${FLEX_IP_ADDR}" ]]; then
  echo "==> Detaching flexible IP ${FLEX_IP_ADDR} (keep reserved)"
  IPS_JSON="$(api GET "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/ips")"
  FLEX_IP_ID="$(printf '%s' "${IPS_JSON}" | python3 -c '
import json,sys
addr = sys.argv[1]
data = json.load(sys.stdin)
for ip in data.get("ips", []):
    if ip.get("address") == addr:
        print(ip.get("id",""))
        raise SystemExit(0)
raise SystemExit(2)
' "${FLEX_IP_ADDR}")" || true
  if [[ -n "${FLEX_IP_ID}" ]]; then
    api PATCH "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/ips/${FLEX_IP_ID}" -d '{"server":null}' >/dev/null
  fi
fi

echo "==> Deleting server ${SERVER_ID}"
# Stop first (Scaleway requires instance powered off before delete).
curl -sS -X POST \
  -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
  -H "Content-Type: application/json" \
  "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}/action" \
  -d '{"action":"poweroff"}' >/dev/null 2>&1 || true

# Wait until stopped (best effort, timeboxed).
for _ in $(seq 1 30); do
  S_JSON="$(api GET "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}")" || true
  STATE="$(printf '%s' "${S_JSON}" | python3 -c 'import json,sys; d=json.load(sys.stdin); print((d.get("server") or {}).get("state",""))' 2>/dev/null || true)"
  if [[ "${STATE}" == "stopped" || "${STATE}" == "stopped_in_place" ]]; then
    break
  fi
  sleep 2
done

api DELETE "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}" >/dev/null || true

# Wait until deleted (avoid immediate recreate reusing same server id).
for _ in $(seq 1 60); do
  code="$(curl -sS -o /dev/null -w "%{http_code}" \
    -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
    "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}" || true)"
  if [[ "${code}" == "404" ]]; then
    break
  fi
  sleep 2
done

echo "✅ Destroy done (server deleted, IP kept)"

