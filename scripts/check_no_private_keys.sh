#!/usr/bin/env bash
set -euo pipefail

# Fails if a likely private key is committed (tracked by git).
# We only scan tracked files to avoid noise from local-only ignored secrets.

cd "$(git rev-parse --show-toplevel)"

echo "==> Checking tracked files for private keys / sensitive material"

# 1) Path-based denylist (fast)
# - .ssh should never be committed
# - common private-key extensions / names
DENY_PATH_REGEX='(^|/)\.ssh/|(^|/)(id_rsa|id_ed25519)(\.pub)?$|\.pem$|\.p12$|\.pfx$|\.key$|(^|/)llm-studio-key$'

if git ls-files | grep -E -n "${DENY_PATH_REGEX}" >/dev/null 2>&1; then
  echo "❌ Found forbidden tracked file paths (possible secrets/keys):" >&2
  git ls-files | grep -E -n "${DENY_PATH_REGEX}" >&2
  exit 2
fi

# 2) Content-based check (headers used by common private keys)
# Scan tracked files only (NUL-safe). Skip binaries with grep -I.
# We intentionally ignore commented lines (starting with '#') to allow documentation examples.
KEY_HEADER_REGEX='^[[:space:]]*[^#].*-----BEGIN (OPENSSH|RSA|EC) PRIVATE KEY-----|^[[:space:]]*[^#].*-----BEGIN PRIVATE KEY-----'

hit=0
while IFS= read -r -d '' f; do
  # Best-effort skip huge vendor files (still tracked) if any; keep simple.
  if grep -I -n -E "${KEY_HEADER_REGEX}" "$f" >/dev/null 2>&1; then
    if [[ "${hit}" -eq 0 ]]; then
      echo "❌ Found private key header(s) in tracked files:" >&2
    fi
    echo " - ${f}" >&2
    grep -I -n -E "${KEY_HEADER_REGEX}" "$f" >&2 || true
    hit=1
  fi
done < <(git ls-files -z)

if [[ "${hit}" -ne 0 ]]; then
  exit 2
fi

echo "✅ No private keys detected in tracked files"


