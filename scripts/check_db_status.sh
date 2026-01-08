#!/bin/bash
# Script de vérification de l'état de la DB et du seeding

set -e

PORT_OFFSET="${PORT_OFFSET:-0}"
DB_NAME="${POSTGRES_DB:-inventiv-agents}"

echo "🔍 Vérification de l'état de la DB (PORT_OFFSET=${PORT_OFFSET}, DB=${DB_NAME})..."
echo ""

# Couleurs
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

ERRORS=0
WARNINGS=0

# Fonction pour exécuter une requête SQL
run_query() {
    docker compose exec -T db psql -U postgres -d "${DB_NAME}" -c "$1" 2>&1
}

# 1. Vérifier que le conteneur DB est up
echo "📦 Vérification du conteneur DB..."
if docker compose ps db | grep -q "Up"; then
    echo -e "${GREEN}✅${NC} Conteneur DB est démarré"
else
    echo -e "${RED}❌${NC} Conteneur DB n'est pas démarré"
    ((ERRORS++))
    exit 1
fi
echo ""

# 2. Vérifier la connexion à la DB
echo "🔌 Vérification de la connexion à la DB..."
if run_query "SELECT 1;" >/dev/null 2>&1; then
    echo -e "${GREEN}✅${NC} Connexion à la DB réussie"
else
    echo -e "${RED}❌${NC} Impossible de se connecter à la DB"
    ((ERRORS++))
    exit 1
fi
echo ""

# 3. Vérifier les migrations
echo "📋 Vérification des migrations..."
MIGRATIONS=$(run_query "SELECT COUNT(*) FROM _sqlx_migrations;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')
if [ -n "$MIGRATIONS" ] && [ "$MIGRATIONS" -gt 0 ]; then
    echo -e "${GREEN}✅${NC} Migrations appliquées: $MIGRATIONS"
    echo "   Dernières migrations:"
    run_query "SELECT version, description FROM _sqlx_migrations ORDER BY version DESC LIMIT 5;" | grep -E "^[[:space:]]*[0-9]" | head -5 | sed 's/^/     - /'
else
    echo -e "${RED}❌${NC} Aucune migration trouvée"
    ((ERRORS++))
fi
echo ""

# 4. Vérifier le seeding du catalog
echo "📚 Vérification du seeding du catalog..."
PROVIDERS=$(run_query "SELECT COUNT(*) FROM providers;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')
REGIONS=$(run_query "SELECT COUNT(*) FROM regions;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')
ZONES=$(run_query "SELECT COUNT(*) FROM zones;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')
INSTANCE_TYPES=$(run_query "SELECT COUNT(*) FROM instance_types;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')
MODELS=$(run_query "SELECT COUNT(*) FROM models;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')

if [ -n "$PROVIDERS" ] && [ "$PROVIDERS" -gt 0 ]; then
    echo -e "${GREEN}✅${NC} Providers: $PROVIDERS"
    run_query "SELECT name FROM providers ORDER BY name;" | grep -E "^[[:space:]]*[A-Z]" | grep -v "name" | sed 's/^/   - /'
else
    echo -e "${YELLOW}⚠️${NC}  Aucun provider trouvé"
    ((WARNINGS++))
fi

if [ -n "$REGIONS" ] && [ "$REGIONS" -gt 0 ]; then
    echo -e "${GREEN}✅${NC} Regions: $REGIONS"
else
    echo -e "${YELLOW}⚠️${NC}  Aucune région trouvée"
    ((WARNINGS++))
fi

if [ -n "$ZONES" ] && [ "$ZONES" -gt 0 ]; then
    echo -e "${GREEN}✅${NC} Zones: $ZONES"
else
    echo -e "${YELLOW}⚠️${NC}  Aucune zone trouvée"
    ((WARNINGS++))
fi

if [ -n "$INSTANCE_TYPES" ] && [ "$INSTANCE_TYPES" -gt 0 ]; then
    echo -e "${GREEN}✅${NC} Instance Types: $INSTANCE_TYPES"
else
    echo -e "${YELLOW}⚠️${NC}  Aucun instance type trouvé"
    ((WARNINGS++))
fi

if [ -n "$MODELS" ] && [ "$MODELS" -gt 0 ]; then
    echo -e "${GREEN}✅${NC} Models: $MODELS"
else
    echo -e "${YELLOW}⚠️${NC}  Aucun model trouvé"
    ((WARNINGS++))
fi
echo ""

# 5. Vérifier le bootstrap admin
echo "👤 Vérification du bootstrap admin..."
USERS=$(run_query "SELECT COUNT(*) FROM users;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')
if [ -n "$USERS" ] && [ "$USERS" -gt 0 ]; then
    echo -e "${GREEN}✅${NC} Users: $USERS"
    echo "   Admin user:"
    run_query "SELECT username, email, role FROM users LIMIT 1;" | grep -E "^[[:space:]]*[a-z]" | sed 's/^/   - /'
else
    echo -e "${RED}❌${NC} Aucun utilisateur trouvé (admin non créé)"
    ((ERRORS++))
fi
echo ""

# 6. Vérifier le bootstrap organization
echo "🏢 Vérification du bootstrap organization..."
ORGS=$(run_query "SELECT COUNT(*) FROM organizations;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')
if [ -n "$ORGS" ] && [ "$ORGS" -gt 0 ]; then
    echo -e "${GREEN}✅${NC} Organizations: $ORGS"
    echo "   Default organization:"
    run_query "SELECT name, slug FROM organizations LIMIT 1;" | grep -E "^[[:space:]]*[A-Z]" | sed 's/^/   - /'
    
    # Vérifier les membreships
    MEMBERSHIPS=$(run_query "SELECT COUNT(*) FROM organization_memberships;" | grep -E "^[[:space:]]*[0-9]+" | tr -d ' ')
    if [ -n "$MEMBERSHIPS" ] && [ "$MEMBERSHIPS" -gt 0 ]; then
        echo -e "${GREEN}✅${NC} Organization memberships: $MEMBERSHIPS"
        echo "   Memberships:"
        run_query "SELECT u.username, o.name as org_name, om.role FROM organization_memberships om JOIN users u ON om.user_id = u.id JOIN organizations o ON om.organization_id = o.id;" | grep -E "^[[:space:]]*[a-z]" | sed 's/^/   - /'
    else
        echo -e "${YELLOW}⚠️${NC}  Aucun membership trouvé"
        ((WARNINGS++))
    fi
else
    echo -e "${RED}❌${NC} Aucune organisation trouvée (default org non créée)"
    ((ERRORS++))
fi
echo ""

# Résumé
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}✅ DB est opérationnelle et correctement seedée !${NC}"
    echo ""
    echo "Résumé:"
    echo "  - Migrations: $MIGRATIONS appliquées"
    echo "  - Catalog: $PROVIDERS providers, $REGIONS regions, $ZONES zones, $INSTANCE_TYPES instance types, $MODELS models"
    echo "  - Users: $USERS (admin créé)"
    echo "  - Organizations: $ORGS (default org créée)"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo -e "${YELLOW}⚠️  DB opérationnelle avec avertissements ($WARNINGS)${NC}"
    echo ""
    echo "La DB fonctionne mais certains éléments du seeding peuvent être incomplets."
    exit 0
else
    echo -e "${RED}❌ DB avec erreurs ($ERRORS erreurs, $WARNINGS avertissements)${NC}"
    echo ""
    echo "Vérifiez les logs:"
    echo "  docker compose logs api"
    echo "  docker compose logs db"
    exit 1
fi
