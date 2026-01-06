# Résumé de Session - Chat et Inference avec Modèles Alloués

## Objectif de la Session

Comprendre et valider le fonctionnement des sessions de chat et du routage des requêtes d'inférence vers les modèles alloués dans Inventiv Agents.

## Architecture Comprise

### 1. Control-Plane / Data-Plane

**Control-Plane** :
- `inventiv-orchestrator` : Gestion du cycle de vie des instances (provisioning, health checks, termination)
- `inventiv-api` : API publique avec endpoints OpenAI-compatible
- PostgreSQL : État des instances, modèles, utilisateurs
- Redis : Bus d'événements (CMD:*, EVT:*)

**Data-Plane** :
- `inventiv-worker` : Agent Python déployé sur les instances GPU
- vLLM : Moteur d'inférence (OpenAI-compatible)
- Routage : Sélection intelligente des workers pour les requêtes

### 2. Flux de Communication

```
┌─────────────┐
│   Frontend  │ (Next.js :3000)
│  (UI/Chat)  │
└──────┬──────┘
       │ HTTP (session JWT ou API key)
       ▼
┌─────────────┐      ┌──────────────┐
│  inventiv-  │──────▶│    Redis     │ (Pub/Sub: CMD:*, EVT:*)
│    api      │      │  (Events)    │
│   (:8003)   │      └──────┬───────┘
└──────┬──────┘             │
       │                    │ Subscribe
       │ PostgreSQL          ▼
       │ (State)      ┌──────────────┐
       │              │  inventiv-   │
       └──────────────▶│ orchestrator │ (Control Plane :8001)
                       │  (Jobs/State)│
                       └──────┬───────┘
                              │
                              │ Provider API
                              ▼
                    ┌─────────────────┐
                    │ Scaleway / Mock  │
                    │  (Instances GPU) │
                    └─────────┬─────────┘
                              │
                              │ Worker Agent
                              ▼
                    ┌─────────────────┐
                    │ inventiv-worker │
                    │ (vLLM + Agent)   │
                    └─────────────────┘
```

### 3. Flux OpenAI Proxy (Inference)

```
┌─────────────┐
│   Client    │
│  (curl/UI)  │
└──────┬──────┘
       │ POST /v1/chat/completions
       │ Authorization: Bearer <api_key>
       │ X-Inventiv-Session: <session_id>
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-api (openai_proxy_chat_completions)         │
│  - auth::require_user_or_api_key()                     │
│  - resolve_openai_model_id()                           │
│  - select_ready_worker_for_model()                      │
│  - proxy_to_worker()                                    │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  worker_routing::select_ready_worker_for_model()       │
│  - SELECT instances WHERE:                              │
│    * status='ready'                                      │
│    * worker_status='ready'                              │
│    * worker_model_id = requested_model                 │
│    * worker_last_heartbeat > NOW() - stale_seconds      │
│  - Load balancing: least queue_depth                    │
│  - Sticky routing: hash(session_id) % workers.len()    │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  Proxy HTTP vers Worker                                 │
│  POST http://<instance_ip>:<worker_vllm_port>/v1/chat/completions
│  Headers: X-Inventiv-Session (forwarded)                │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-worker (vLLM)                                │
│  - Traite la requête                                    │
│  - Génère la réponse (streaming ou non)                │
│  - Retourne les tokens dans usage                       │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-api (extraction tokens)                     │
│  - Parse réponse (SSE ou JSON)                          │
│  - Extrait tokens (input/output/total)                  │
│  - Met à jour instance_request_metrics                  │
│  - Stocke dans finops.inference_usage                    │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────┐
│   Client    │
│  (Réponse)  │
└─────────────┘
```

## Points Clés Identifiés

### 1. Sessions de Chat

**Sticky Routing** :
- Header `X-Inventiv-Session` pour maintenir l'affinité
- Hash stable pour sélectionner le même worker
- Best-effort (pas de garantie stricte)

**Génération Session ID** :
- Frontend génère `rid = crypto.randomUUID()`
- Unique par session de chat
- Persiste pendant la durée de la session

### 2. Routage vers Modèles Alloués

**Résolution Modèle** :
- Support UUID, HF repo id, offering ID
- Vérifie d'abord UUID → HF repo id
- Ensuite modèle public (HF repo id)
- Enfin offering d'organisation

**Sélection Worker** :
- Critères : `status='ready'`, `worker_status='ready'`, `worker_model_id` correspondant
- Freshness : `worker_last_heartbeat` ou `last_health_check` récent
- Load balancing : `queue_depth` ASC, freshness DESC

### 3. Tracking et Métriques

**Métriques par Instance** :
- `instance_request_metrics` : Compteurs de requêtes et tokens
- Mis à jour après chaque requête

**Usage FinOps** :
- `finops.inference_usage` : Enregistrement détaillé de chaque requête
- Permet calcul des coûts et facturation

**Runtime Models** :
- `runtime_models` : Modèles réellement servis
- Mis à jour pour `/v1/models` et `/runtime/models`

## Documents Créés

### 1. `docs/CHAT_SESSIONS_AND_INFERENCE.md`
Documentation complète sur :
- Architecture des sessions de chat
- Routage vers modèles alloués
- Flux d'inférence
- Tracking et métriques
- Gestion des sessions
- Améliorations futures

### 2. `docs/TEST_PLAN_CHAT_SESSIONS.md`
Plan de tests et validation :
- Tests unitaires
- Tests d'intégration
- Tests E2E
- Tests de performance
- Scripts de test
- Checklist de validation

## Actions Recommandées

### 1. Tests Immédiats

**À faire maintenant** :
1. ✅ Créer les scripts de test dans `scripts/`
2. ✅ Exécuter les tests de base (session simple, load balancing)
3. ✅ Vérifier le tracking des tokens
4. ✅ Valider le sticky routing

**Commandes** :
```bash
# Créer les scripts
cp docs/TEST_PLAN_CHAT_SESSIONS.md scripts/test_chat_session.sh
chmod +x scripts/test_chat_session.sh

# Exécuter les tests
make up
make ui
./scripts/test_chat_session.sh
```

### 2. Tests Unitaires

**À implémenter** :
- `test_resolve_openai_model_id()` dans `inventiv-api/src/worker_routing.rs`
- `test_select_ready_worker_for_model()` dans `inventiv-api/src/worker_routing.rs`
- `test_token_extraction()` dans `inventiv-api/src/metrics.rs`

**Commande** :
```bash
cd inventiv-api
cargo test worker_routing::test_resolve_openai_model_id
```

### 3. Tests d'Intégration

**Scénarios à tester** :
1. Session simple avec plusieurs messages
2. Load balancing avec plusieurs workers
3. Failover quand un worker devient indisponible
4. Token tracking pour streaming et non-streaming

### 4. Améliorations Futures

**Priorité haute** :
- Session persistence (DB) pour maintenir le contexte entre refresh
- Context window management pour éviter dépassement
- Multi-turn conversation tracking pour analytics

**Priorité moyenne** :
- Rate limiting par session
- Amélioration du sticky routing (garantie stricte)
- Optimisation de la sélection des workers

## Carte Mentale des Flux

```
┌─────────────────────────────────────────────────────────────────┐
│                    INVENTIV AGENTS PLATFORM                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │         FRONTEND (Next.js)              │
        │  - Chat UI (/chat)                      │
        │  - Workbench (/workbench)               │
        │  - Génère session_id (rid)              │
        └──────────────┬──────────────────────────┘
                       │ HTTP (session JWT)
                       ▼
        ┌─────────────────────────────────────────┐
        │         INVENTIV-API (:8003)            │
        │  - Auth (session/API key)                │
        │  - OpenAI Proxy (/v1/*)                  │
        │  - Résolution modèle                    │
        │  - Sélection worker                     │
        │  - Proxy vers worker                     │
        │  - Extraction tokens                    │
        │  - Tracking métriques                   │
        └──────────────┬───────────────────────────┘
                       │
        ┌──────────────┴──────────────┐
        │                             │
        ▼                             ▼
┌───────────────┐          ┌──────────────────┐
│   PostgreSQL  │          │      Redis       │
│  (State DB)   │          │  (Event Bus)     │
│               │          │  - CMD:*          │
│ - instances   │          │  - EVT:*          │
│ - models      │          └─────────┬─────────┘
│ - users       │                    │
│ - metrics     │                    │ Subscribe
└───────────────┘                    ▼
                            ┌──────────────────┐
                            │ INVENTIV-         │
                            │ ORCHESTRATOR     │
                            │ (:8001)          │
                            │                  │
                            │ - Jobs           │
                            │ - State machine  │
                            │ - Health checks  │
                            └─────────┬────────┘
                                      │
                                      │ Provider API
                                      ▼
                            ┌──────────────────┐
                            │   PROVIDERS       │
                            │  - Scaleway       │
                            │  - Mock           │
                            └─────────┬─────────┘
                                      │
                                      │ Provision
                                      ▼
                            ┌──────────────────┐
                            │   INSTANCES      │
                            │  (GPU VMs)       │
                            └─────────┬─────────┘
                                      │
                                      │ Deploy
                                      ▼
                            ┌──────────────────┐
                            │ INVENTIV-WORKER  │
                            │  - Agent Python  │
                            │  - vLLM          │
                            │  - Heartbeat     │
                            │  - Metrics       │
                            └──────────────────┘
```

## Points d'Extension Identifiés

### 1. Routage
- **Actuel** : Sélection basique avec queue_depth
- **Extension** : Health scoring, failover automatique, retry policy

### 2. Sessions
- **Actuel** : Sticky routing best-effort
- **Extension** : Persistence DB, context management, multi-turn tracking

### 3. Métriques
- **Actuel** : Tracking basique (requêtes, tokens)
- **Extension** : Latence, throughput, qualité, SLOs

### 4. Scaling
- **Actuel** : Manuel (provisioning via UI/API)
- **Extension** : Auto-scaling basé sur queue_depth, latence, GPU util

## Conclusion

L'architecture des sessions de chat et du routage vers les modèles alloués est bien comprise. Le système utilise :

1. **Sticky routing** via `X-Inventiv-Session` pour maintenir l'affinité
2. **Résolution intelligente** des modèles (UUID/HF/offering)
3. **Sélection optimale** des workers (queue_depth, freshness)
4. **Tracking complet** des métriques et tokens

Les prochaines étapes sont :
1. Créer et exécuter les tests
2. Valider le fonctionnement end-to-end
3. Implémenter les améliorations identifiées

## Références

- **Architecture** : `docs/architecture.md`
- **Chat Sessions** : `docs/CHAT_SESSIONS_AND_INFERENCE.md`
- **Test Plan** : `docs/TEST_PLAN_CHAT_SESSIONS.md`
- **Worker Routing** : `docs/worker_and_router_phase_0_2.md`
- **Code API** : `inventiv-api/src/openai_proxy.rs`
- **Code Routing** : `inventiv-api/src/worker_routing.rs`

