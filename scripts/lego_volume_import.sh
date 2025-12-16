#!/usr/bin/env bash
set -euo pipefail

# Import a local lego_data tar.gz into the remote docker volume.
# This is the counterpart of scripts/lego_volume_export.sh.
#
# Usage:
#   REMOTE_SSH=root@1.2.3.4 SSH_IDENTITY_FILE=./.ssh/key \
#     ./scripts/lego_volume_import.sh <env_file> <input_tar_gz>
#
# Notes:
# - This overwrites the remote volume content (idempotent restore).
# - Safe to run before `make stg-create` / `make prod-create`.

ENV_FILE="${1:-}"
IN_FILE="${2:-}"

if [[ -z "${ENV_FILE}" || -z "${IN_FILE}" ]]; then
  echo "Usage: REMOTE_SSH=... $0 <env_file> <input_tar_gz>" >&2
  exit 2
fi
if [[ ! -f "${ENV_FILE}" ]]; then
  echo "Env file not found: ${ENV_FILE}" >&2
  exit 2
fi
if [[ ! -f "${IN_FILE}" ]]; then
  echo "Input archive not found: ${IN_FILE}" >&2
  exit 2
fi
: "${REMOTE_SSH:?set REMOTE_SSH (ex: root@51.159.184.73)}"

BYTES="$(wc -c < "${IN_FILE}" | tr -d ' ')"
if [[ "${BYTES}" -lt 256 ]]; then
  echo "Input archive is too small (${BYTES} bytes): ${IN_FILE}" >&2
  exit 2
fi
if ! tar -tzf "${IN_FILE}" >/dev/null 2>&1; then
  echo "Input archive is not a valid tar.gz: ${IN_FILE}" >&2
  exit 2
fi

set -a
# shellcheck disable=SC1090
source "${ENV_FILE}"
set +a

ROOT_DOMAIN="${ROOT_DOMAIN:-}"
if [[ -z "${ROOT_DOMAIN}" ]]; then
  echo "ROOT_DOMAIN is not set in ${ENV_FILE}" >&2
  exit 2
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KNOWN_HOSTS_FILE="${SSH_KNOWN_HOSTS_FILE:-${REPO_ROOT}/deploy/known_hosts}"
SSH_EXTRA_OPTS="${SSH_EXTRA_OPTS:-} -o UserKnownHostsFile=${KNOWN_HOSTS_FILE} -o StrictHostKeyChecking=accept-new"

SSH_ID_ARGS=()
if [[ -n "${SSH_IDENTITY_FILE:-}" ]]; then
  SSH_ID_ARGS=(-i "${SSH_IDENTITY_FILE}")
fi

VOLUME_NAME="inventiv-agents_lego_data"
REMOTE_TMP="/tmp/lego_data_${ROOT_DOMAIN}.tar.gz"

echo "==> uploading cert cache to ${REMOTE_SSH}:${REMOTE_TMP}"
rsync -az -e "ssh ${SSH_ID_ARGS[*]} ${SSH_EXTRA_OPTS}" "${IN_FILE}" "${REMOTE_SSH}:${REMOTE_TMP}"

echo "==> restoring ${VOLUME_NAME} on ${REMOTE_SSH}"
ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" \
  "set -euo pipefail; \
   docker volume create '${VOLUME_NAME}' >/dev/null; \
   docker run --rm \
     -v '${VOLUME_NAME}:/data' \
     -v '${REMOTE_TMP}:/tmp/lego.tgz:ro' \
     alpine:3.20 sh -lc 'rm -rf /data/*; mkdir -p /data; tar -xzf /tmp/lego.tgz -C /data'; \
   rm -f '${REMOTE_TMP}' || true; \
   echo '[ok] lego_data restored'"

