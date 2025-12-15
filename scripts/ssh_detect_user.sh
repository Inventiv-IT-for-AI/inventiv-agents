#!/usr/bin/env bash
set -euo pipefail

# Detect SSH username by trying common defaults.
#
# Usage:
#   ./scripts/ssh_detect_user.sh <host> [port]
#
# Prints: "<user>@<host>" on success.

HOST="${1:-}"
PORT="${2:-22}"

if [[ -z "${HOST}" ]]; then
  echo "Usage: $0 <host> [port]" >&2
  exit 2
fi

CANDIDATES="${SSH_USER_CANDIDATES:-root ubuntu debian admin centos ec2-user}"

SSH_ID_FILE="${SSH_IDENTITY_FILE:-}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KNOWN_HOSTS_FILE="${SSH_KNOWN_HOSTS_FILE:-${REPO_ROOT}/deploy/known_hosts}"
SSH_EXTRA_OPTS="${SSH_EXTRA_OPTS:-} -o UserKnownHostsFile=${KNOWN_HOSTS_FILE} -o StrictHostKeyChecking=accept-new"

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

# For ephemeral VMs, host keys change often: remove any previous key entry in our dedicated file.
ssh-keygen -R "${HOST}" -f "${KNOWN_HOSTS_FILE}" >/dev/null 2>&1 || true
ssh-keygen -R "[${HOST}]:${PORT}" -f "${KNOWN_HOSTS_FILE}" >/dev/null 2>&1 || true

for u in ${CANDIDATES}; do
  if ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} \
      -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new \
      -p "${PORT}" "${u}@${HOST}" "echo ok" >/dev/null 2>&1; then
    echo "${u}@${HOST}"
    exit 0
  fi
done

echo "Could not detect SSH user for ${HOST}:${PORT}. Tried: ${CANDIDATES}" >&2
exit 2

