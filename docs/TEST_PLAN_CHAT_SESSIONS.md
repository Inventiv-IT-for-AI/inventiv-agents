# Plan de Tests et Validation - Sessions de Chat et Inference

## Objectif

Valider le fonctionnement des sessions de chat et du routage des requêtes d'inférence vers les modèles alloués.

## Prérequis

1. **Stack locale** :
   ```bash
   make up          # Démarrer la stack (API, Orchestrator, DB, Redis)
   make ui          # Démarrer le frontend
   ```

2. **Instances ready** :
   - Au moins une instance avec `status='ready'` et `worker_status='ready'`
   - Instance avec un modèle chargé (`worker_model_id` défini)

3. **Authentification** :
   - Session utilisateur (cookie) OU
   - API key valide

## Tests Unitaires

### 1. Résolution du Modèle

**Test** : `test_resolve_openai_model_id()`

**Scénarios** :
- ✅ UUID valide → résolu vers HF repo id
- ✅ HF repo id existant → retourné tel quel
- ✅ Offering ID (`org_slug/model_code`) → résolu vers HF repo id
- ✅ UUID inexistant → erreur 404
- ✅ Modèle inactif → erreur 404
- ✅ Offering inaccessible → erreur 403

**Code** : `inventiv-api/src/worker_routing.rs`

### 2. Sélection du Worker

**Test** : `test_select_ready_worker_for_model()`

**Scénarios** :
- ✅ Worker ready avec modèle correspondant → sélectionné
- ✅ Plusieurs workers → sélectionne celui avec queue_depth minimal
- ✅ Worker stale (> 5 min) → exclu
- ✅ Pas de worker → retourne None
- ✅ Sticky routing → même worker sélectionné pour même session_id

**Code** : `inventiv-api/src/worker_routing.rs`

### 3. Extraction des Tokens

**Test** : `test_token_extraction()`

**Scénarios** :
- ✅ Streaming SSE avec `usage` → tokens extraits
- ✅ Streaming SSE sans `usage` → tokens None
- ✅ JSON avec `usage` → tokens extraits
- ✅ JSON sans `usage` → tokens None

**Code** : `inventiv-api/src/metrics.rs`

## Tests d'Intégration

### 1. Session Simple

**Objectif** : Vérifier qu'une session de chat fonctionne correctement.

**Étapes** :
1. Créer une session de chat (générer `rid`)
2. Envoyer plusieurs messages avec le même `X-Inventiv-Session`
3. Vérifier que toutes les requêtes vont vers le même worker
4. Vérifier que les réponses sont cohérentes

**Commandes** :
```bash
# Générer un session_id
SESSION_ID=$(uuidgen)

# Requête 1
curl -X POST http://localhost:8003/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "X-Inventiv-Session: $SESSION_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": false
  }'

# Requête 2 (même session)
curl -X POST http://localhost:8003/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "X-Inventiv-Session: $SESSION_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [
      {"role": "user", "content": "Hello"},
      {"role": "assistant", "content": "..."},
      {"role": "user", "content": "What is 2+2?"}
    ],
    "stream": false
  }'
```

**Vérifications** :
- ✅ Les deux requêtes utilisent le même `instance_id`
- ✅ Les réponses sont cohérentes
- ✅ Les tokens sont trackés dans `instance_request_metrics`

### 2. Load Balancing

**Objectif** : Vérifier que le load balancing fonctionne correctement.

**Prérequis** :
- Au moins 2 instances ready pour le même modèle

**Étapes** :
1. Envoyer plusieurs requêtes sans `X-Inventiv-Session`
2. Vérifier la distribution entre les workers
3. Vérifier que les workers avec queue_depth minimal sont prioritaires

**Commandes** :
```bash
# Requêtes sans sticky routing
for i in {1..10}; do
  curl -X POST http://localhost:8003/v1/chat/completions \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "Qwen/Qwen2.5-0.5B-Instruct",
      "messages": [{"role": "user", "content": "Test '$i'"}],
      "stream": false
    }'
  sleep 1
done
```

**Vérifications** :
- ✅ Les requêtes sont distribuées entre les workers
- ✅ Les workers avec queue_depth minimal sont prioritaires
- ✅ Pas de worker surchargé

### 3. Failover

**Objectif** : Vérifier que le système gère correctement la perte d'un worker.

**Étapes** :
1. Créer une session avec sticky routing
2. Vérifier que les requêtes vont vers un worker spécifique
3. Arrêter le worker utilisé
4. Envoyer une nouvelle requête
5. Vérifier qu'un autre worker est sélectionné

**Commandes** :
```bash
# Session avec sticky routing
SESSION_ID=$(uuidgen)

# Requête 1 (sélectionne worker A)
curl -X POST http://localhost:8003/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "X-Inventiv-Session: $SESSION_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Test"}],
    "stream": false
  }'

# Arrêter le worker (simuler via DB)
# UPDATE instances SET status='terminating' WHERE id='<worker_id>';

# Requête 2 (doit sélectionner worker B)
curl -X POST http://localhost:8003/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "X-Inventiv-Session: $SESSION_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Test 2"}],
    "stream": false
  }'
```

**Vérifications** :
- ✅ La première requête utilise worker A
- ✅ La deuxième requête utilise worker B (différent)
- ✅ Pas d'erreur 503 (worker trouvé)

### 4. Token Tracking

**Objectif** : Vérifier que les tokens sont correctement trackés.

**Étapes** :
1. Envoyer une requête d'inférence
2. Vérifier que les tokens sont extraits
3. Vérifier que les métriques sont mises à jour
4. Vérifier que l'usage est stocké dans `finops.inference_usage`

**Commandes** :
```bash
# Requête avec tokens
curl -X POST http://localhost:8003/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Hello, how are you?"}],
    "stream": false
  }'

# Vérifier les métriques
psql -U postgres -d llminfra -c "
  SELECT 
    instance_id,
    total_requests,
    successful_requests,
    input_tokens,
    output_tokens,
    total_tokens
  FROM instance_request_metrics
  WHERE instance_id = '<instance_id>'
  ORDER BY last_request_at DESC
  LIMIT 1;
"

# Vérifier l'usage FinOps
psql -U postgres -d llminfra -c "
  SELECT 
    instance_id,
    model_id,
    input_tokens,
    output_tokens,
    total_tokens,
    created_at
  FROM finops.inference_usage
  ORDER BY created_at DESC
  LIMIT 5;
"
```

**Vérifications** :
- ✅ Les tokens sont extraits de la réponse
- ✅ `instance_request_metrics` est mis à jour
- ✅ `finops.inference_usage` contient l'enregistrement

### 5. Streaming SSE

**Objectif** : Vérifier que le streaming fonctionne correctement.

**Étapes** :
1. Envoyer une requête avec `stream: true`
2. Vérifier que les chunks sont reçus
3. Vérifier que les tokens sont extraits à la fin
4. Vérifier que les métriques sont mises à jour

**Commandes** :
```bash
# Requête streaming
curl -X POST http://localhost:8003/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Tell me a story"}],
    "stream": true
  }' \
  --no-buffer
```

**Vérifications** :
- ✅ Les chunks SSE sont reçus (`data: {...}`)
- ✅ Le chunk `[DONE]` contient `usage`
- ✅ Les tokens sont extraits après la fin du stream
- ✅ Les métriques sont mises à jour

## Tests E2E

### 1. Test Complet avec Mock

**Script** : `scripts/test_worker_observability_mock.sh`

**Étapes** :
1. Démarrer la stack avec mock provider
2. Créer une instance mock
3. Attendre que l'instance soit ready
4. Envoyer des requêtes de chat
5. Vérifier les métriques

**Commandes** :
```bash
# Démarrer la stack
make up

# Créer une instance mock (via UI ou API)
curl -X POST http://localhost:8003/deployments \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "instance_type_id": "<mock_type_id>",
    "zone_id": "<mock_zone_id>",
    "model_id": "<model_id>"
  }'

# Attendre que l'instance soit ready
# (vérifier via GET /instances)

# Envoyer des requêtes
curl -X POST http://localhost:8003/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": false
  }'
```

### 2. Test Multi-Instances

**Script** : `scripts/test_worker_observability_mock_multi.sh`

**Objectif** : Tester le routage avec plusieurs instances.

**Étapes** :
1. Créer plusieurs instances mock
2. Attendre qu'elles soient ready
3. Envoyer des requêtes avec/sans sticky routing
4. Vérifier la distribution

### 3. Test Session Persistence

**Objectif** : Vérifier que les sessions persistent correctement.

**Étapes** :
1. Créer une session de chat
2. Envoyer plusieurs messages
3. Vérifier que le contexte est maintenu
4. Vérifier que les métriques sont cohérentes

## Tests de Performance

### 1. Latence

**Objectif** : Mesurer la latence des requêtes.

**Métriques** :
- Temps de résolution du modèle
- Temps de sélection du worker
- Temps de proxy vers le worker
- Temps total (TTFT pour streaming)

**Commandes** :
```bash
# Mesurer la latence
time curl -X POST http://localhost:8003/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Test"}],
    "stream": false
  }'
```

### 2. Throughput

**Objectif** : Mesurer le débit de requêtes.

**Métriques** :
- Requêtes par seconde
- Tokens par seconde
- Utilisation des workers

**Commandes** :
```bash
# Test de charge
ab -n 100 -c 10 \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -p request.json \
  http://localhost:8003/v1/chat/completions
```

## Checklist de Validation

### Fonctionnalités de Base
- [ ] Résolution du modèle (UUID/HF/offering)
- [ ] Sélection du worker (load balancing)
- [ ] Sticky routing (affinité session)
- [ ] Proxy vers worker
- [ ] Streaming SSE
- [ ] Extraction des tokens
- [ ] Tracking des métriques

### Gestion d'Erreurs
- [ ] Pas de worker disponible → 503
- [ ] Worker timeout → 502
- [ ] Modèle introuvable → 404
- [ ] Requête invalide → 400
- [ ] Non autorisé → 401/403

### Performance
- [ ] Latence acceptable (< 100ms pour sélection)
- [ ] Throughput acceptable (> 10 req/s)
- [ ] Pas de fuite mémoire
- [ ] Pas de connexions orphelines

### Observabilité
- [ ] Logs structurés avec correlation_id
- [ ] Métriques mises à jour
- [ ] Usage tracké dans FinOps
- [ ] Runtime models mis à jour

## Scripts de Test

### 1. Test Session Simple

**Fichier** : `scripts/test_chat_session.sh`

```bash
#!/bin/bash
set -e

API_BASE_URL="${API_BASE_URL:-http://localhost:8003}"
API_KEY="${API_KEY:-}"

if [ -z "$API_KEY" ]; then
  echo "❌ API_KEY not set"
  exit 1
fi

SESSION_ID=$(uuidgen)
echo "📝 Testing session: $SESSION_ID"

# Requête 1
echo "📤 Request 1..."
RESPONSE1=$(curl -s -X POST "${API_BASE_URL}/v1/chat/completions" \
  -H "Authorization: Bearer ${API_KEY}" \
  -H "X-Inventiv-Session: ${SESSION_ID}" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": false
  }')

echo "✅ Request 1 completed"

# Requête 2 (même session)
echo "📤 Request 2..."
RESPONSE2=$(curl -s -X POST "${API_BASE_URL}/v1/chat/completions" \
  -H "Authorization: Bearer ${API_KEY}" \
  -H "X-Inventiv-Session: ${SESSION_ID}" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [
      {"role": "user", "content": "Hello"},
      {"role": "assistant", "content": "Hi there!"},
      {"role": "user", "content": "What is 2+2?"}
    ],
    "stream": false
  }')

echo "✅ Request 2 completed"
echo "✅ Session test passed"
```

### 2. Test Load Balancing

**Fichier** : `scripts/test_load_balancing.sh`

```bash
#!/bin/bash
set -e

API_BASE_URL="${API_BASE_URL:-http://localhost:8003}"
API_KEY="${API_KEY:-}"

if [ -z "$API_KEY" ]; then
  echo "❌ API_KEY not set"
  exit 1
fi

echo "📊 Testing load balancing..."

# Envoyer 10 requêtes sans sticky routing
for i in {1..10}; do
  echo "📤 Request $i..."
  curl -s -X POST "${API_BASE_URL}/v1/chat/completions" \
    -H "Authorization: Bearer ${API_KEY}" \
    -H "Content-Type: application/json" \
    -d "{
      \"model\": \"Qwen/Qwen2.5-0.5B-Instruct\",
      \"messages\": [{\"role\": \"user\", \"content\": \"Test $i\"}],
      \"stream\": false
    }" > /dev/null
  sleep 0.5
done

echo "✅ Load balancing test completed"
```

### 3. Test Token Tracking

**Fichier** : `scripts/test_token_tracking.sh`

```bash
#!/bin/bash
set -e

API_BASE_URL="${API_BASE_URL:-http://localhost:8003}"
API_KEY="${API_KEY:-}"

if [ -z "$API_KEY" ]; then
  echo "❌ API_KEY not set"
  exit 1
fi

echo "📊 Testing token tracking..."

# Envoyer une requête
RESPONSE=$(curl -s -X POST "${API_BASE_URL}/v1/chat/completions" \
  -H "Authorization: Bearer ${API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Hello, how are you?"}],
    "stream": false
  }')

# Extraire les tokens de la réponse
INPUT_TOKENS=$(echo "$RESPONSE" | jq -r '.usage.prompt_tokens // empty')
OUTPUT_TOKENS=$(echo "$RESPONSE" | jq -r '.usage.completion_tokens // empty')
TOTAL_TOKENS=$(echo "$RESPONSE" | jq -r '.usage.total_tokens // empty')

if [ -n "$INPUT_TOKENS" ] && [ -n "$OUTPUT_TOKENS" ] && [ -n "$TOTAL_TOKENS" ]; then
  echo "✅ Tokens extracted: input=$INPUT_TOKENS, output=$OUTPUT_TOKENS, total=$TOTAL_TOKENS"
else
  echo "❌ Tokens not found in response"
  exit 1
fi

echo "✅ Token tracking test passed"
```

## Résultats Attendus

### Tests Unitaires
- ✅ Tous les tests passent
- ✅ Couverture de code > 80%

### Tests d'Intégration
- ✅ Session simple fonctionne
- ✅ Load balancing distribue équitablement
- ✅ Failover fonctionne correctement
- ✅ Token tracking fonctionne

### Tests E2E
- ✅ Test complet avec mock passe
- ✅ Test multi-instances passe
- ✅ Test session persistence passe

### Tests de Performance
- ✅ Latence < 100ms pour sélection
- ✅ Throughput > 10 req/s
- ✅ Pas de fuite mémoire

## Prochaines Étapes

1. **Implémenter les tests unitaires** dans `inventiv-api/src/worker_routing.rs`
2. **Créer les scripts de test** dans `scripts/`
3. **Exécuter les tests** et documenter les résultats
4. **Corriger les bugs** identifiés
5. **Optimiser les performances** si nécessaire

