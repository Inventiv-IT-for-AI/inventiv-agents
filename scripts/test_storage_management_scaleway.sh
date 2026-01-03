#!/usr/bin/env bash
set -euo pipefail

# Script de test pour la gestion des volumes Scaleway
# Teste : création instance → vérification volumes → terminaison → vérification suppression

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PORT_OFFSET="${PORT_OFFSET:-0}"
API_HOST_PORT="$((8003 + PORT_OFFSET))"
API_BASE_URL="http://127.0.0.1:${API_HOST_PORT}"

DEV_ENV_FILE="${DEV_ENV_FILE:-${ROOT_DIR}/env/dev.env}"

# Couleurs pour output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Variables de test
INSTANCE_ID=""
VOLUME_BOOT_ID=""
VOLUME_DATA_ID=""
SESSION_COOKIE=""

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

# Vérifier que la stack locale est démarrée
check_stack() {
    log_info "Vérification de la stack locale..."
    if ! docker compose ps api 2>/dev/null | grep -q "Up"; then
        log_error "Stack locale non démarrée. Lancez 'make up' d'abord."
        exit 1
    fi
    log_success "Stack locale démarrée"
}

# Login et récupération du cookie de session
login() {
    log_info "Connexion à l'API..."
    
    # Utiliser admin/admin pour dev local
    local username="admin"
    local password="admin"
    
    # Utiliser un fichier de cookies unique pour cette session
    COOKIE_FILE="/tmp/test_cookies_$$.txt"
    rm -f "$COOKIE_FILE"
    
    local response=$(curl -s -c "$COOKIE_FILE" -X POST "${API_BASE_URL}/auth/login" \
        -H "Content-Type: application/json" \
        -d "{\"email\":\"${username}\",\"password\":\"${password}\"}")
    
    if echo "$response" | grep -q "error"; then
        log_error "Échec de la connexion: $response"
        log_info "Tentative avec email complet..."
        # Essayer avec l'email complet si username ne fonctionne pas
        local email=$(docker compose exec -T db psql -U postgres -d llminfra -t -c \
            "SELECT email FROM users WHERE username = 'admin' LIMIT 1" | tr -d '[:space:]')
        if [[ -n "$email" ]]; then
            rm -f "$COOKIE_FILE"
            response=$(curl -s -c "$COOKIE_FILE" -X POST "${API_BASE_URL}/auth/login" \
                -H "Content-Type: application/json" \
                -d "{\"email\":\"${email}\",\"password\":\"${password}\"}")
            if echo "$response" | grep -q "error"; then
                log_error "Échec de la connexion avec email aussi: $response"
                exit 1
            fi
        else
            exit 1
        fi
    fi
    
    # Vérifier que le cookie a été sauvegardé
    if [[ ! -f "$COOKIE_FILE" ]] || [[ ! -s "$COOKIE_FILE" ]]; then
        log_error "Cookie de session non sauvegardé"
        exit 1
    fi
    
    log_success "Connexion réussie"
    export COOKIE_FILE
}

# Récupérer les IDs nécessaires depuis la DB
# Fonction utilitaire pour nettoyer les UUIDs (supprimer tous les caractères non-printables)
clean_uuid() {
    echo "$1" | tr -d '[:space:][:cntrl:]' | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1
}

# Ces fonctions ne sont plus utilisées mais gardées pour référence
get_zone_id() {
    log_info "Récupération de l'ID de la zone fr-par-2..." >&2
    local raw_id=$(docker compose exec -T db psql -U postgres -d llminfra -t -c \
        "SELECT id::text FROM zones WHERE code = 'fr-par-2' LIMIT 1" 2>/dev/null)
    local zone_id=$(clean_uuid "$raw_id")
    if [[ -z "$zone_id" ]]; then
        log_error "Zone fr-par-2 non trouvée dans la DB" >&2
        exit 1
    fi
    echo "$zone_id"
}

get_instance_type_id() {
    log_info "Récupération de l'ID du type d'instance RENDER-S..." >&2
    local raw_id=$(docker compose exec -T db psql -U postgres -d llminfra -t -c \
        "SELECT it.id::text FROM instance_types it JOIN providers p ON it.provider_id = p.id WHERE p.code = 'scaleway' AND it.code = 'RENDER-S' LIMIT 1" 2>/dev/null)
    local instance_type_id=$(clean_uuid "$raw_id")
    if [[ -z "$instance_type_id" ]]; then
        log_error "Type d'instance RENDER-S non trouvé dans la DB" >&2
        exit 1
    fi
    echo "$instance_type_id"
}

get_model_id() {
    log_info "Récupération du modèle Qwen 2.5 7B Instruct..." >&2
    local raw_id=$(docker compose exec -T db psql -U postgres -d llminfra -t -c \
        "SELECT id::text FROM models WHERE is_active = true AND (model_id ILIKE '%qwen%7b%instruct%' OR model_id ILIKE '%Qwen%7B%Instruct%') LIMIT 1" 2>/dev/null)
    local model_id=$(clean_uuid "$raw_id")
    if [[ -z "$model_id" ]]; then
        log_warning "Modèle Qwen 2.5 7B Instruct non trouvé, recherche d'un modèle actif..." >&2
        raw_id=$(docker compose exec -T db psql -U postgres -d llminfra -t -c \
            "SELECT id::text FROM models WHERE is_active = true LIMIT 1" 2>/dev/null)
        model_id=$(clean_uuid "$raw_id")
        if [[ -z "$model_id" ]]; then
            log_error "Aucun modèle actif trouvé dans la DB" >&2
            exit 1
        fi
    fi
    
    # Afficher le nom du modèle sélectionné (sur stderr pour ne pas polluer la sortie)
    local model_name=$(docker compose exec -T db psql -U postgres -d llminfra -t -c \
        "SELECT name || ' (' || model_id || ')' FROM models WHERE id = '${model_id}'::uuid" 2>/dev/null | tr -d '[:space:][:cntrl:]')
    log_info "Modèle sélectionné: $model_name" >&2
    echo "$model_id"
}

# Créer une instance
create_instance() {
    log_info "Création d'une instance Scaleway..."
    # L'API attend les codes (zone, instance_type), pas les UUIDs
    local zone_code="fr-par-2"
    local instance_type_code="RENDER-S"
    local model_id=$(get_model_id)
    
    log_info "Zone: $zone_code"
    log_info "Instance Type: $instance_type_code"
    log_info "Model ID: $model_id"
    
    # Nettoyer model_id au cas où (supprimer tous les caractères non-printables)
    model_id=$(echo "$model_id" | tr -d '[:cntrl:]' | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1)
    
    # Construire le JSON avec jq pour éviter les problèmes d'échappement
    local json_payload=$(jq -n \
        --arg zone "$zone_code" \
        --arg instance_type "$instance_type_code" \
        --arg model_id "$model_id" \
        '{zone: $zone, instance_type: $instance_type, model_id: $model_id}')
    
    log_info "Payload JSON: $json_payload"
    
    local response=$(curl -s -b "${COOKIE_FILE:-/tmp/test_cookies.txt}" -X POST "${API_BASE_URL}/deployments" \
        -H "Content-Type: application/json" \
        -d "$json_payload")
    
    log_info "Réponse brute: $response"
    
    INSTANCE_ID=$(echo "$response" | jq -r '.instance_id // .id // empty' 2>/dev/null || echo "")
    if [[ -z "$INSTANCE_ID" || "$INSTANCE_ID" == "null" ]]; then
        log_error "Échec de la création d'instance: $response"
        exit 1
    fi
    log_success "Instance créée: $INSTANCE_ID"
}

# Vérifier l'état de l'instance dans la DB
check_instance_db() {
    log_info "Vérification de l'instance dans la DB..."
    local status=$(docker compose exec -T db psql -U postgres -d llminfra -t -c \
        "SELECT status::text FROM instances WHERE id = '${INSTANCE_ID}'" | tr -d '[:space:]')
    log_info "Status de l'instance: $status"
    echo "$status"
}

# Vérifier les volumes dans la DB
check_volumes_db() {
    log_info "Vérification des volumes dans la DB (instance_volumes)..."
    docker compose exec -T db psql -U postgres -d llminfra -c "
        SELECT 
            provider_volume_id,
            provider_volume_name,
            volume_type,
            size_bytes / 1000000000 as size_gb,
            is_boot,
            delete_on_terminate,
            status,
            deleted_at IS NULL as active
        FROM instance_volumes
        WHERE instance_id = '${INSTANCE_ID}'
        ORDER BY is_boot DESC, created_at;
    "
}

# Vérifier storage_count et storage_sizes_gb via API
check_storage_api() {
    log_info "Vérification storage_count et storage_sizes_gb via API..."
    local response=$(curl -s -b "${COOKIE_FILE:-/tmp/test_cookies.txt}" "${API_BASE_URL}/instances/${INSTANCE_ID}")
    local storage_count=$(echo "$response" | jq -r '.storage_count // 0')
    local storage_sizes=$(echo "$response" | jq -r '.storage_sizes_gb // []')
    local storages=$(echo "$response" | jq -r '.storages // []')
    
    log_info "storage_count: $storage_count"
    log_info "storage_sizes_gb: $storage_sizes"
    log_info "storages:"
    echo "$storages" | jq '.'
    
    if [[ "$storage_count" == "0" ]]; then
        log_warning "⚠️  storage_count est 0 - aucun volume tracké!"
    else
        log_success "storage_count: $storage_count"
    fi
}

# Vérifier les volumes dans Scaleway (via API Scaleway)
check_volumes_scaleway() {
    log_info "Vérification des volumes dans Scaleway..."
    
    # Récupérer provider_instance_id depuis DB
    local provider_instance_id=$(docker compose exec -T db psql -U postgres -d llminfra -t -c \
        "SELECT provider_instance_id::text FROM instances WHERE id = '${INSTANCE_ID}'" | tr -d '[:space:]')
    
    if [[ -z "$provider_instance_id" || "$provider_instance_id" == "null" ]]; then
        log_warning "provider_instance_id non disponible - instance peut-être encore en création"
        return
    fi
    
    log_info "Provider Instance ID: $provider_instance_id"
    log_info "Pour vérifier dans Scaleway console:"
    log_info "  - Instance: https://console.scaleway.com/instance/servers/${provider_instance_id}"
    log_info "  - Volumes: https://console.scaleway.com/instance/storage"
    
    # Note: On pourrait utiliser l'API Scaleway directement ici, mais pour l'instant
    # on se contente de donner les liens vers la console
}

# Attendre que l'instance soit dans un état stable
wait_for_instance() {
    log_info "Attente que l'instance atteigne un état stable..."
    local max_wait=300  # 5 minutes max
    local waited=0
    local check_interval=10
    
    while [[ $waited -lt $max_wait ]]; do
        local status=$(check_instance_db)
        log_info "Status actuel: $status (attendu depuis ${waited}s)"
        
        case "$status" in
            "ready")
                log_success "Instance prête!"
                return 0
                ;;
            "startup_failed"|"failed"|"terminated")
                log_error "Instance en état d'erreur: $status"
                return 1
                ;;
            *)
                sleep $check_interval
                waited=$((waited + check_interval))
                ;;
        esac
    done
    
    log_error "Timeout: instance n'a pas atteint un état stable après ${max_wait}s"
    return 1
}

# Vérifier les action_logs pour comprendre ce qui s'est passé
check_action_logs() {
    log_info "Vérification des action_logs pour l'instance..."
    docker compose exec -T db psql -U postgres -d llminfra -c "
        SELECT 
            action_type,
            status,
            created_at,
            completed_at,
            error_message,
            metadata->>'size_gb' as size_gb,
            metadata->>'volume_id' as volume_id
        FROM action_logs
        WHERE instance_id = '${INSTANCE_ID}'
        ORDER BY created_at DESC
        LIMIT 20;
    "
}

# Terminer l'instance
terminate_instance() {
    log_info "Terminaison de l'instance..."
    local response=$(curl -s -b "${COOKIE_FILE:-/tmp/test_cookies.txt}" -X DELETE "${API_BASE_URL}/instances/${INSTANCE_ID}")
    
    if echo "$response" | grep -q "error"; then
        log_error "Échec de la terminaison: $response"
        return 1
    fi
    log_success "Commande de terminaison envoyée"
}

# Attendre que l'instance soit terminée
wait_for_termination() {
    log_info "Attente que l'instance soit terminée..."
    local max_wait=300  # 5 minutes max
    local waited=0
    local check_interval=10
    
    while [[ $waited -lt $max_wait ]]; do
        local status=$(check_instance_db)
        log_info "Status actuel: $status (attendu depuis ${waited}s)"
        
        if [[ "$status" == "terminated" ]]; then
            log_success "Instance terminée!"
            return 0
        fi
        
        sleep $check_interval
        waited=$((waited + check_interval))
    done
    
    log_error "Timeout: instance n'a pas été terminée après ${max_wait}s"
    return 1
}

# Vérifier que les volumes ont été supprimés dans Scaleway
check_volumes_deleted() {
    log_info "Vérification que les volumes ont été supprimés..."
    log_info "Vérifiez manuellement dans Scaleway console que les volumes sont supprimés"
    log_info "  - Volumes: https://console.scaleway.com/instance/storage"
    
    # Vérifier dans DB que les volumes sont marqués comme supprimés
    log_info "Vérification dans DB (volumes marqués deleted_at)..."
    docker compose exec -T db psql -U postgres -d llminfra -c "
        SELECT 
            provider_volume_id,
            volume_type,
            size_bytes / 1000000000 as size_gb,
            is_boot,
            deleted_at IS NOT NULL as deleted,
            deleted_at
        FROM instance_volumes
        WHERE instance_id = '${INSTANCE_ID}'
        ORDER BY is_boot DESC;
    "
}

# Nettoyage
cleanup() {
    log_info "Nettoyage..."
    rm -f "${COOKIE_FILE:-/tmp/test_cookies.txt}"
}

# Main
main() {
    log_info "=== Test de gestion des volumes Scaleway ==="
    log_info "API: ${API_BASE_URL}"
    
    trap cleanup EXIT
    
    # Phase 1: Préparation
    check_stack
    login
    
    # Phase 2: Création instance
    create_instance
    log_info "Attente 10s pour que l'instance commence à être créée..."
    sleep 10
    
    # Phase 3: Vérifications après création
    log_info "=== Vérifications après création ==="
    check_instance_db
    check_volumes_db
    check_storage_api
    check_action_logs
    
    # Phase 4: Attendre que l'instance soit prête (optionnel, peut être long)
    read -p "Attendre que l'instance soit prête? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        wait_for_instance
        log_info "=== Vérifications après instance prête ==="
        check_volumes_db
        check_storage_api
        check_volumes_scaleway
    fi
    
    # Phase 5: Terminaison
    read -p "Terminer l'instance maintenant? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        terminate_instance
        log_info "Attente 10s pour que la terminaison commence..."
        sleep 10
        
        # Phase 6: Vérifications après terminaison
        log_info "=== Vérifications après terminaison ==="
        check_action_logs
        
        read -p "Attendre que l'instance soit terminée? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            wait_for_termination
            check_volumes_db
            check_volumes_deleted
        fi
    fi
    
    log_success "=== Test terminé ==="
    log_info "Instance ID: ${INSTANCE_ID}"
    log_info "Vérifiez les résultats ci-dessus et dans Scaleway console"
}

main "$@"

