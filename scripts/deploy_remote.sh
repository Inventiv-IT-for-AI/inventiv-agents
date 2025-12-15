#!/usr/bin/env bash
set -euo pipefail

ENV_NAME="${1:-}"
ACTION="${2:-}"

if [[ -z "${ENV_NAME}" || -z "${ACTION}" ]]; then
  echo "Usage: $0 <staging|prod> <create|update|start|stop|delete|status|logs|cert|renew>"
  exit 2
fi

: "${REMOTE_SSH:?set REMOTE_SSH (ex: ubuntu@1.2.3.4)}"
: "${REMOTE_DIR:?set REMOTE_DIR (ex: /opt/inventiv-agents)}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_DEPLOY_DIR="${REPO_ROOT}/deploy"
LOCAL_ENV_FILE="${REPO_ROOT}/env/${ENV_NAME}.env"

if [[ ! -f "${LOCAL_ENV_FILE}" ]]; then
  echo "Missing env file: ${LOCAL_ENV_FILE}"
  echo "Create it locally (not committed) with at least:"
  echo "  IMAGE_REGISTRY=..."
  echo "  IMAGE_TAG=..."
  echo "  FRONTEND_DOMAIN=..."
  echo "  API_DOMAIN=..."
  echo "  POSTGRES_PASSWORD=..."
  exit 2
fi

REMOTE_DEPLOY_DIR="${REMOTE_DIR}/deploy"
REMOTE_ENV_FILE="${REMOTE_DEPLOY_DIR}/.env"

SSH_ID_FILE="${SSH_IDENTITY_FILE:-}"
SSH_EXTRA_OPTS="${SSH_EXTRA_OPTS:-}"
SSH_ID_ARGS=()
if [[ -n "${SSH_ID_FILE}" ]]; then
  SSH_ID_ARGS=(-i "${SSH_ID_FILE}")
fi

echo "==> Ensuring remote dir exists: ${REMOTE_SSH}:${REMOTE_DEPLOY_DIR}"
ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" "mkdir -p '${REMOTE_DEPLOY_DIR}'"

echo "==> Syncing deploy assets"
rsync -az --delete \
  -e "ssh ${SSH_ID_ARGS[*]} ${SSH_EXTRA_OPTS}" \
  "${LOCAL_DEPLOY_DIR}/" \
  "${REMOTE_SSH}:${REMOTE_DEPLOY_DIR}/"

echo "==> Uploading env file (.env.${ENV_NAME} -> .env)"
rsync -az -e "ssh ${SSH_ID_ARGS[*]} ${SSH_EXTRA_OPTS}" "${LOCAL_ENV_FILE}" "${REMOTE_SSH}:${REMOTE_ENV_FILE}"

EDGE_ENABLED="${EDGE_ENABLED:-1}" # default: enable edge profile on remote
PROFILE_ARGS=""
if [[ "${EDGE_ENABLED}" == "1" ]]; then
  PROFILE_ARGS="--profile edge"
fi

compose() {
  ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" "cd '${REMOTE_DEPLOY_DIR}' && docker compose --env-file .env -f docker-compose.nginx.yml ${PROFILE_ARGS} $*"
}

ensure_registry_login() {
  # Optional, but required for private registries (e.g. GHCR).
  #
  # We keep credentials out of env files:
  # - put a token in $SECRETS_DIR/ghcr_token (read:packages) on the VM
  # - set REGISTRY_USERNAME (non-secret) in env/<env>.env
  #
  # If the registry is public, this is a no-op.
  ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" "set -euo pipefail; \
    cd '${REMOTE_DEPLOY_DIR}'; \
    IMAGE_REPO=\$(. ./.env >/dev/null 2>&1; echo \"\${IMAGE_REPO:-}\"); \
    REGISTRY_USERNAME=\$(. ./.env >/dev/null 2>&1; echo \"\${REGISTRY_USERNAME:-}\"); \
    SECRETS_DIR=\$(. ./.env >/dev/null 2>&1; echo \"\${SECRETS_DIR:-}\"); \
    if [[ -z \"\$IMAGE_REPO\" ]]; then exit 0; fi; \
    # Extract registry host (everything before first slash)
    REGISTRY_HOST=\${IMAGE_REPO%%/*}; \
    if [[ \"\$REGISTRY_HOST\" != \"ghcr.io\" ]]; then \
      exit 0; \
    fi; \
    if [[ -z \"\$REGISTRY_USERNAME\" ]]; then \
      echo 'REGISTRY_USERNAME is not set in env file (needed for ghcr login)'; exit 2; \
    fi; \
    if [[ -z \"\$SECRETS_DIR\" ]]; then \
      echo 'SECRETS_DIR is not set in env file'; exit 2; \
    fi; \
    TOKEN_FILE=\"\$SECRETS_DIR/ghcr_token\"; \
    if [[ ! -f \"\$TOKEN_FILE\" ]]; then \
      echo \"Missing GHCR token file: \$TOKEN_FILE\"; \
      echo 'Create it on the VM with a GitHub PAT (read:packages).'; \
      exit 2; \
    fi; \
    # Login is idempotent; it updates ~/.docker/config.json
    cat \"\$TOKEN_FILE\" | docker login ghcr.io -u \"\$REGISTRY_USERNAME\" --password-stdin >/dev/null; \
    echo '[ok] registry login ghcr.io' \
  "
}

ensure_secrets_dir() {
  # Ensure secrets directory exists on remote (as declared in env file).
  # shellcheck disable=SC2016
  ssh "${SSH_ID_ARGS[@]}" ${SSH_EXTRA_OPTS} "${REMOTE_SSH}" "set -euo pipefail; \
    cd '${REMOTE_DEPLOY_DIR}'; \
    SECRETS_DIR=\$(. ./.env >/dev/null 2>&1; echo \"\${SECRETS_DIR:-}\"); \
    if [[ -z \"\$SECRETS_DIR\" ]]; then echo 'SECRETS_DIR is not set in env file'; exit 2; fi; \
    if [[ ! -d \"\$SECRETS_DIR\" ]]; then echo \"Secrets dir not found: \$SECRETS_DIR\"; exit 2; fi; \
    if [[ ! -f \"\$SECRETS_DIR/scaleway_access_key\" ]]; then echo \"Missing \$SECRETS_DIR/scaleway_access_key\"; exit 2; fi; \
    if [[ ! -f \"\$SECRETS_DIR/scaleway_secret_key\" ]]; then echo \"Missing \$SECRETS_DIR/scaleway_secret_key\"; exit 2; fi; \
    if [[ ! -f \"\$SECRETS_DIR/llm-studio-key.pub\" ]]; then echo \"Missing \$SECRETS_DIR/llm-studio-key.pub\"; exit 2; fi"
}

case "${ACTION}" in
  create)
    ensure_registry_login
    compose pull
    if [[ "${EDGE_ENABLED}" == "1" ]]; then
      ensure_secrets_dir
      compose run --rm lego
    fi
    compose up -d --remove-orphans
    ;;
  update)
    ensure_registry_login
    compose pull
    if [[ "${EDGE_ENABLED}" == "1" ]]; then
      ensure_secrets_dir
      # On update we prefer renew. If it fails (no cert yet), fall back to run.
      if ! compose run --rm lego renew --days 30; then
        compose run --rm lego
      fi
    fi
    compose up -d --remove-orphans
    ;;
  start)
    compose up -d --remove-orphans
    ;;
  stop)
    compose stop
    ;;
  delete)
    compose down -v
    ;;
  status)
    compose ps
    ;;
  logs)
    compose logs -f --tail=200
    ;;
  cert)
    ensure_secrets_dir
    compose run --rm lego
    ;;
  renew)
    ensure_secrets_dir
    compose run --rm lego renew --days 30
    ;;
  *)
    echo "Unknown action: ${ACTION}"
    exit 2
    ;;
esac
