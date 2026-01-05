#!/bin/bash
# Script pour monitorer le provisioning d'une instance Scaleway avec timeout

set -e

INSTANCE_ID="${1:-}"
TIMEOUT_MINUTES="${2:-30}"
INTERVAL_SECONDS="${3:-10}"

if [ -z "$INSTANCE_ID" ]; then
    echo "Usage: $0 <instance_id> [timeout_minutes] [interval_seconds]"
    echo "Récupération de la dernière instance..."
    INSTANCE_ID=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT id::text FROM instances WHERE status NOT IN ('terminated', 'archived') ORDER BY created_at DESC LIMIT 1" | tr -d '[:space:]')
    if [ -z "$INSTANCE_ID" ] || [ ${#INSTANCE_ID} -lt 10 ]; then
        echo "❌ Aucune instance trouvée"
        exit 1
    fi
    echo "Instance trouvée: $INSTANCE_ID"
fi

echo "=== Monitoring provisioning: $INSTANCE_ID ==="
echo "Timeout: ${TIMEOUT_MINUTES} minutes"
echo "Interval: ${INTERVAL_SECONDS} secondes"
echo ""

START_TIME=$(date +%s)
TIMEOUT_SECONDS=$((TIMEOUT_MINUTES * 60))
MAX_ITERATIONS=$((TIMEOUT_SECONDS / INTERVAL_SECONDS))

for i in $(seq 1 $MAX_ITERATIONS); do
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - START_TIME))
    ELAPSED_MIN=$((ELAPSED / 60))
    ELAPSED_SEC=$((ELAPSED % 60))
    
    STATUS=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT status FROM instances WHERE id = '$INSTANCE_ID'::uuid" 2>/dev/null | tr -d '[:space:]')
    PROGRESS=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT progress_percent FROM instances WHERE id = '$INSTANCE_ID'::uuid" 2>/dev/null | tr -d '[:space:]')
    IP=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT ip_address FROM instances WHERE id = '$INSTANCE_ID'::uuid" 2>/dev/null | tr -d '[:space:]' | sed 's|/32||')
    WORKER=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT worker_status FROM instances WHERE id = '$INSTANCE_ID'::uuid" 2>/dev/null | tr -d '[:space:]')
    PROVIDER_ID=$(docker compose exec -T db psql -U postgres -d llminfra -t -c "SELECT provider_instance_id FROM instances WHERE id = '$INSTANCE_ID'::uuid" 2>/dev/null | tr -d '[:space:]')
    
    echo "[$i/${MAX_ITERATIONS}] $(date +%H:%M:%S) | Elapsed: ${ELAPSED_MIN}m${ELAPSED_SEC}s | Status: ${STATUS:-N/A} | Progress: ${PROGRESS:-0}% | IP: ${IP:-N/A} | Worker: ${WORKER:-N/A} | Provider: ${PROVIDER_ID:-N/A}"
    
    # Vérifier si terminé avec succès
    if [ "$STATUS" = "ready" ] && [ "$WORKER" = "ready" ]; then
        echo ""
        echo "✅✅✅ Instance PRÊTE ! ✅✅✅"
        echo ""
        echo "=== Volumes attachés ==="
        docker compose exec -T db psql -U postgres -d llminfra -c "SELECT provider_volume_id, volume_type, size_bytes/1000000000 as size_gb, status FROM instance_volumes WHERE instance_id = '$INSTANCE_ID'::uuid"
        exit 0
    fi
    
    # Vérifier si échec
    if [ "$STATUS" = "failed" ]; then
        echo ""
        echo "❌ Échec du provisioning"
        echo ""
        echo "=== Derniers logs ==="
        docker compose logs orchestrator --tail 100 | grep -E "Volume|attach|404|error|PROVIDER" | tail -30
        echo ""
        echo "=== Action logs ==="
        docker compose exec -T db psql -U postgres -d llminfra -c "SELECT action_type, status, error_message FROM action_logs WHERE instance_id = '$INSTANCE_ID'::uuid ORDER BY created_at DESC LIMIT 10;"
        exit 1
    fi
    
    # Vérifier timeout
    if [ $ELAPSED -ge $TIMEOUT_SECONDS ]; then
        echo ""
        echo "⏱️ Timeout atteint (${TIMEOUT_MINUTES} minutes)"
        echo ""
        echo "=== État final ==="
        docker compose exec -T db psql -U postgres -d llminfra -c "SELECT status, progress_percent, worker_status FROM instances WHERE id = '$INSTANCE_ID'::uuid;"
        exit 2
    fi
    
    sleep $INTERVAL_SECONDS
done

echo ""
echo "⏱️ Nombre maximum d'itérations atteint"
exit 2

