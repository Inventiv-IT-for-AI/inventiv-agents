#!/bin/bash
# Script de vérification de la configuration multi-worktree

set -e

PORT_OFFSET="${PORT_OFFSET:-0}"
DB_HOST_PORT=$((5432 + PORT_OFFSET))
UI_HOST_PORT=$((3000 + PORT_OFFSET))
API_HOST_PORT=$((8003 + PORT_OFFSET))

echo "🔍 Vérification de la configuration multi-worktree (PORT_OFFSET=${PORT_OFFSET})..."
echo ""

# Couleurs
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

ERRORS=0
WARNINGS=0

# Vérifier les ports
check_port() {
    local port=$1
    local description=$2
    if lsof -Pi :${port} -sTCP:LISTEN -t >/dev/null 2>&1; then
        echo -e "${YELLOW}⚠️${NC} $description: Port $port est déjà utilisé"
        ((WARNINGS++))
        return 1
    else
        echo -e "${GREEN}✅${NC} $description: Port $port disponible"
        return 0
    fi
}

# Vérifier les volumes
check_volume() {
    local volume=$1
    local description=$2
    if docker volume inspect "$volume" >/dev/null 2>&1; then
        echo -e "${GREEN}✅${NC} $description: Volume $volume existe"
        return 0
    else
        echo -e "${YELLOW}⚠️${NC} $description: Volume $volume n'existe pas encore (sera créé au premier démarrage)"
        return 1
    fi
}

echo "📊 Ports calculés pour PORT_OFFSET=${PORT_OFFSET}:"
echo "   - UI : ${UI_HOST_PORT}"
echo "   - API : ${API_HOST_PORT}"
echo "   - DB : ${DB_HOST_PORT}"
echo ""

echo "🔌 Vérification des ports..."
check_port "${UI_HOST_PORT}" "UI"
check_port "${API_HOST_PORT}" "API"
check_port "${DB_HOST_PORT}" "DB"
echo ""

echo "💾 Vérification des volumes..."
VOLUME_NAME="inventiv-agents_db_data_${PORT_OFFSET}"
check_volume "${VOLUME_NAME}" "DB volume"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}✅ Configuration multi-worktree OK !${NC}"
    echo ""
    echo "Vous pouvez démarrer avec:"
    echo "  PORT_OFFSET=${PORT_OFFSET} make up"
    echo "  PORT_OFFSET=${PORT_OFFSET} make ui"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo -e "${YELLOW}⚠️  Configuration avec avertissements ($WARNINGS)${NC}"
    echo ""
    echo "Vous pouvez démarrer, mais vérifiez les ports utilisés."
    exit 0
else
    echo -e "${RED}❌ Configuration avec erreurs ($ERRORS erreurs, $WARNINGS avertissements)${NC}"
    exit 1
fi
