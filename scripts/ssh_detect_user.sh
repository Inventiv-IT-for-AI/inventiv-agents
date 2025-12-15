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
SSH_EXTRA_OPTS="${SSH_EXTRA_OPTS:-}"

SSH_ID_ARGS=()
if [[ -n "${SSH_ID_FILE}" ]]; then
  SSH_ID_ARGS=(-i "${SSH_ID_FILE}")
fi

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

