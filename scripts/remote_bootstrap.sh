#!/usr/bin/env bash
set -euo pipefail

# Bootstrap a remote VM to run the staging/prod stack:
# - installs docker + compose plugin (if missing)
# - creates directories
# - optionally configures firewall basics (not enabled by default)
#
# Usage:
#   REMOTE_SSH=ubuntu@1.2.3.4 REMOTE_DIR=/opt/inventiv-agents \
#     ./scripts/remote_bootstrap.sh <env> [secrets_dir]
#
# env: staging|prod

ENV_NAME="${1:-}"
SECRETS_DIR_OVERRIDE="${2:-}"

if [[ -z "${ENV_NAME}" ]]; then
  echo "Usage: REMOTE_SSH=... REMOTE_DIR=... $0 <staging|prod> [secrets_dir]"
  exit 2
fi

: "${REMOTE_SSH:?set REMOTE_SSH (ex: ubuntu@51.159.184.73)}"
: "${REMOTE_DIR:?set REMOTE_DIR (ex: /opt/inventiv-agents)}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_ENV_FILE="${REPO_ROOT}/env/${ENV_NAME}.env"
if [[ ! -f "${LOCAL_ENV_FILE}" ]]; then
  echo "Missing env file: ${LOCAL_ENV_FILE}"
  exit 2
fi

SECRETS_DIR="$(grep -E '^SECRETS_DIR=' "${LOCAL_ENV_FILE}" | head -n1 | cut -d= -f2- || true)"
if [[ -n "${SECRETS_DIR_OVERRIDE}" ]]; then
  SECRETS_DIR="${SECRETS_DIR_OVERRIDE}"
fi
if [[ -z "${SECRETS_DIR}" ]]; then
  echo "SECRETS_DIR is missing in ${LOCAL_ENV_FILE}"
  exit 2
fi

echo "==> Bootstrapping ${REMOTE_SSH} (REMOTE_DIR=${REMOTE_DIR}, SECRETS_DIR=${SECRETS_DIR})"

SSH_ID_FILE="${SSH_IDENTITY_FILE:-}"
SSH_EXTRA_OPTS="${SSH_EXTRA_OPTS:-}"
SSH_ID_ARGS=()
TMP_KEY_FILE=""
cleanup_tmp_key() {
  if [[ -n "${TMP_KEY_FILE}" ]]; then
    rm -f "${TMP_KEY_FILE}" >/dev/null 2>&1 || true
  fi
}
trap cleanup_tmp_key EXIT
if [[ -n "${SSH_ID_FILE}" ]]; then
  if [[ ! -f "${SSH_ID_FILE}" && -n "${SSH_PRIVATE_KEY:-}" ]]; then
    TMP_KEY_FILE="$(mktemp)"
    umask 077
    printf '%s\n' "${SSH_PRIVATE_KEY}" > "${TMP_KEY_FILE}"
    chmod 600 "${TMP_KEY_FILE}" || true
    SSH_ID_FILE="${TMP_KEY_FILE}"
  fi
  SSH_ID_ARGS=(-i "${SSH_ID_FILE}")
fi

ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" "set -euo pipefail

if command -v docker >/dev/null 2>&1; then
  echo '[ok] docker already installed'
else
  echo '[..] installing docker'
  if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update -y
    sudo apt-get install -y ca-certificates curl gnupg rsync
    curl -fsSL https://get.docker.com | sudo sh
    # Ensure compose plugin is installed
    sudo apt-get install -y docker-compose-plugin || true
  else
    echo 'Unsupported distro (expected Debian/Ubuntu with apt-get). Install docker manually.'
    exit 2
  fi
fi

sudo mkdir -p '${REMOTE_DIR}' '${REMOTE_DIR}/deploy' '${SECRETS_DIR}'
sudo chown -R \"\$(id -u):\$(id -g)\" '${REMOTE_DIR}'

if groups | grep -q '\\bdocker\\b'; then
  echo '[ok] user already in docker group'
else
  echo '[..] adding user to docker group (re-login may be required)'
  sudo usermod -aG docker \"\$(id -un)\" || true
fi

echo '[ok] bootstrap done'
"
