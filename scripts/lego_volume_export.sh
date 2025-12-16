#!/usr/bin/env bash
set -euo pipefail

# Export the docker volume that contains lego (ACME) accounts + certificates
# to a local tar.gz file. This allows reusing the wildcard cert across
# ephemeral VMs and even across environments (staging/prod).
#
# Usage:
#   REMOTE_SSH=root@1.2.3.4 SSH_IDENTITY_FILE=./.ssh/key \
#     ./scripts/lego_volume_export.sh <env_file> <output_tar_gz>
#
# Notes:
# - Output file is created/overwritten.
# - Does not print secret material.

ENV_FILE="${1:-}"
OUT_FILE="${2:-}"

if [[ -z "${ENV_FILE}" || -z "${OUT_FILE}" ]]; then
  echo "Usage: REMOTE_SSH=... $0 <env_file> <output_tar_gz>" >&2
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

# Compose project name is pinned by deploy/docker-compose.nginx.yml `name: inventiv-agents`
VOLUME_NAME="inventiv-agents_lego_data"

mkdir -p "$(dirname "${OUT_FILE}")"
umask 077

echo "==> exporting lego_data volume from ${REMOTE_SSH} (domain=${ROOT_DOMAIN})"

TMP_OUT="${OUT_FILE}.tmp"
rm -f "${TMP_OUT}" >/dev/null 2>&1 || true

# Stream tar from remote to local temp file.
ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" \
  "set -euo pipefail; \
   if ! docker volume inspect '${VOLUME_NAME}' >/dev/null 2>&1; then \
     echo '[warn] volume not found; nothing to export' >&2; \
     exit 3; \
   fi; \
   docker run --rm -v '${VOLUME_NAME}:/data:ro' alpine:3.20 \
     sh -lc 'cd /data && tar -czf - .'" > "${TMP_OUT}"

BYTES="$(wc -c < "${TMP_OUT}" | tr -d ' ')"
if [[ "${BYTES}" -lt 256 ]]; then
  echo "[error] exported archive is too small (${BYTES} bytes): ${TMP_OUT}" >&2
  rm -f "${TMP_OUT}" >/dev/null 2>&1 || true
  exit 2
fi

if ! tar -tzf "${TMP_OUT}" >/dev/null 2>&1; then
  echo "[error] exported archive is not a valid tar.gz: ${TMP_OUT}" >&2
  rm -f "${TMP_OUT}" >/dev/null 2>&1 || true
  exit 2
fi

mv -f "${TMP_OUT}" "${OUT_FILE}"
chmod 600 "${OUT_FILE}" || true
echo "[ok] wrote ${OUT_FILE} (${BYTES} bytes)"

