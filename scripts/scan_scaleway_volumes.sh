#!/bin/bash
# Script pour scanner l'API Scaleway et lister les volumes Block Storage

set -e

ZONE="${1:-fr-par-2}"
SCALEWAY_SECRET_KEY="${SCALEWAY_SECRET_KEY:-$(docker compose exec -T orchestrator printenv SCALEWAY_SECRET_KEY | tr -d '\r')}"

if [ -z "$SCALEWAY_SECRET_KEY" ]; then
    echo "❌ SCALEWAY_SECRET_KEY not found"
    exit 1
fi

echo "=== Scan des volumes Block Storage Scaleway (zone: $ZONE) ==="
echo ""

# Lister tous les volumes Block Storage
echo "📦 Volumes Block Storage dans la zone $ZONE:"
echo ""

curl -s -H "X-Auth-Token: $SCALEWAY_SECRET_KEY" \
     -H "Content-Type: application/json" \
     "https://api.scaleway.com/block/v1/zones/$ZONE/volumes" | \
python3 -c "
import json
import sys

data = json.load(sys.stdin)
volumes = data.get('volumes', [])

if not volumes:
    print('  Aucun volume trouvé')
else:
    print(f'  Total: {len(volumes)} volumes')
    print('')
    print('  ID                                    | Name                                    | Size (GB) | Status      | Server ID')
    print('  ' + '-' * 100)
    for vol in volumes:
        vol_id = vol.get('id', 'N/A')
        name = vol.get('name', 'N/A')[:40]
        size_bytes = vol.get('size', 0)
        size_gb = size_bytes / 1_000_000_000 if size_bytes else 0
        status = vol.get('status', 'N/A')
        server_id = vol.get('server_id') or 'NOT_ATTACHED'
        print(f'  {vol_id} | {name:<40} | {size_gb:>9.0f} | {status:<11} | {server_id}')
"

echo ""
echo "=== Comparaison avec la DB locale ==="
echo ""

docker compose exec -T db psql -U postgres -d llminfra -c "
SELECT 
    provider_volume_id,
    provider_volume_name,
    volume_type,
    size_bytes/1000000000 as size_gb,
    status,
    instance_id
FROM instance_volumes 
WHERE volume_type = 'sbs_volume' 
  AND deleted_at IS NULL 
ORDER BY created_at DESC 
LIMIT 10;
"

echo ""
echo "=== Test d'attachement pour un volume non attaché ==="
echo ""

# Trouver un volume non attaché
VOLUME_ID=$(curl -s -H "X-Auth-Token: $SCALEWAY_SECRET_KEY" \
     -H "Content-Type: application/json" \
     "https://api.scaleway.com/block/v1/zones/$ZONE/volumes" | \
python3 -c "
import json
import sys

data = json.load(sys.stdin)
volumes = data.get('volumes', [])

for vol in volumes:
    if not vol.get('server_id'):
        print(vol.get('id'))
        break
")

if [ -n "$VOLUME_ID" ]; then
    echo "Volume non attaché trouvé: $VOLUME_ID"
    echo ""
    echo "Détails du volume:"
    curl -s -H "X-Auth-Token: $SCALEWAY_SECRET_KEY" \
         -H "Content-Type: application/json" \
         "https://api.scaleway.com/block/v1/zones/$ZONE/volumes/$VOLUME_ID" | \
    python3 -c "
import json
import sys

vol = json.load(sys.stdin).get('volume', {})
print(f\"  ID: {vol.get('id')}\")
print(f\"  Name: {vol.get('name')}\")
print(f\"  Size: {vol.get('size', 0) / 1_000_000_000} GB\")
print(f\"  Status: {vol.get('status')}\")
print(f\"  Server ID: {vol.get('server_id') or 'NOT_ATTACHED'}\")
print(f\"  Volume Type: {vol.get('volume_type')}\")
"
else
    echo "Aucun volume non attaché trouvé"
fi

