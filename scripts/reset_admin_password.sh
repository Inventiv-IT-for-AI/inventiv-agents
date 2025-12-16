#!/bin/bash
# Script pour réinitialiser le mot de passe admin avec celui du fichier secret
# Usage:
#   Local:  ./scripts/reset_admin_password.sh
#   Remote: ./scripts/reset_admin_password.sh staging|prod

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse arguments
ENV_TYPE="${1:-}"

if [ -n "$ENV_TYPE" ] && [ "$ENV_TYPE" != "staging" ] && [ "$ENV_TYPE" != "prod" ]; then
    echo "Error: Environment must be 'staging' or 'prod' (or empty for local dev)" >&2
    echo "Usage: $0 [staging|prod]" >&2
    exit 1
fi

if [ -n "$ENV_TYPE" ]; then
    # Remote mode (staging/prod)
    ENV_FILE="${PROJECT_ROOT}/env/${ENV_TYPE}.env"
    if [ ! -f "$ENV_FILE" ]; then
        echo "Error: Environment file not found: $ENV_FILE" >&2
        echo "Create it from: env/${ENV_TYPE}.env.example" >&2
        exit 1
    fi

    # Source environment variables
    set -a
    source "$ENV_FILE"
    set +a

    # Check required variables
    if [ -z "${REMOTE_HOST:-}" ] || [ -z "${SECRETS_DIR:-}" ]; then
        echo "Error: REMOTE_HOST and SECRETS_DIR must be set in $ENV_FILE" >&2
        exit 1
    fi

    REMOTE_USER="${REMOTE_USER:-ubuntu}"
    REMOTE_DIR="${REMOTE_DIR:-/opt/inventiv-agents}"
    REMOTE_PORT="${REMOTE_PORT:-22}"
    
    # SSH configuration (similar to deploy_remote.sh pattern)
    SSH_ID_FILE="${SSH_IDENTITY_FILE:-}"
    SSH_OPTS=""
    if [ -n "$SSH_ID_FILE" ]; then
        SSH_OPTS="-i $SSH_ID_FILE"
    fi
    SSH_OPTS="$SSH_OPTS -p $REMOTE_PORT"

    # Detect remote user if not set
    REMOTE_SSH="${REMOTE_USER}@${REMOTE_HOST}"
    if [ -z "${REMOTE_USER:-}" ]; then
        if [ -f "${SCRIPT_DIR}/ssh_detect_user.sh" ]; then
            REMOTE_SSH=$(SSH_IDENTITY_FILE="$SSH_ID_FILE" "${SCRIPT_DIR}/ssh_detect_user.sh" "$REMOTE_HOST" "$REMOTE_PORT")
        else
            REMOTE_SSH="$REMOTE_HOST"
        fi
    fi

    echo "Resetting admin password on ${ENV_TYPE} (${REMOTE_SSH})..."

    # Read password from remote secrets directory
    PASSWORD=$(ssh $SSH_OPTS "$REMOTE_SSH" "cat ${SECRETS_DIR}/default_admin_password 2>/dev/null" | tr -d '\n\r')

    if [ -z "$PASSWORD" ]; then
        echo "Error: Could not read password from ${REMOTE_SSH}:${SECRETS_DIR}/default_admin_password" >&2
        echo "Make sure the secrets are synced with: make ${ENV_TYPE}-secrets-sync" >&2
        exit 1
    fi

    # Execute SQL on remote via docker compose
    # Use the same compose file pattern as deploy_remote.sh
    ssh $SSH_OPTS "$REMOTE_SSH" bash <<EOF
set -e
cd ${REMOTE_DIR}
# Load environment from deploy/.env if it exists
if [ -f deploy/.env ]; then
    set -a
    source deploy/.env
    set +a
fi
# Use docker compose with the deploy compose file
docker compose -f deploy/docker-compose.deploy.yml exec -T db psql -U postgres -d llminfra <<SQL
-- Réinitialiser le mot de passe admin avec le mot de passe du fichier secret
UPDATE users
SET password_hash = crypt('${PASSWORD}', gen_salt('bf')),
    updated_at = NOW()
WHERE username = 'admin';
SELECT username, email, role FROM users WHERE username = 'admin';
SQL
EOF

    if [ $? -eq 0 ]; then
        echo "✓ Admin password reset successfully on ${ENV_TYPE}!"
        echo "  Username: admin"
        echo "  Password: (from ${REMOTE_SSH}:${SECRETS_DIR}/default_admin_password)"
    else
        echo "✗ Failed to reset admin password on ${ENV_TYPE}" >&2
        exit 1
    fi
else
    # Local mode (dev)
    SECRETS_DIR="${SECRETS_DIR:-$PROJECT_ROOT/deploy/secrets}"
    PASSWORD_FILE="${SECRETS_DIR}/default_admin_password"

    if [ ! -f "$PASSWORD_FILE" ]; then
        echo "Error: Password file not found at $PASSWORD_FILE" >&2
        exit 1
    fi

    PASSWORD=$(cat "$PASSWORD_FILE" | tr -d '\n\r')

    if [ -z "$PASSWORD" ]; then
        echo "Error: Password file is empty" >&2
        exit 1
    fi

    echo "Resetting admin password from $PASSWORD_FILE..."

    # Utiliser docker compose exec pour exécuter la commande SQL
    docker compose exec -T db psql -U postgres -d llminfra <<EOF
-- Réinitialiser le mot de passe admin avec le mot de passe du fichier secret
UPDATE users
SET password_hash = crypt('${PASSWORD}', gen_salt('bf')),
    updated_at = NOW()
WHERE username = 'admin';
SELECT username, email, role FROM users WHERE username = 'admin';
EOF

    if [ $? -eq 0 ]; then
        echo "✓ Admin password reset successfully!"
        echo "  Username: admin"
        echo "  Password: (from $PASSWORD_FILE)"
    else
        echo "✗ Failed to reset admin password" >&2
        exit 1
    fi
fi

