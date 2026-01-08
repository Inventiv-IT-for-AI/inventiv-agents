#!/usr/bin/env bash
set -euo pipefail

# Prune Docker resources for THIS compose project older than N hours (default 168h = 7 days).
# It only prunes *unused* resources (Docker prune semantics) and scopes by compose project label.
#
# Notes:
# - Most compose-created containers/networks/volumes have label: com.docker.compose.project=<project>
# - Images may or may not be labeled depending on build; we try label-based prune first (safe).
# - You can opt into pruning ALL unused images older than N hours with CLEAN_ALL_UNUSED_IMAGES_OLD=1 (more aggressive).

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PROJECT_NAME="${COMPOSE_PROJECT_NAME:-${PROJECT_NAME:-$(basename "$ROOT_DIR")}}"
OLDER_THAN_HOURS="${OLDER_THAN_HOURS:-168}"

if ! [[ "${OLDER_THAN_HOURS}" =~ ^[0-9]+$ ]]; then
  echo "❌ OLDER_THAN_HOURS must be an integer (got: '${OLDER_THAN_HOURS}')" >&2
  exit 2
fi

UNTIL="${OLDER_THAN_HOURS}h"

echo "🧹 Docker prune (project='${PROJECT_NAME}', until='${UNTIL}')"

# Helper: parse RFC3339-ish timestamps to epoch seconds (python, cross-platform).
to_epoch() {
  python3 - <<'PY' "$1"
import sys
from datetime import datetime, timezone
s=sys.argv[1].strip()
if not s:
  print("0"); raise SystemExit(0)
try:
  # Docker often returns RFC3339 with nanoseconds + Z.
  if s.endswith("Z"):
    s=s[:-1]+"+00:00"
  # Trim nanoseconds if present (python fromisoformat supports up to microseconds).
  if "." in s:
    head, tail = s.split(".", 1)
    # tail includes timezone, e.g. 123456789+00:00
    tz = ""
    if "+" in tail:
      frac, tz = tail.split("+", 1)
      tz = "+" + tz
    elif "-" in tail:
      # rare
      frac, tz = tail.split("-", 1)
      tz = "-" + tz
    else:
      frac = tail
    frac = (frac[:6]).ljust(6, "0")
    s = head + "." + frac + tz
  dt=datetime.fromisoformat(s)
  if dt.tzinfo is None:
    dt=dt.replace(tzinfo=timezone.utc)
  print(int(dt.timestamp()))
except Exception:
  print("0")
PY
}

now_epoch="$(date +%s)"
cutoff_epoch="$((now_epoch - (OLDER_THAN_HOURS * 3600)))"

echo "cutoff_epoch=${cutoff_epoch}"

# 1) Containers (stopped only). Docker supports --filter until for container prune.
docker container prune -f \
  --filter "label=com.docker.compose.project=${PROJECT_NAME}" \
  --filter "until=${UNTIL}" >/dev/null || true

# 2) Networks: remove only if (a) project label matches, (b) no attached containers, (c) older than cutoff.
while IFS= read -r net_id; do
  [ -z "$net_id" ] && continue
  created="$(docker network inspect -f '{{.Created}}' "$net_id" 2>/dev/null | tr -d '\r' || true)"
  created_epoch="$(to_epoch "${created:-}")"
  [ "$created_epoch" -eq 0 ] && continue
  if [ "$created_epoch" -gt "$cutoff_epoch" ]; then
    continue
  fi
  # Check usage
  containers_count="$(docker network inspect -f '{{len .Containers}}' "$net_id" 2>/dev/null | tr -d '\r' || echo 1)"
  if [ "${containers_count}" = "0" ]; then
    docker network rm "$net_id" >/dev/null 2>&1 || true
  fi
done < <(docker network ls -q --filter "label=com.docker.compose.project=${PROJECT_NAME}" 2>/dev/null || true)

# 3) Volumes: remove only if (a) project label matches, (b) not used by any container, (c) older than cutoff.
while IFS= read -r vol; do
  [ -z "$vol" ] && continue
  created="$(docker volume inspect -f '{{.CreatedAt}}' "$vol" 2>/dev/null | tr -d '\r' || true)"
  created_epoch="$(to_epoch "${created:-}")"
  [ "$created_epoch" -eq 0 ] && continue
  if [ "$created_epoch" -gt "$cutoff_epoch" ]; then
    continue
  fi
  # Used by any container?
  in_use="$(docker ps -a --filter "volume=${vol}" --format '{{.ID}}' 2>/dev/null | head -n 1 || true)"
  if [ -z "$in_use" ]; then
    docker volume rm "$vol" >/dev/null 2>&1 || true
  fi
done < <(docker volume ls -q --filter "label=com.docker.compose.project=${PROJECT_NAME}" 2>/dev/null || true)

# 4) Images: best-effort, project-name prefix. Remove only if unused and older than cutoff.
# Compose-built images are typically named like: <project>-api, <project>-orchestrator, ...
while IFS= read -r img; do
  [ -z "$img" ] && continue
  created="$(docker image inspect -f '{{.Created}}' "$img" 2>/dev/null | tr -d '\r' || true)"
  created_epoch="$(to_epoch "${created:-}")"
  [ "$created_epoch" -eq 0 ] && continue
  if [ "$created_epoch" -gt "$cutoff_epoch" ]; then
    continue
  fi
  # Skip if any container uses it
  used="$(docker ps -a --filter "ancestor=${img}" --format '{{.ID}}' 2>/dev/null | head -n 1 || true)"
  if [ -z "$used" ]; then
    docker image rm -f "$img" >/dev/null 2>&1 || true
  fi
done < <(docker image ls --format '{{.Repository}}:{{.Tag}}' 2>/dev/null | grep -E "^${PROJECT_NAME}-" || true)

# Optional: prune ALL unused images older than N hours (can affect other repos).
if [ "${CLEAN_ALL_UNUSED_IMAGES_OLD:-0}" = "1" ]; then
  echo "⚠️  CLEAN_ALL_UNUSED_IMAGES_OLD=1 → pruning ALL unused images older than ${UNTIL}"
  docker image prune -af --filter "until=${UNTIL}" >/dev/null || true
fi

# 5) Build cache (global)
docker builder prune -af --filter "until=${UNTIL}" >/dev/null || true

echo "✅ Docker prune done"


