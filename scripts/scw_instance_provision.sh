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
# Non-fatal version for retry loops
api_get_safe() {
  local url="$1"
  local resp status body
  resp="$(curl -sS -X GET \
    -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
    -H "Content-Type: application/json" \
    -w "\n__HTTP_STATUS__:%{http_code}\n" \
    "${url}")" || return 1
  status="$(printf '%s' "${resp}" | tail -n 1 | sed -n 's/^__HTTP_STATUS__:\([0-9]\{3\}\)$/\1/p')"
  body="$(printf '%s' "${resp}" | sed '$d')"
  
  if [[ -z "${status}" ]]; then
    return 1
  fi
  if [[ "${status}" != 2* ]]; then
    return 1
  fi
  
  printf '%s' "${body}"
}
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

echo "==> Ensuring Flexible IP exists: ${FLEX_IP_ADDR} zone=${SCW_ZONE}"
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
    echo "==> Resolving an Ubuntu image id zone=${SCW_ZONE}"
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

  echo "==> Creating server '${SCW_SERVER_NAME}' type=${SCW_COMMERCIAL_TYPE} zone=${SCW_ZONE}"
  # Root volume size (optional, defaults to Scaleway default ~10GB for BASIC2 instances)
  # NOTE: Scaleway API doesn't support root_volume.size in creation request for BASIC2 instances
  # We'll create the server with default size and resize after creation if needed
  ROOT_VOLUME_SIZE_GB="${SCW_ROOT_VOLUME_SIZE_GB:-}"
  if [[ -n "${ROOT_VOLUME_SIZE_GB}" ]]; then
    echo "==> Root volume size specified: ${ROOT_VOLUME_SIZE_GB}GB (will resize after creation)"
  fi
  
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
  
  # If root volume size was specified but API doesn't support it, we'll need to resize after creation
  # For BASIC2 instances, Scaleway creates a local volume (l_ssd) which can be resized via API
  if [[ -n "${ROOT_VOLUME_SIZE_GB}" ]]; then
    echo "==> Checking if root volume resize is needed (target: ${ROOT_VOLUME_SIZE_GB}GB)"
    # Wait a bit for server to be fully created and volumes to be available
    echo "==> Waiting for server volumes to be available..."
    S_JSON=""
    for wait_attempt in $(seq 1 15); do
      sleep 2
      S_JSON="$(api_get_safe "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}")" || S_JSON=""
      # Check if volumes are available in the response
      if [[ -n "${S_JSON}" ]]; then
        if printf '%s' "${S_JSON}" | python3 -c "import json,sys; d=json.load(sys.stdin); v=d.get('server',{}).get('volumes',{}); print('found' if v else 'not_found')" 2>/dev/null | grep -q 'found'; then
          echo "==> Volumes found in API response"
          # Store S_JSON in a temp file to preserve it across potential variable scoping issues
          TMP_JSON_FILE=$(mktemp)
          printf '%s' "${S_JSON}" > "${TMP_JSON_FILE}"
          break
        fi
      fi
      if [[ "${wait_attempt}" -eq 15 ]]; then
        echo "⚠️  Timeout waiting for volumes in API response" >&2
      fi
    done
    
    # Get server details to find root volume (use safe version or temp file)
    if [[ -n "${TMP_JSON_FILE:-}" && -f "${TMP_JSON_FILE}" ]]; then
      S_JSON="$(cat "${TMP_JSON_FILE}")"
      rm -f "${TMP_JSON_FILE}"
    elif [[ -z "${S_JSON}" ]]; then
      S_JSON="$(api_get_safe "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}")" || S_JSON=""
    fi
    
    # Debug: check if response is empty
    if [[ -z "${S_JSON}" ]]; then
      echo "⚠️  Empty response from API. Retrying after longer wait..." >&2
      sleep 5
      S_JSON="$(api_get "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}")" || true
    fi
    
    # Debug: check response validity
    if [[ -z "${S_JSON}" ]]; then
      echo "⚠️  Still empty response from API. Cannot get volume info." >&2
      ROOT_VOLUME_INFO=""
    else
      # Use temp file to avoid pipe issues with large JSON
      TMP_JSON_PARSE=$(mktemp)
      printf '%s' "${S_JSON}" > "${TMP_JSON_PARSE}"
      S_JSON_LEN=$(wc -c < "${TMP_JSON_PARSE}" | tr -d ' ')
      if [[ "${S_JSON_LEN}" -eq 0 ]]; then
        echo "⚠️  S_JSON is empty (length=0). Cannot parse volumes." >&2
        ROOT_VOLUME_INFO=""
        rm -f "${TMP_JSON_PARSE}"
      else
        echo "==> Parsing volume info from API response (length=${S_JSON_LEN} bytes)"
        ROOT_VOLUME_INFO=$(python3 << PYEOF
import json,sys
try:
    with open('${TMP_JSON_PARSE}', 'r') as f:
        raw_input = f.read()
    if not raw_input or raw_input.strip() == '':
        print('DEBUG: Empty JSON input', file=sys.stderr)
        print('')
        sys.exit(0)
    data = json.loads(raw_input)
    server = data.get('server', {})
    volumes = server.get('volumes', {})
    vol_id = ''
    vol_type = ''
    vol_size_gb = 0
    if isinstance(volumes, dict):
        if '0' in volumes:
            vol = volumes['0']
            vol_id = vol.get('id', '')
            vol_type = vol.get('volume_type', '')
            vol_size_bytes = vol.get('size', 0)
            vol_size_gb = vol_size_bytes // 1_000_000_000 if vol_size_bytes > 0 else 0
        elif 'id' in volumes:
            vol_id = volumes.get('id', '')
            vol_type = volumes.get('volume_type', '')
            vol_size_bytes = volumes.get('size', 0)
            vol_size_gb = vol_size_bytes // 1_000_000_000 if vol_size_bytes > 0 else 0
        elif len(volumes) > 0:
            first_key = list(volumes.keys())[0]
            vol = volumes[first_key]
            vol_id = vol.get('id', '')
            vol_type = vol.get('volume_type', '')
            vol_size_bytes = vol.get('size', 0)
            vol_size_gb = vol_size_bytes // 1_000_000_000 if vol_size_bytes > 0 else 0
    elif isinstance(volumes, list) and len(volumes) > 0:
        vol = volumes[0]
        vol_id = vol.get('id', '')
        vol_type = vol.get('volume_type', '')
        vol_size_bytes = vol.get('size', 0)
        vol_size_gb = vol_size_bytes // 1_000_000_000 if vol_size_bytes > 0 else 0
    if vol_id:
        print(f'{vol_id}:{vol_type}:{vol_size_gb}')
    else:
        print(f'DEBUG: Could not find root volume. volumes type={type(volumes)}, value={volumes}', file=sys.stderr)
        print(f'DEBUG: server keys={list(server.keys())}', file=sys.stderr)
        print('')
except Exception as e:
    import traceback
    print(f'DEBUG: Error parsing volumes: {e}', file=sys.stderr)
    traceback.print_exc(file=sys.stderr)
    print('')
PYEOF
)
        rm -f "${TMP_JSON_PARSE}"
      fi
    fi
    
    # If volume type is Block Storage and size is 0, get real size from Block Storage API
    if [[ -n "${ROOT_VOLUME_INFO}" ]]; then
      VOL_ID="${ROOT_VOLUME_INFO%%:*}"
      VOL_TYPE="${ROOT_VOLUME_INFO#*:}"
      VOL_TYPE="${VOL_TYPE%%:*}"
      CURRENT_SIZE_GB="${ROOT_VOLUME_INFO##*:}"
      
      if [[ "${VOL_TYPE}" == "sbs_volume" && "${CURRENT_SIZE_GB}" == "0" ]]; then
        echo "==> Block Storage detected but size is 0GB, fetching real size from Block Storage API..."
        BLOCK_VOL_JSON="$(curl -sS -X GET \
          -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
          "https://api.scaleway.com/block/v1/zones/${SCW_ZONE}/volumes/${VOL_ID}" 2>&1 || true)"
        
        if [[ -n "${BLOCK_VOL_JSON}" ]]; then
          REAL_SIZE_GB="$(printf '%s' "${BLOCK_VOL_JSON}" | python3 -c '
import json,sys
try:
    data = json.load(sys.stdin)
    size_bytes = data.get("size", 0)
    size_gb = size_bytes // 1_000_000_000
    print(size_gb)
except:
    print("0")
' 2>/dev/null || echo "0")"
          
          if [[ "${REAL_SIZE_GB}" != "0" ]]; then
            CURRENT_SIZE_GB="${REAL_SIZE_GB}"
            echo "==> Real Block Storage size from API: ${CURRENT_SIZE_GB}GB"
          fi
        fi
      fi
      
      ROOT_VOLUME_INFO="${VOL_ID}:${VOL_TYPE}:${CURRENT_SIZE_GB}"
    fi
    
    if [[ -n "${ROOT_VOLUME_INFO}" ]]; then
      VOL_ID="${ROOT_VOLUME_INFO%%:*}"
      VOL_TYPE="${ROOT_VOLUME_INFO#*:}"
      VOL_TYPE="${VOL_TYPE%%:*}"
      CURRENT_SIZE_GB="${ROOT_VOLUME_INFO##*:}"
      echo "==> Root volume found: ${VOL_ID} type: ${VOL_TYPE}, current size: ${CURRENT_SIZE_GB}GB"
      
      if [[ "${CURRENT_SIZE_GB}" -lt "${ROOT_VOLUME_SIZE_GB}" ]]; then
        echo "==> Root volume needs resize: ${CURRENT_SIZE_GB}GB → ${ROOT_VOLUME_SIZE_GB}GB"
        echo "==> Stopping server to resize root volume..."
        curl -sS -X POST \
          -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
          -H "Content-Type: application/json" \
          "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}/action" \
          -d '{"action":"poweroff"}' >/dev/null 2>&1 || true
        
        # Wait for server to stop
        echo "==> Waiting for server to stop..."
        SERVER_STOPPED=false
        for wait_iter in $(seq 1 30); do
          S_JSON="$(api_get_safe "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}")" || S_JSON=""
          if [[ -n "${S_JSON}" ]]; then
            STATE="$(printf '%s' "${S_JSON}" | python3 -c 'import json,sys; d=json.load(sys.stdin); print((d.get("server") or {}).get("state",""))' 2>/dev/null || true)"
            if [[ "${STATE}" == "stopped" || "${STATE}" == "stopped_in_place" ]]; then
              echo "==> Server is stopped (state: ${STATE})"
              SERVER_STOPPED=true
              break
            fi
          fi
          if [[ $((wait_iter % 5)) -eq 0 ]]; then
            echo "==> Still waiting for server to stop... (attempt ${wait_iter}/30)"
          fi
          sleep 2
        done
        
        if [[ "${SERVER_STOPPED}" != "true" ]]; then
          echo "⚠️  Server did not stop within timeout. Attempting resize anyway..."
        fi
        
        echo "==> Resizing root volume ${VOL_ID} type ${VOL_TYPE} to ${ROOT_VOLUME_SIZE_GB}GB..."
        RESIZE_SIZE_BYTES=$((ROOT_VOLUME_SIZE_GB * 1000000000))
        
        if [[ "${VOL_TYPE}" == "sbs_volume" ]]; then
          # Block Storage: Scaleway Block Storage can ONLY be resized via CLI (API doesn't support resize)
          echo "==> Block Storage detected: using scw CLI"
          
          if ! command -v scw >/dev/null 2>&1; then
            echo "❌ scw CLI not available. Block Storage resize requires scw CLI."
            echo "   Install it from: https://github.com/scaleway/scaleway-cli"
            echo "   Or resize manually via Scaleway console."
            exit 2
          fi
          
          # Get organization ID and access key (required by scw CLI)
          # Read from env file first (already loaded via source "${ENV_FILE}"), then from secrets directory
          SCW_ORG_ID="${SCALEWAY_ORGANIZATION_ID:-${SCW_DEFAULT_ORGANIZATION_ID:-}}"
          SCW_ACCESS_KEY="${SCALEWAY_ACCESS_KEY:-${SCW_ACCESS_KEY:-}}"
          
          # Fallback: read from secrets directory (for SCALEWAY_ACCESS_KEY only, ORG_ID is in .env)
          if [[ -z "${SCW_ACCESS_KEY}" && -f "./deploy/secrets/scaleway_access_key" ]]; then
            SCW_ACCESS_KEY="$(cat ./deploy/secrets/scaleway_access_key | tr -d '\n\r ')"
          fi
          
          if [[ -z "${SCW_ORG_ID}" ]]; then
            echo "❌ SCALEWAY_ORGANIZATION_ID is required for Block Storage resize via CLI."
            echo "   Set it in ${ENV_FILE} or .env"
            exit 2
          fi
          if [[ -z "${SCW_ACCESS_KEY}" ]]; then
            echo "❌ SCALEWAY_ACCESS_KEY is required for Block Storage resize via CLI."
            echo "   Set it in ${ENV_FILE}, .env, or create ./deploy/secrets/scaleway_access_key"
            exit 2
          fi
          
          echo "==> Resizing Block Storage volume ${VOL_ID} to ${ROOT_VOLUME_SIZE_GB}GB via scw CLI..."
          echo "==> Command: scw block volume update ${VOL_ID} zone=${SCW_ZONE} size=${ROOT_VOLUME_SIZE_GB}GB"
          SCW_CLI_OUTPUT="$(SCW_ACCESS_KEY="${SCW_ACCESS_KEY}" \
            SCW_SECRET_KEY="${SCW_SECRET_KEY}" \
            SCW_DEFAULT_PROJECT_ID="${PROJECT_ID}" \
            SCW_DEFAULT_ORGANIZATION_ID="${SCW_ORG_ID}" \
            scw block volume update "${VOL_ID}" \
            zone="${SCW_ZONE}" \
            size="${ROOT_VOLUME_SIZE_GB}GB" 2>&1)"
          SCW_CLI_EXIT_CODE=$?
          
          if [[ ${SCW_CLI_EXIT_CODE} -ne 0 ]]; then
            echo "❌ CLI resize failed (exit code: ${SCW_CLI_EXIT_CODE}):"
            echo "${SCW_CLI_OUTPUT}"
            exit 2
          fi
          
          echo "✅ Block Storage volume resize command executed successfully"
          echo "==> CLI output:"
          echo "${SCW_CLI_OUTPUT}"
          
          # Wait for resize to propagate and verify
          echo "==> Waiting for resize to propagate and verify..."
          VERIFY_ATTEMPTS=20
          VERIFIED=false
          for attempt in $(seq 1 ${VERIFY_ATTEMPTS}); do
            sleep 5
            VERIFY_JSON="$(curl -sS -X GET \
              -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
              "https://api.scaleway.com/block/v1/zones/${SCW_ZONE}/volumes/${VOL_ID}" 2>&1 || true)"
            
            if [[ -n "${VERIFY_JSON}" ]]; then
              VERIFY_SIZE_GB="$(printf '%s' "${VERIFY_JSON}" | python3 -c '
import json,sys
try:
    data = json.load(sys.stdin)
    size_bytes = data.get("size", 0)
    size_gb = size_bytes // 1_000_000_000
    print(size_gb)
except:
    print("0")
' 2>/dev/null || echo "0")"
              
              echo "==> Verification attempt ${attempt}/${VERIFY_ATTEMPTS}: current size=${VERIFY_SIZE_GB}GB, target=${ROOT_VOLUME_SIZE_GB}GB"
              
              if [[ "${VERIFY_SIZE_GB}" -ge "${ROOT_VOLUME_SIZE_GB}" ]]; then
                echo "✅ Verified resize: volume is now ${VERIFY_SIZE_GB}GB (target: ${ROOT_VOLUME_SIZE_GB}GB)"
                VERIFIED=true
                break
              elif [[ ${attempt} -lt ${VERIFY_ATTEMPTS} ]]; then
                echo "⏳ Resize in progress: ${VERIFY_SIZE_GB}GB (target: ${ROOT_VOLUME_SIZE_GB}GB), waiting 5 more seconds..."
              else
                echo "⚠️  Resize not complete after ${VERIFY_ATTEMPTS} attempts: ${VERIFY_SIZE_GB}GB (target: ${ROOT_VOLUME_SIZE_GB}GB)"
                echo "   Scaleway resize operations can take several minutes. The resize command was executed successfully."
                echo "   The volume will continue resizing in the background. Check the Scaleway console for status."
              fi
            else
              echo "⚠️  Could not fetch volume info for verification (attempt ${attempt}/${VERIFY_ATTEMPTS})"
            fi
          done
          
          if [[ "${VERIFIED}" != "true" ]]; then
            echo "⚠️  Could not verify resize completion after ${VERIFY_ATTEMPTS} attempts ($((VERIFY_ATTEMPTS * 5))s total wait)."
            echo "   The resize command was executed successfully. Scaleway resize operations can take several minutes."
            echo "   Please check the Scaleway console to verify the resize status."
          fi
        else
          # Local volume (l_ssd): use Instance API
          RESIZE_BODY="$(python3 -c '
import json,sys
size_bytes = int(sys.argv[1])
print(json.dumps({"size": size_bytes}))
' "${RESIZE_SIZE_BYTES}")"
          
          api PATCH "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/volumes/${VOL_ID}" -d "${RESIZE_BODY}" >/dev/null || {
            echo "⚠️  Failed to resize local volume via API. You may need to resize manually via Scaleway console or CLI."
          }
        fi
        
        echo "==> Root volume resize requested. Waiting for resize to complete..."
        sleep 5
        
        # Restart server after resize
        echo "==> Starting server after root volume resize..."
        curl -sS -X POST \
          -H "X-Auth-Token: ${SCW_SECRET_KEY}" \
          -H "Content-Type: application/json" \
          "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}/action" \
          -d '{"action":"poweron"}' >/dev/null 2>&1 || true
      else
        echo "==> Root volume size is already ${CURRENT_SIZE_GB}GB >= ${ROOT_VOLUME_SIZE_GB}GB target"
      fi
    else
      echo "⚠️  Could not find root volume ID. Skipping resize."
    fi
  fi
else
  echo "==> Reusing server: ${SERVER_ID}"
fi

# Note: cloud-init configuration removed - SSH key and Flexible IP configuration
# should be done manually after provisioning if needed.

ensure_security_group() {
  # Ensure a security group exists and allows inbound SSH/HTTP/HTTPS.
  # Without this, Scaleway may block 80/443 by default on new servers.
  local sg_name sg_id sgs_json create_body rules_body

  sg_name="${SCW_SECURITY_GROUP_NAME:-inventiv-agents-${ENV_NAME}-sg}"

  echo "==> Ensuring security group '${sg_name}' allows 22/80/443"
  sgs_json="$(api_get "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/security_groups")"
  sg_id="$(printf '%s' "${sgs_json}" | python3 -c '
import json,sys
name = sys.argv[1]
project = sys.argv[2]
data = json.load(sys.stdin)
for sg in data.get("security_groups", []):
    if sg.get("name") == name and sg.get("project") == project:
        print(sg.get("id",""))
        raise SystemExit(0)
raise SystemExit(0)
' "${sg_name}" "${PROJECT_ID}")"

  if [[ -z "${sg_id}" ]]; then
    create_body="$(python3 -c '
import json,sys
name, project = sys.argv[1], sys.argv[2]
print(json.dumps({
  "name": name,
  "project": project,
  "stateful": True,
  "inbound_default_policy": "drop",
  "outbound_default_policy": "accept",
  "tags": ["inventiv-agents", "control-plane"],
}))
' "${sg_name}" "${PROJECT_ID}")"
    sg_id="$(printf '%s' "$(api_post "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/security_groups" "${create_body}")" | python3 -c '
import json,sys
data=json.load(sys.stdin)
print(data["security_group"]["id"])
')"
  fi

  # Replace rules with a known-good allowlist (idempotent).
  rules_body="$(python3 -c '
import json
rules=[]
pos=1
for port in (22,80,443):
    rules.append({
        "action":"accept",
        "protocol":"TCP",
        "direction":"inbound",
        "ip_range":"0.0.0.0/0",
        "dest_port_from":port,
        "dest_port_to":port,
        "position":pos,
        "editable":True,
    })
    pos += 1
print(json.dumps({"rules": rules}))
')"
  api PUT "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/security_groups/${sg_id}/rules" -d "${rules_body}" >/dev/null

  # Attach SG to server (idempotent).
  api_patch "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}" "$(python3 -c '
import json,sys
sg_id=sys.argv[1]
print(json.dumps({"security_group": {"id": sg_id}}))
' "${sg_id}")" >/dev/null
}

ensure_security_group

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

echo "==> Attaching Flexible IP ${FLEX_IP_ADDR} id=${FLEX_IP_ID} to server ${SERVER_ID}"
# Scaleway Instance API expects server id as a string (not an object).
PATCH_BODY=$(python3 -c "import json; print(json.dumps({'server': '${SERVER_ID}'}))")
api_patch "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/ips/${FLEX_IP_ID}" "${PATCH_BODY}" >/dev/null

echo "==> Waiting for SSH on ${FLEX_IP_ADDR}:22 using ${SSH_KEY_FILE}"
S_JSON="$(api_get "https://api.scaleway.com/instance/v1/zones/${SCW_ZONE}/servers/${SERVER_ID}")" || true
DYN_IP="$(printf '%s' "${S_JSON}" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(((d.get("server") or {}).get("public_ip") or {}).get("address",""))' 2>/dev/null || true)"
if [[ -n "${DYN_IP}" ]]; then
  echo "==> Dynamic IP - fallback SSH: ${DYN_IP}"
fi

forced_flexip=0
for i in $(seq 1 60); do
  if SSH_IDENTITY_FILE="${SSH_KEY_FILE}" ./scripts/ssh_detect_user.sh "${FLEX_IP_ADDR}" 22 >/dev/null 2>&1; then
    USER_AT_HOST="$(SSH_IDENTITY_FILE="${SSH_KEY_FILE}" ./scripts/ssh_detect_user.sh "${FLEX_IP_ADDR}" 22)"
    echo "✅ SSH OK: ${USER_AT_HOST}"
    echo "SERVER_ID=${SERVER_ID}"
    echo "SSH_TARGET=${USER_AT_HOST}"
    exit 0
  fi

  # Note: Flexible IP configuration may need manual setup after provisioning.
  # SSH keys are automatically added by Scaleway, so no cloud-init needed for that.

  sleep 5
done

echo "SSH not reachable after timeout on ${FLEX_IP_ADDR}:22" >&2
echo "SERVER_ID=${SERVER_ID}" >&2
exit 2

