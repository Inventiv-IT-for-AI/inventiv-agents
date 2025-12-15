#!/usr/bin/env bash
set -euo pipefail

# Provision (or reuse) a Scaleway Instance server (control-plane VM), attach a pre-existing Flexible IP,
# and ensure cloud-init includes your SSH public key.
#
# Uses Scaleway Instance API directly (no scw CLI required).
#
# Required inputs (via sourced env file):
#   SCALEWAY_PROJECT_ID
#   SCW_ZONE (default: fr-par-2)
#   SCW_SERVER_NAME (default: inventiv-agents-<env>-control-plane)
#   SCW_COMMERCIAL_TYPE (default: BASIC2-A4C-8G)
#   REMOTE_HOST (used as the target Flexible IP address, e.g. 51.159.184.73)
#   SSH_IDENTITY_FILE (private key path for SSH test)
# Optional:
#   SSH_PUBLIC_KEY_FILE (default: ./.ssh/llm-studio-key.pub)
#   SCW_IMAGE_ID (if set, no image lookup is done)
# Secrets (local machine):
#   Either export SCW_SECRET_KEY, or have ./deploy/secrets/scaleway_secret_key
#
# Usage:
#   ./scripts/scw_instance_provision.sh env/staging.env staging

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
SCW_COMMERCIAL_TYPE="${SCW_COMMERCIAL_TYPE:-BASIC2-A4C-8G}"
SCW_SERVER_NAME="${SCW_SERVER_NAME:-inventiv-agents-${ENV_NAME}-control-plane}"
FLEX_IP_ADDR="${REMOTE_HOST:?REMOTE_HOST must be set to the flexible IP address}"
PROJECT_ID="${SCALEWAY_PROJECT_ID:?SCALEWAY_PROJECT_ID must be set}"

# Server arch inference:
# - BASIC2-A* are ARM64 on Scaleway (common control-plane types).
# - You can override with SCW_ARCH=x86_64|arm64 in the env file.
SCW_ARCH="${SCW_ARCH:-}"
if [[ -z "${SCW_ARCH}" ]]; then
  if [[ "${SCW_COMMERCIAL_TYPE}" == *"-A"* ]]; then
    SCW_ARCH="arm64"
  else
    SCW_ARCH="x86_64"
  fi
fi

SSH_PUB_FILE="${SSH_PUBLIC_KEY_FILE:-./.ssh/llm-studio-key.pub}"
SSH_PUBLIC_KEY_INLINE="${SSH_PUBLIC_KEY:-}"
SSH_KEY_FILE="${SSH_IDENTITY_FILE:-}"
SSH_PRIVATE_KEY_INLINE="${SSH_PRIVATE_KEY:-}"

TMP_KEY_FILE=""
cleanup_tmp_key() {
  if [[ -n "${TMP_KEY_FILE}" ]]; then
    rm -f "${TMP_KEY_FILE}" >/dev/null 2>&1 || true
  fi
}
trap cleanup_tmp_key EXIT
if [[ -z "${SSH_KEY_FILE}" ]]; then
  echo "SSH_IDENTITY_FILE must be set (private key), e.g. ./.ssh/llm-studio-key" >&2
  exit 2
fi
if [[ ! -f "${SSH_PUB_FILE}" && -z "${SSH_PUBLIC_KEY_INLINE}" ]]; then
  echo "SSH public key missing. Provide SSH_PUBLIC_KEY_FILE or SSH_PUBLIC_KEY." >&2
  exit 2
fi

if [[ ! -f "${SSH_KEY_FILE}" ]]; then
  if [[ -n "${SSH_PRIVATE_KEY_INLINE}" ]]; then
    TMP_KEY_FILE="$(mktemp)"
    umask 077
    printf '%s\n' "${SSH_PRIVATE_KEY_INLINE}" > "${TMP_KEY_FILE}"
    chmod 600 "${TMP_KEY_FILE}" || true
    SSH_KEY_FILE="${TMP_KEY_FILE}"
  else
    echo "SSH private key file not found: ${SSH_KEY_FILE}. Provide SSH_IDENTITY_FILE or SSH_PRIVATE_KEY." >&2
    exit 2
  fi
fi

SCW_SECRET_KEY="${SCW_SECRET_KEY:-}"
# Accept alternative variable names (common in this repo)
if [[ -z "${SCW_SECRET_KEY}" ]]; then
  SCW_SECRET_KEY="${SCALEWAY_SECRET_KEY:-}"
fi

# As a fallback, source repo-root .env if present (local-only, gitignored).
if [[ -z "${SCW_SECRET_KEY}" && -f ".env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source ".env" || true
  set +a
  SCW_SECRET_KEY="${SCW_SECRET_KEY:-${SCALEWAY_SECRET_KEY:-}}"
fi
if [[ -z "${SCW_SECRET_KEY}" ]]; then
  if [[ -f "./deploy/secrets/scaleway_secret_key" ]]; then
    SCW_SECRET_KEY="$(cat ./deploy/secrets/scaleway_secret_key)"
  fi
fi
if [[ -z "${SCW_SECRET_KEY}" ]]; then
  echo "Missing Scaleway secret key. Set SCW_SECRET_KEY or create ./deploy/secrets/scaleway_secret_key" >&2
  exit 2
fi

api() {
  local method="$1"; shift
  local url="$1"; shift

  # We capture HTTP status to fail fast with a useful message (without printing secrets).
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
  if [[ "${status}" != 2* ]]; then
    echo "HTTP ${status} from ${method} ${url}" >&2
    # Print a short body excerpt for debugging.
    echo "${body}" | head -c 800 >&2 || true
    echo "" >&2
    exit 2
  fi

  printf '%s' "${body}"
}

api_get() { api GET "$1"; }
api_post() { api POST "$1" -d "$2"; }
api_put_text() {
  local url="$1"; local body="$2"
  curl -sS -X PUT \
    -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
    -H "Content-Type: text/plain" \
    "${url}" --data-binary "${body}"
}
api_patch() { api PATCH "$1" -d "$2"; }

py() { python3 - "$@"; }

echo "==> Ensuring Flexible IP exists: ${FLEX_IP_ADDR} (zone=${SCW_ZONE})"
IPS_JSON="$(api_get "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/ips")"
if [[ "${SCW_DEBUG:-0}" == "1" ]]; then
  echo "[debug] ips json bytes=$(printf '%s' "${IPS_JSON}" | wc -c | tr -d ' ')"
  echo "[debug] ips json head=$(printf '%s' "${IPS_JSON}" | head -c 120)"
fi
FLEX_IP_ID="$(printf '%s' "${IPS_JSON}" | python3 -c '
import json,sys
addr = sys.argv[1]
data = json.load(sys.stdin)
for ip in data.get("ips", []):
    if ip.get("address") == addr:
        print(ip.get("id",""))
        raise SystemExit(0)
raise SystemExit(2)
' "${FLEX_IP_ADDR}")" || {
  echo "Flexible IP not found in zone ${SCW_ZONE}: ${FLEX_IP_ADDR}" >&2
  exit 2
}
if [[ -z "${FLEX_IP_ID}" ]]; then
  echo "Flexible IP id resolution failed for ${FLEX_IP_ADDR}" >&2
  exit 2
fi

echo "==> Looking for existing server '${SCW_SERVER_NAME}' in project ${PROJECT_ID}"
SERVERS_JSON="$(api_get "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers")"
SERVER_ID="$(printf '%s' "${SERVERS_JSON}" | python3 -c '
import json,sys
name = sys.argv[1]
project = sys.argv[2]
data = json.load(sys.stdin)
for s in data.get("servers", []):
    if s.get("name") == name and s.get("project") == project:
        print(s.get("id",""))
        raise SystemExit(0)
raise SystemExit(0)
' "${SCW_SERVER_NAME}" "${PROJECT_ID}")"

if [[ -z "${SERVER_ID}" ]]; then
  IMAGE_ID="${SCW_IMAGE_ID:-}"
  if [[ -z "${IMAGE_ID}" ]]; then
    echo "==> Resolving an Ubuntu image id (zone=${SCW_ZONE})"
    IMAGES_JSON="$(api_get "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/images")"
    IMAGE_ID="$(printf '%s' "${IMAGES_JSON}" | python3 -c '
import json,sys
desired_arch = sys.argv[1]
data = json.load(sys.stdin)
best = None
for img in data.get("images", []):
    name = (img.get("name") or "").upper()
    img_id = img.get("id")
    if not img_id:
        continue
    if (img.get("arch") or "").lower() != desired_arch.lower():
        continue
    score = 0
    if "UBUNTU" in name: score += 50
    if "22.04" in name or "JAMMY" in name: score += 20
    if "DEBIAN" in name: score += 5
    if best is None or score > best[0]:
        best = (score, img_id)
if best:
    print(best[1])
' "${SCW_ARCH}")"
  fi

  if [[ -z "${IMAGE_ID}" ]]; then
    echo "Could not resolve an image id; set SCW_IMAGE_ID in env file." >&2
    exit 2
  fi

  echo "==> Creating server '${SCW_SERVER_NAME}' (type=${SCW_COMMERCIAL_TYPE}, zone=${SCW_ZONE})"
  CREATE_BODY="$(python3 -c '
import json,sys
name, ctype, project, image = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
print(json.dumps({
  "name": name,
  "commercial_type": ctype,
  "project": project,
  "image": image,
  "dynamic_ip_required": True,
  "tags": ["inventiv-agents","control-plane"],
}))
' "${SCW_SERVER_NAME}" "${SCW_COMMERCIAL_TYPE}" "${PROJECT_ID}" "${IMAGE_ID}")"

  CREATE_RESP="$(api_post "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers" "${CREATE_BODY}")"
  SERVER_ID="$(printf '%s' "${CREATE_RESP}" | python3 -c '
import json,sys
data=json.load(sys.stdin)
print(data["server"]["id"])
')"
  echo "==> Server created: ${SERVER_ID}"
else
  echo "==> Reusing server: ${SERVER_ID}"
fi

echo "==> Setting cloud-init on server (SSH key + flex IP config)"
if [[ -n "${SSH_PUBLIC_KEY_INLINE}" ]]; then
  PUB_KEY="$(printf '%s' "${SSH_PUBLIC_KEY_INLINE}" | tr -d '\n')"
else
  PUB_KEY="$(cat "${SSH_PUB_FILE}" | tr -d '\n')"
fi

# For routed_ipv4 (manual), the OS must add the /32 address to an interface.
# We do it at boot via cloud-init so SSH works on the Flexible IP immediately.
CLOUD_INIT="#cloud-config
ssh_authorized_keys:
  - ${PUB_KEY}

write_files:
  - path: /usr/local/bin/inventiv-flexip.sh
    permissions: '0755'
    content: |
      #!/usr/bin/env bash
      set -euo pipefail
      FLEX_IP='${FLEX_IP_ADDR}'
      IFACE=\$(ip route show default | awk '{print \$5}' | head -n1 || true)
      if [ -z \"\$IFACE\" ]; then
        IFACE=\$(ip -o link show | awk -F': ' '\$2!=\"lo\"{print \$2; exit}')
      fi
      # Immediate config
      ip addr add \"\${FLEX_IP}/32\" dev \"\$IFACE\" 2>/dev/null || true
      sysctl -w net.ipv4.conf.all.rp_filter=0 >/dev/null 2>&1 || true
      sysctl -w net.ipv4.conf.\"\\\$IFACE\".rp_filter=0 >/dev/null 2>&1 || true
      # Persist on Ubuntu via netplan (keep DHCP enabled)
      mkdir -p /etc/netplan
      cat > /etc/netplan/99-inventiv-flexip.yaml <<EOF
      network:
        version: 2
        ethernets:
          \$IFACE:
            dhcp4: true
            addresses:
              - \${FLEX_IP}/32
      EOF
      netplan apply || (netplan generate && netplan apply) || true

runcmd:
  - [ bash, -lc, /usr/local/bin/inventiv-flexip.sh ]
"
api_put_text "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}/user_data/cloud-init" "${CLOUD_INIT}" >/dev/null

echo "==> Ensuring server is running"
for _ in $(seq 1 60); do
  S_JSON="$(api_get "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}")" || true
  STATE="$(printf '%s' "${S_JSON}" | python3 -c 'import json,sys; d=json.load(sys.stdin); print((d.get("server") or {}).get("state",""))' 2>/dev/null || true)"
  if [[ "${STATE}" == "running" ]]; then
    break
  fi
  if [[ "${STATE}" == "stopped" || "${STATE}" == "stopped_in_place" ]]; then
    # Best effort poweron. We retry in the loop anyway.
    curl -sS -X POST \
      -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
      -H "Content-Type: application/json" \
      "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}/action" \
      -d '{"action":"poweron"}' >/dev/null 2>&1 || true
  fi
  sleep 2
done

echo "==> Attaching Flexible IP ${FLEX_IP_ADDR} (id=${FLEX_IP_ID}) to server ${SERVER_ID}"
# Scaleway Instance API expects server id as a string (not an object).
api_patch "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/ips/${FLEX_IP_ID}" "{\"server\":\"${SERVER_ID}\"}" >/dev/null

echo "==> Waiting for SSH on ${FLEX_IP_ADDR}:22 (using ${SSH_KEY_FILE})"
for i in $(seq 1 120); do
  if SSH_IDENTITY_FILE="${SSH_KEY_FILE}" ./scripts/ssh_detect_user.sh "${FLEX_IP_ADDR}" 22 >/dev/null 2>&1; then
    USER_AT_HOST="$(SSH_IDENTITY_FILE="${SSH_KEY_FILE}" ./scripts/ssh_detect_user.sh "${FLEX_IP_ADDR}" 22)"
    echo "✅ SSH OK: ${USER_AT_HOST}"
    echo "SERVER_ID=${SERVER_ID}"
    echo "SSH_TARGET=${USER_AT_HOST}"
    exit 0
  fi
  sleep 5
done

echo "SSH not reachable after timeout on ${FLEX_IP_ADDR}:22" >&2
echo "SERVER_ID=${SERVER_ID}" >&2
exit 2

