# Chat Sessions et Inference avec Modèles Alloués

## Vue d'ensemble

Ce document décrit le fonctionnement des sessions de chat et du routage des requêtes d'inférence vers les modèles alloués dans Inventiv Agents.

## Architecture des Sessions de Chat

### 1. Sticky Routing (Affinité de Session)

Le système utilise le header `X-Inventiv-Session` pour maintenir l'affinité entre les requêtes d'une même session et un worker spécifique.

**Flux** :
```
Client (UI) → API → Worker Selection → Worker (vLLM)
   ↓              ↓           ↓              ↓
Session ID   Sticky Key   Hash-based    HAProxy
(rid)        Extraction   Selection     Sticky
```

**Implémentation** :
- **Frontend** : Génère un `rid` (session ID) unique par session de chat
- **API** : Extrait `X-Inventiv-Session` du header et l'utilise pour le routage sticky
- **Worker Routing** : Utilise un hash stable du session ID pour sélectionner le même worker parmi les workers disponibles
- **HAProxy** (si multi-vLLM) : Reçoit le header `x-inventiv-session` pour maintenir l'affinité au niveau du worker

**Code** :
- Frontend : `inventiv-frontend/src/app/(app)/chat/page.tsx` (ligne 237, 271)
- Frontend : `inventiv-frontend/src/app/(app)/workbench/page.tsx` (ligne 297)
- API : `inventiv-api/src/openai_proxy.rs` (ligne 61)
- Routing : `inventiv-api/src/worker_routing.rs` (ligne 96-104)

### 2. Résolution du Modèle

Le système supporte plusieurs formats d'identifiants de modèles :

**Formats acceptés** :
1. **UUID** : Identifiant interne (`models.id`) → résolu vers `models.model_id` (HF repo id)
2. **HF Repo ID** : Identifiant HuggingFace (ex: `Qwen/Qwen2.5-0.5B-Instruct`)
3. **Organization Offering ID** : Format `org_slug/model_code` (pour modèles partagés entre orgs)

**Résolution** :
- `worker_routing::resolve_openai_model_id()` dans `inventiv-api/src/worker_routing.rs`
- Vérifie d'abord si c'est un UUID → résout vers HF repo id
- Vérifie ensuite si c'est un modèle public (HF repo id)
- Enfin, vérifie si c'est un offering d'organisation (nécessite session utilisateur)

### 3. Sélection du Worker

**Critères de sélection** :
- `status = 'ready'` : Instance prête
- `worker_status = 'ready'` : Worker prêt (ou NULL pour compatibilité)
- `worker_model_id = requested_model` : Modèle correspondant
- **Freshness** : `worker_last_heartbeat` ou `last_health_check` récent (< `OPENAI_WORKER_STALE_SECONDS`, défaut: 300s)

**Load Balancing** :
- **Priorité 1** : `worker_queue_depth` ASC (moins de queue = priorité)
- **Priorité 2** : Freshness DESC (worker le plus récent)
- **Priorité 3** : `created_at` DESC (instance la plus récente)

**Sticky Routing** :
- Si `X-Inventiv-Session` est fourni, utilise un hash stable pour sélectionner le même worker
- Hash : `stable_hash_u64(session_id) % workers.len()`
- Garantit que les requêtes d'une même session vont vers le même worker (meilleur effort)

**Code** :
- `worker_routing::select_ready_worker_for_model()` dans `inventiv-api/src/worker_routing.rs`

## Flux d'Inférence

### 1. Requête Client → API

```
POST /v1/chat/completions
Headers:
  Authorization: Bearer <api_key> (ou cookie de session)
  X-Inventiv-Session: <session_id> (optionnel)
Body:
  {
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [...],
    "stream": true
  }
```

### 2. Traitement API

**Étapes** :
1. **Authentification** : `auth::require_user_or_api_key()` vérifie session ou API key
2. **Résolution modèle** : `resolve_openai_model_id()` convertit l'ID en HF repo id
3. **Sélection worker** : `select_ready_worker_for_model()` trouve un worker ready
4. **Proxy** : Envoie la requête au worker sélectionné

**Gestion d'erreurs** :
- **Pas de worker** : `503 Service Unavailable` avec `error: "no_ready_worker"`
- **Timeout** : `502 Bad Gateway` avec `error: "upstream_unreachable"`
- **Modèle introuvable** : `404 Not Found` avec `error: "model_not_found"`

### 3. Réponse Worker → Client

**Streaming (SSE)** :
- Chaque chunk est forwardé au client
- Tokens extraits à la fin du stream
- Métriques mises à jour (succès/échec, tokens)

**Non-streaming** :
- Réponse JSON complète
- Tokens extraits immédiatement
- Métriques mises à jour

**Extraction des tokens** :
- **Streaming** : Parse SSE pour trouver `usage` dans les chunks `[DONE]`
- **Non-streaming** : Parse JSON pour `usage.prompt_tokens`, `usage.completion_tokens`, `usage.total_tokens`

**Code** :
- `openai_proxy::proxy_to_worker()` dans `inventiv-api/src/openai_proxy.rs`
- `openai_proxy::handle_streaming_response()` pour SSE
- `openai_proxy::handle_non_streaming_response()` pour JSON

## Tracking et Métriques

### 1. Métriques par Instance

**Table** : `instance_request_metrics`
- `total_requests` : Nombre total de requêtes
- `successful_requests` : Requêtes réussies
- `failed_requests` : Requêtes échouées
- `input_tokens` : Tokens d'entrée cumulés
- `output_tokens` : Tokens de sortie cumulés
- `total_tokens` : Tokens totaux cumulés
- `first_request_at` : Première requête
- `last_request_at` : Dernière requête

**Mise à jour** :
- `metrics::update_instance_request_metrics()` après chaque requête
- Incrémente les compteurs et met à jour les timestamps

### 2. Usage FinOps

**Table** : `finops.inference_usage`
- Enregistre chaque requête d'inférence avec :
  - `instance_id` : Instance utilisée
  - `model_id` : Modèle utilisé (UUID)
  - `input_tokens`, `output_tokens`, `total_tokens`
  - `user_id` : Utilisateur (si session)
  - `api_key_id` : API key (si utilisé)
  - `created_at` : Timestamp

**Stockage** :
- `metrics::store_inference_usage()` après extraction des tokens
- Permet le calcul des coûts et la facturation

### 3. Runtime Models

**Table** : `runtime_models`
- Track les modèles réellement servis par les workers
- `model_id` : HF repo id
- `first_seen_at`, `last_seen_at` : Première/dernière utilisation
- `total_requests`, `failed_requests` : Compteurs

**Mise à jour** :
- `worker_routing::bump_runtime_model_counters()` après chaque requête
- Utilisé pour `/v1/models` et `/runtime/models`

## Gestion des Sessions

### 1. Session ID (Frontend)

**Génération** :
- `rid = crypto.randomUUID()` dans le frontend
- Unique par session de chat
- Persiste pendant la durée de la session (jusqu'à refresh de la page)

**Utilisation** :
- Envoyé dans `X-Inventiv-Session` header
- Utilisé pour sticky routing
- Permet de maintenir le contexte entre requêtes

### 2. Sticky Routing (Backend)

**Algorithme** :
```rust
if sticky_key.is_some() {
    let hash = stable_hash_u64(sticky_key);
    let idx = hash % workers.len();
    workers[idx]  // Sélectionne le même worker
} else {
    workers[0]  // Load balancing normal
}
```

**Avantages** :
- Maintient l'affinité avec un worker spécifique
- Réutilise le KV cache du worker (si supporté)
- Réduit la latence pour les requêtes suivantes

**Limitations** :
- Best-effort : si le worker devient indisponible, un autre est sélectionné
- Pas de garantie stricte (hash peut changer si workers changent)

## Tests et Validation

### 1. Tests Unitaires

**À créer** :
- `test_resolve_openai_model_id()` : Vérifier résolution UUID/HF/offering
- `test_select_ready_worker_for_model()` : Vérifier sélection worker
- `test_sticky_routing()` : Vérifier affinité session
- `test_token_extraction()` : Vérifier extraction tokens (streaming/non-streaming)

### 2. Tests d'Intégration

**Scénarios** :
1. **Session simple** :
   - Créer une session de chat
   - Envoyer plusieurs messages avec le même `X-Inventiv-Session`
   - Vérifier que toutes les requêtes vont vers le même worker

2. **Load balancing** :
   - Créer plusieurs instances ready pour le même modèle
   - Envoyer des requêtes sans `X-Inventiv-Session`
   - Vérifier la distribution équitable

3. **Failover** :
   - Créer une session avec sticky routing
   - Arrêter le worker utilisé
   - Vérifier qu'une nouvelle requête sélectionne un autre worker

4. **Token tracking** :
   - Envoyer une requête d'inférence
   - Vérifier que les tokens sont extraits et stockés
   - Vérifier les métriques mises à jour

### 3. Tests E2E

**Scripts existants** :
- `scripts/test_worker_observability_mock.sh` : Test complet avec mock
- `scripts/test_worker_observability_mock_multi.sh` : Test multi-instances

**À étendre** :
- Ajouter tests de sticky routing
- Ajouter tests de résolution modèle (UUID/HF/offering)
- Ajouter tests de token extraction

## Points d'Attention

### 1. Staleness des Workers

**Problème** : Un worker peut devenir indisponible sans que le système le détecte immédiatement.

**Solution** : 
- Utilise `OPENAI_WORKER_STALE_SECONDS` (défaut: 300s)
- Vérifie `worker_last_heartbeat` ou `last_health_check`
- Exclut les workers trop anciens de la sélection

### 2. Sticky Routing et Disponibilité

**Problème** : Si le worker sticky devient indisponible, la session peut échouer.

**Solution** :
- Le système sélectionne automatiquement un autre worker
- Le client peut retry la requête
- Pas de garantie stricte d'affinité (best-effort)

### 3. Extraction des Tokens

**Problème** : Les tokens peuvent ne pas être présents dans toutes les réponses.

**Solution** :
- Extraction best-effort (ne bloque pas la requête)
- Logs détaillés pour debugging
- Métriques mises à jour même sans tokens

## Améliorations Futures

### 1. Session Persistence

**Idée** : Stocker les sessions dans la DB pour persister entre refresh.

**Implémentation** :
- Table `chat_sessions` avec `session_id`, `user_id`, `model_id`, `messages`
- Endpoint `GET /chat/sessions` pour récupérer les sessions
- Endpoint `POST /chat/sessions/:id/messages` pour ajouter des messages

### 2. Context Window Management

**Idée** : Gérer automatiquement le contexte pour éviter de dépasser les limites.

**Implémentation** :
- Vérifier `context_length` du modèle
- Tronquer les messages anciens si nécessaire
- Avertir l'utilisateur si le contexte est tronqué

### 3. Multi-turn Conversation Tracking

**Idée** : Tracker les conversations multi-tours pour analytics.

**Implémentation** :
- Table `conversations` avec `session_id`, `user_id`, `model_id`
- Table `conversation_messages` avec les messages
- Dashboard pour visualiser les conversations

### 4. Rate Limiting par Session

**Idée** : Limiter le nombre de requêtes par session pour éviter l'abus.

**Implémentation** :
- Redis pour tracking des requêtes par session
- Limite configurable (ex: 100 req/min)
- Retourner `429 Too Many Requests` si dépassé

## Références

- **Architecture** : `docs/architecture.md`
- **Worker Routing** : `docs/worker_and_router_phase_0_2.md`
- **OpenAI Proxy** : `inventiv-api/src/openai_proxy.rs`
- **Worker Routing** : `inventiv-api/src/worker_routing.rs`
- **Frontend Chat** : `inventiv-frontend/src/app/(app)/chat/page.tsx`
- **Frontend Workbench** : `inventiv-frontend/src/app/(app)/workbench/page.tsx`

