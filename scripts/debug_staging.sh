#!/usr/bin/env bash
# Script de debug pour staging

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

STG_ENV_FILE="${STG_ENV_FILE:-env/staging.env}"

if [[ ! -f "$STG_ENV_FILE" ]]; then
  echo "❌ Fichier env manquant: $STG_ENV_FILE"
  exit 1
fi

# Charger les variables
set -a
source "$STG_ENV_FILE"
set +a

REMOTE_HOST="${REMOTE_HOST:?REMOTE_HOST manquant dans $STG_ENV_FILE}"
SSH_KEY="${SSH_IDENTITY_FILE:-./.ssh/llm-studio-key}"
REMOTE_USER="${REMOTE_USER:-ubuntu}"
REMOTE_DIR="${REMOTE_DIR:-/opt/inventiv-agents}"

echo "🔍 Debug Staging - $REMOTE_HOST"
echo "=================================="
echo ""

# 1. Test SSH
echo "1️⃣  Test SSH..."
if ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new -o ConnectTimeout=5 "${REMOTE_USER}@${REMOTE_HOST}" "echo 'SSH OK'" >/dev/null 2>&1; then
  echo "   ✅ SSH accessible"
else
  echo "   ❌ SSH inaccessible"
  exit 1
fi

# 2. État des containers
echo ""
echo "2️⃣  État des containers..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new "${REMOTE_USER}@${REMOTE_HOST}" "cd ${REMOTE_DIR}/deploy && docker compose ps -a" || true

# 3. Logs récents (tous services)
echo ""
echo "3️⃣  Logs récents (50 dernières lignes)..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new "${REMOTE_USER}@${REMOTE_HOST}" "cd ${REMOTE_DIR}/deploy && docker compose logs --tail=50 2>&1 | tail -50" || true

# 4. Ports ouverts
echo ""
echo "4️⃣  Ports ouverts (80/443)..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new "${REMOTE_USER}@${REMOTE_HOST}" "netstat -tlnp 2>/dev/null | grep -E ':(80|443)' || ss -tlnp 2>/dev/null | grep -E ':(80|443)' || echo 'Aucun port 80/443 trouvé'" || true

# 5. Configuration nginx
echo ""
echo "5️⃣  Configuration nginx..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new "${REMOTE_USER}@${REMOTE_HOST}" "cd ${REMOTE_DIR}/deploy && docker compose exec nginx cat /etc/nginx/conf.d/default.conf 2>&1 | head -50 || echo 'Nginx non démarré'" || true

# 6. Certificats SSL
echo ""
echo "6️⃣  Certificats SSL..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new "${REMOTE_USER}@${REMOTE_HOST}" "cd ${REMOTE_DIR}/deploy && docker compose exec lego ls -la /var/lib/lego/certificates/ 2>&1 | head -20 || echo 'Lego non démarré ou certs non trouvés'" || true

# 7. Variables d'environnement
echo ""
echo "7️⃣  Variables d'environnement (premières 20)..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new "${REMOTE_USER}@${REMOTE_HOST}" "cd ${REMOTE_DIR}/deploy && cat .env 2>&1 | head -20 || echo 'Fichier .env non trouvé'" || true

# 8. Health checks
echo ""
echo "8️⃣  Health checks (depuis la VM)..."
ssh -i "$SSH_KEY" -o StrictHostKeyChecking=accept-new "${REMOTE_USER}@${REMOTE_HOST}" "curl -s http://localhost/health 2>&1 | head -5 || curl -s http://api:8003/health 2>&1 | head -5 || echo 'Health check échoué'" || true

echo ""
echo "=================================="
echo "✅ Debug terminé"

