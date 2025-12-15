#!/usr/bin/env bash
set -euo pipefail

# Securely create/update a secret file on the remote VM in $SECRETS_DIR.
#
# Usage:
#   REMOTE_SSH=root@1.2.3.4 REMOTE_DIR=/opt/inventiv-agents SSH_IDENTITY_FILE=./.ssh/key \
#     ./scripts/remote_set_secret.sh env/staging.env ghcr_token
#
# Provide secret via:
#   - GHCR_TOKEN env var (recommended in CI), or
#   - interactive prompt (no echo)

ENV_FILE="${1:-}"
SECRET_NAME="${2:-}"

if [[ -z "${ENV_FILE}" || -z "${SECRET_NAME}" ]]; then
  echo "Usage: $0 <env_file> <secret_name>" >&2
  exit 2
fi

: "${REMOTE_SSH:?set REMOTE_SSH (ex: root@51.159.184.73)}"

if [[ ! -f "${ENV_FILE}" ]]; then
  echo "Env file not found: ${ENV_FILE}" >&2
  exit 2
fi

set -a
# shellcheck disable=SC1090
source "${ENV_FILE}"
set +a

SECRETS_DIR="${SECRETS_DIR:-}"
if [[ -z "${SECRETS_DIR}" ]]; then
  echo "SECRETS_DIR is not set in ${ENV_FILE}" >&2
  exit 2
fi

SSH_ID_FILE="${SSH_IDENTITY_FILE:-}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KNOWN_HOSTS_FILE="${SSH_KNOWN_HOSTS_FILE:-${REPO_ROOT}/deploy/known_hosts}"
SSH_EXTRA_OPTS="${SSH_EXTRA_OPTS:-} -o UserKnownHostsFile=${KNOWN_HOSTS_FILE} -o StrictHostKeyChecking=accept-new"
SSH_ID_ARGS=()
if [[ -n "${SSH_ID_FILE}" ]]; then
  SSH_ID_ARGS=(-i "${SSH_ID_FILE}")
fi

VALUE="${GHCR_TOKEN:-}"
if [[ -z "${VALUE}" ]]; then
  if [[ "${SECRET_NAME}" == "ghcr_token" ]]; then
    echo -n "Enter GHCR token (PAT, scope read:packages): " >&2
  else
    echo -n "Enter secret value for ${SECRET_NAME}: " >&2
  fi
  # shellcheck disable=SC2162
  read -s VALUE
  echo "" >&2
fi

if [[ -z "${VALUE}" ]]; then
  echo "Secret value is empty; aborting." >&2
  exit 2
fi

REMOTE_PATH="${SECRETS_DIR}/${SECRET_NAME}"

echo "==> Writing secret to ${REMOTE_SSH}:${REMOTE_PATH}"
printf '%s' "${VALUE}" | ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" "set -euo pipefail; \
  sudo mkdir -p '${SECRETS_DIR}'; \
  sudo sh -c 'umask 077; cat > \"${REMOTE_PATH}\"'; \
  sudo chmod 600 '${REMOTE_PATH}'; \
  echo '[ok] secret written' \
"

