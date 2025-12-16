#!/usr/bin/env bash
set -euo pipefail

# Creates/validates deploy/secrets/ghcr_token (gitignored).
#
# Source priority:
#  1) GHCR_TOKEN env var (CI-friendly)
#  2) existing deploy/secrets/ghcr_token (validate only)
#  3) interactive masked prompt (TTY)
#
# It also validates the token scopes via GitHub API headers and fails fast if
# read:packages is missing (required for pulling private GHCR images).

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOKEN_FILE="${REPO_ROOT}/deploy/secrets/ghcr_token"

mkdir -p "$(dirname "${TOKEN_FILE}")"

TOKEN="${GHCR_TOKEN:-}"
MODE="env"

if [[ -z "${TOKEN}" && -f "${TOKEN_FILE}" ]]; then
  TOKEN="$(cat "${TOKEN_FILE}")"
  MODE="existing"
fi

if [[ -z "${TOKEN}" ]]; then
  if [[ -t 0 ]]; then
    echo -n "Enter GHCR classic PAT (needs read:packages): " >&2
    # shellcheck disable=SC2162
    read -s TOKEN
    echo "" >&2
    MODE="prompt"
  else
    echo "Missing GHCR_TOKEN and ${TOKEN_FILE} does not exist (non-interactive)." >&2
    exit 2
  fi
fi

if [[ -z "${TOKEN}" ]]; then
  echo "Token is empty; aborting." >&2
  exit 2
fi

if [[ "${MODE}" != "existing" ]]; then
  umask 077
  printf '%s' "${TOKEN}" > "${TOKEN_FILE}"
  chmod 600 "${TOKEN_FILE}" || true
  echo "[ok] wrote ${TOKEN_FILE}"
else
  echo "[ok] using existing ${TOKEN_FILE}"
fi

# Validate scopes (do not print the token).
SCOPES_HEADER="$(curl -sI -H "Authorization: token ${TOKEN}" https://api.github.com/user | awk -F': ' 'BEGIN{IGNORECASE=1} /^x-oauth-scopes:/{print $2}' | tr -d '\r')"
if [[ -z "${SCOPES_HEADER}" ]]; then
  echo "[warn] couldn't read x-oauth-scopes header (token may be invalid or GitHub blocked the request)" >&2
else
  echo "[info] token scopes: ${SCOPES_HEADER}"
fi

# Normalize for robust matching (GitHub returns comma+space separated scopes).
SCOPES_NORM="$(echo "${SCOPES_HEADER}" | tr -d ' ')"

# For classic PATs, GHCR pulls can succeed with either read:packages or write:packages
# (write typically implies read). We accept either to avoid false negatives.
if echo ",${SCOPES_NORM}," | grep -qi ",read:packages,"; then
  echo "[ok] token has read:packages"
  exit 0
fi
if echo ",${SCOPES_NORM}," | grep -qi ",write:packages,"; then
  echo "[ok] token has write:packages (assumed sufficient for pulls)"
  exit 0
fi

echo "[error] token is missing read:packages (and write:packages). Pulls from private GHCR will fail." >&2
echo "        Regenerate a classic PAT with read:packages (and authorize SSO for the org if applicable)." >&2
exit 2
