#!/usr/bin/env bash
set -euo pipefail

# Sync required secret files to the remote VM under $SECRETS_DIR.
# This is designed to be replayable for ephemeral VMs (CI/CD).
#
# Required:
#   REMOTE_SSH, SSH_IDENTITY_FILE (optional), ENV_FILE
#
# Sources (in order):
# 1) Local files in $LOCAL_SECRETS_DIR (if set)
# 2) Local files in ./deploy/secrets
# 3) Local .env (gitignored) variables (SCALEWAY_ACCESS_KEY, SCALEWAY_SECRET_KEY, GHCR_TOKEN)
#
# Writes on remote (chmod 600):
#   scaleway_access_key
#   scaleway_secret_key
#   provider_settings_key
#   llm-studio-key.pub
#   ghcr_token
#   default_admin_password
#
# Usage:
#   REMOTE_SSH=root@51.159.184.73 SSH_IDENTITY_FILE=./.ssh/llm-studio-key \
#     ./scripts/remote_sync_secrets.sh env/staging.env

ENV_FILE="${1:-}"
if [[ -z "${ENV_FILE}" ]]; then
  echo "Usage: $0 <env_file>" >&2
  exit 2
fi
if [[ ! -f "${ENV_FILE}" ]]; then
  echo "Env file not found: ${ENV_FILE}" >&2
  exit 2
fi

: "${REMOTE_SSH:?set REMOTE_SSH (ex: root@51.159.184.73)}"

set -a
# shellcheck disable=SC1090
source "${ENV_FILE}"
set +a

SECRETS_DIR="${SECRETS_DIR:-}"
if [[ -z "${SECRETS_DIR}" ]]; then
  echo "SECRETS_DIR is not set in ${ENV_FILE}" >&2
  exit 2
fi

# Save SECRETS_DIR from env file before loading local .env (which may override it)
SAVED_SECRETS_DIR="${SECRETS_DIR}"

# Load local .env if present (local-only, gitignored) as a fallback source for secrets.
# BUT: Don't override SECRETS_DIR from the env file, as it's environment-specific
if [[ -f ".env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source ".env" || true
  # Restore SECRETS_DIR from env file (it's environment-specific, not local-dev-specific)
  SECRETS_DIR="${SAVED_SECRETS_DIR}"
  set +a
fi

SSH_ID_FILE="${SSH_IDENTITY_FILE:-}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KNOWN_HOSTS_FILE="${SSH_KNOWN_HOSTS_FILE:-${REPO_ROOT}/deploy/known_hosts}"
SSH_EXTRA_OPTS="${SSH_EXTRA_OPTS:-} -o UserKnownHostsFile=${KNOWN_HOSTS_FILE} -o StrictHostKeyChecking=accept-new"
SSH_ID_ARGS=()
if [[ -n "${SSH_ID_FILE}" ]]; then
  SSH_ID_ARGS=(-i "${SSH_ID_FILE}")
fi

write_remote_file_from_stdin() {
  local filename="$1"
  # SECRETS_DIR is set in the calling script's environment - expand it here before passing to SSH
  local secrets_dir_expanded="${SECRETS_DIR}"
  ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" "set -euo pipefail
    SECRETS_DIR=\"${secrets_dir_expanded}\"
    REMOTE_FILE=\"\${SECRETS_DIR}/${filename}\"
    sudo mkdir -p \"\${SECRETS_DIR}\"
    sudo sh -c \"umask 077; cat > \\\"\${REMOTE_FILE}\\\"\"
    sudo chmod 644 \"\${REMOTE_FILE}\"
    sudo chown root:root \"\${REMOTE_FILE}\" || true
  "
}

upload_secret_file() {
  local name="$1"
  local local_path="$2"
  if [[ -f "${local_path}" ]]; then
    echo "==> uploading ${name} from ${local_path}"
    # Pass just the filename, SECRETS_DIR will be expanded in write_remote_file_from_stdin
    cat "${local_path}" | write_remote_file_from_stdin "${name}"
    return 0
  fi
  return 1
}

upload_secret_value() {
  local name="$1"
  local value="$2"
  local remote_path="${SECRETS_DIR}/${name}"
  if [[ -n "${value}" ]]; then
    echo "==> uploading ${name} from env var"
    printf '%s' "${value}" | write_remote_file_from_stdin "${remote_path}"
    return 0
  fi
  return 1
}

LOCAL_SECRETS_DIR="${LOCAL_SECRETS_DIR:-}"
if [[ -z "${LOCAL_SECRETS_DIR}" ]]; then
  LOCAL_SECRETS_DIR="./deploy/secrets"
fi

# 1) Scaleway DNS API creds for lego
if ! upload_secret_file "scaleway_access_key" "${LOCAL_SECRETS_DIR}/scaleway_access_key"; then
  upload_secret_value "scaleway_access_key" "${SCALEWAY_ACCESS_KEY:-${SCW_ACCESS_KEY:-}}"
fi

if ! upload_secret_file "scaleway_secret_key" "${LOCAL_SECRETS_DIR}/scaleway_secret_key"; then
  upload_secret_value "scaleway_secret_key" "${SCALEWAY_SECRET_KEY:-${SCW_SECRET_KEY:-}}"
fi

# 1bis) Provider settings encryption key (used to store encrypted provider credentials in DB).
# Generate once and keep it stable per environment:
#   openssl rand -base64 32 > deploy/secrets/provider_settings_key
upload_secret_file "provider_settings_key" "${LOCAL_SECRETS_DIR}/provider_settings_key" || true

# 2) SSH pub key used by orchestrator for worker provisioning
if ! upload_secret_file "llm-studio-key.pub" "${LOCAL_SECRETS_DIR}/llm-studio-key.pub"; then
  upload_secret_file "llm-studio-key.pub" "./.ssh/llm-studio-key.pub" || true
fi

# 3) GHCR pull token (if registry is private)
if ! upload_secret_file "ghcr_token" "${LOCAL_SECRETS_DIR}/ghcr_token"; then
  # Optional: only required when pulling from a private registry.
  if ! upload_secret_value "ghcr_token" "${GHCR_TOKEN:-}"; then
    echo "[warn] ghcr_token not provided; skipping (pull will fail if GHCR packages are private)" >&2
  fi
fi

# 4) Default admin password (used by inventiv-api bootstrap)
if ! upload_secret_file "default_admin_password" "${LOCAL_SECRETS_DIR}/default_admin_password"; then
  # Optional: can be passed via env var (CI or local .env)
  if ! upload_secret_value "default_admin_password" "${DEFAULT_ADMIN_PASSWORD:-}"; then
    echo "[warn] default_admin_password not provided; skipping (default admin bootstrap may be disabled or fail)" >&2
  fi
fi

echo "==> secrets sync done (remote: ${SECRETS_DIR})"

