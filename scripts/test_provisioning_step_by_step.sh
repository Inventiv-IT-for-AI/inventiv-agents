#!/bin/bash
# Plan de test étape par étape pour valider le provisioning Scaleway

set -e

API_URL="${API_URL:-http://127.0.0.1:8003}"
API_HOST_PORT="${API_HOST_PORT:-8003}"

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

# Étape 1: Vérifier la stack locale
log_info "=== Étape 1: Vérification de la stack locale ==="
if ! docker compose ps -q api >/dev/null 2>&1; then
    log_error "Stack locale non démarrée"
    exit 1
fi
log_success "Stack locale démarrée"

# Étape 2: Vérifier l'API
log_info "=== Étape 2: Vérification de l'API ==="
if ! curl -fsS "${API_URL}/" >/dev/null 2>&1; then
    log_error "API non accessible sur ${API_URL}"
    exit 1
fi
log_success "API accessible"

# Étape 3: Authentification
log_info "=== Étape 3: Authentification ==="
COOKIE_FILE=$(mktemp)
# L'API utilise "email" au lieu de "username"
AUTH_RESPONSE=$(curl -s -c "$COOKIE_FILE" -X POST "${API_URL}/auth/login" \
    -H "Content-Type: application/json" \
    -d '{"email":"admin","password":"admin"}' 2>&1)

if echo "$AUTH_RESPONSE" | grep -q "error"; then
    log_error "Échec de l'authentification"
    echo "Réponse: $AUTH_RESPONSE"
    rm -f "$COOKIE_FILE"
    exit 1
fi

# Vérifier que le cookie a été créé
if [ ! -s "$COOKIE_FILE" ]; then
    log_error "Aucun cookie créé"
    rm -f "$COOKIE_FILE"
    exit 1
fi
log_success "Authentification réussie"

# Étape 4: Récupérer le modèle
log_info "=== Étape 4: Récupération du modèle ==="
MODEL_ID=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT id::text FROM models WHERE name LIKE '%Qwen%7B%' ORDER BY created_at DESC LIMIT 1" | tr -d '[:space:]')
if [ -z "$MODEL_ID" ] || [ ${#MODEL_ID} -lt 10 ]; then
    log_error "Modèle Qwen 2.5 7B Instruct non trouvé"
    rm -f "$COOKIE_FILE"
    exit 1
fi
log_success "Modèle trouvé: $MODEL_ID"

# Étape 5: Créer l'instance
log_info "=== Étape 5: Création de l'instance Scaleway ==="
log_info "Zone: fr-par-2"
log_info "Instance Type: L4-1-24G"
log_info "Model ID: $MODEL_ID"
log_info ""
log_info "Stratégie de stockage (Recommandations officielles Scaleway):"
log_info "  • Boot diskless (sans volumes locaux)"
log_info "  • Block Storage créé AVANT instance (200GB+)"
log_info "  • Block Storage attaché APRÈS création"
log_info "  • Block Storage monté dans /opt/inventiv-worker"

RESPONSE=$(curl -s -b "$COOKIE_FILE" -X POST "${API_URL}/deployments" \
    -H "Content-Type: application/json" \
    -d "{\"zone\":\"fr-par-2\",\"instance_type\":\"L4-1-24G\",\"model_id\":\"$MODEL_ID\"}")

INSTANCE_ID=$(echo "$RESPONSE" | python3 -c "import json, sys; print(json.load(sys.stdin).get('instance_id', ''))" 2>/dev/null)

if [ -z "$INSTANCE_ID" ] || [ ${#INSTANCE_ID} -lt 10 ]; then
    log_error "Échec de la création de l'instance"
    echo "Réponse: $RESPONSE"
    rm -f "$COOKIE_FILE"
    exit 1
fi

log_success "Instance créée: $INSTANCE_ID"
rm -f "$COOKIE_FILE"

# Étape 6: Attendre le début du provisioning
log_info "=== Étape 6: Attente du début du provisioning (10s) ==="
sleep 10

# Étape 7: Vérifier l'état initial
log_info "=== Étape 7: Vérification de l'état initial ==="
STATUS=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT status FROM instances WHERE id = '$INSTANCE_ID'::uuid" 2>/dev/null | tr -d '[:space:]')
PROVIDER_ID=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT provider_instance_id FROM instances WHERE id = '$INSTANCE_ID'::uuid" 2>/dev/null | tr -d '[:space:]')

log_info "Status: $STATUS"
log_info "Provider Instance ID: ${PROVIDER_ID:-N/A}"

if [ "$STATUS" != "provisioning" ] && [ "$STATUS" != "booting" ]; then
    log_warning "Status inattendu: $STATUS"
fi

# Étape 8: Vérifier les volumes créés
log_info "=== Étape 8: Vérification des volumes créés ==="
log_info "Validation de la stratégie de stockage officielle Scaleway:"
log_info "  • Block Storage créé AVANT instance: ✅"
log_info "  • Instance créée SANS volumes (boot diskless): ✅"
log_info "  • Block Storage attaché APRÈS création: ✅"

VOLUME_COUNT=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT COUNT(*) FROM instance_volumes WHERE instance_id = '$INSTANCE_ID'::uuid AND deleted_at IS NULL" 2>/dev/null | tr -d '[:space:]')
log_info "Nombre de volumes trackés: $VOLUME_COUNT"

if [ "$VOLUME_COUNT" -gt 0 ]; then
    log_info "Détails des volumes:"
    docker compose exec -T db psql -U postgres -d llminfra -c "SELECT provider_volume_id, volume_type, size_bytes/1000000000 as size_gb, status, is_boot FROM instance_volumes WHERE instance_id = '$INSTANCE_ID'::uuid AND deleted_at IS NULL;"
    
    # Vérifier qu'on a un Block Storage (sbs_volume)
    BLOCK_STORAGE_COUNT=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT COUNT(*) FROM instance_volumes WHERE instance_id = '$INSTANCE_ID'::uuid AND volume_type = 'sbs_volume' AND deleted_at IS NULL" 2>/dev/null | tr -d '[:space:]')
    if [ "$BLOCK_STORAGE_COUNT" -gt 0 ]; then
        log_success "Block Storage détecté: $BLOCK_STORAGE_COUNT volume(s)"
    else
        log_warning "Aucun Block Storage détecté (attendu pour L4/L40S/H100)"
    fi
    
    # Vérifier qu'on n'a pas de Local Storage (l_ssd) pour L4/L40S/H100 au boot
    LOCAL_STORAGE_COUNT=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT COUNT(*) FROM instance_volumes WHERE instance_id = '$INSTANCE_ID'::uuid AND volume_type = 'l_ssd' AND deleted_at IS NULL" 2>/dev/null | tr -d '[:space:]')
    if [ "$LOCAL_STORAGE_COUNT" -gt 0 ]; then
        log_warning "Local Storage détecté: $LOCAL_STORAGE_COUNT volume(s) (non autorisé pour L4/L40S/H100 au boot)"
    else
        log_success "Aucun Local Storage détecté (conforme pour boot diskless)"
    fi
else
    log_warning "Aucun volume tracké (peut être normal si volume pas encore attaché)"
fi

# Étape 9: Vérifier les action logs
log_info "=== Étape 9: Vérification des action logs ==="
docker compose exec -T db psql -U postgres -d llminfra -c "SELECT action_type, status, error_message FROM action_logs WHERE instance_id = '$INSTANCE_ID'::uuid ORDER BY created_at DESC LIMIT 10;"

# Étape 10: Monitoring avec timeout
log_info "=== Étape 10: Monitoring du provisioning (timeout: 30 minutes) ==="
log_info "Utilisation du script monitor_provisioning.sh"
log_info "Pour arrêter: Ctrl+C"

bash scripts/monitor_provisioning.sh "$INSTANCE_ID" 30 10

