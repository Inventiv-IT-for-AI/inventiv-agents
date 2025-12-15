#!/usr/bin/env bash
set -euo pipefail

# Non-interactive docker login to GHCR using:
# - GHCR_TOKEN env var, or
# - deploy/secrets/ghcr_token file (gitignored)
#
# Username:
# - GHCR_USERNAME env var, or
# - REGISTRY_USERNAME env var
#
# Usage:
#   GHCR_TOKEN=... GHCR_USERNAME=... ./scripts/ghcr_login.sh

TOKEN="${GHCR_TOKEN:-}"
if [[ -z "${TOKEN}" && -f "deploy/secrets/ghcr_token" ]]; then
  TOKEN="$(cat deploy/secrets/ghcr_token)"
fi
if [[ -z "${TOKEN}" ]]; then
  echo "Missing GHCR_TOKEN (or deploy/secrets/ghcr_token). Can't login to GHCR." >&2
  exit 2
fi

USERNAME="${GHCR_USERNAME:-${REGISTRY_USERNAME:-}}"
if [[ -z "${USERNAME}" ]]; then
  if command -v gh >/dev/null 2>&1; then
    USERNAME="$(gh api user -q .login 2>/dev/null || true)"
  fi
fi
if [[ -z "${USERNAME}" ]]; then
  echo "Missing GHCR_USERNAME/REGISTRY_USERNAME and couldn't infer from gh. Can't login to GHCR." >&2
  exit 2
fi

printf '%s' "${TOKEN}" | docker login ghcr.io -u "${USERNAME}" --password-stdin >/dev/null
echo "[ok] docker login ghcr.io as ${USERNAME}"

