# Compréhension Architecture Inventiv-Agents - Session Init

**Date**: 2026-01-XX  
**Objectif**: Comprendre l'infrastructure LLM (control-plane/data-plane) et les conventions pour préparer le déploiement v0.5.0 sur Scaleway (Staging/Prod).

---

## 1. Vue d'ensemble de l'architecture

### 1.1 Composants principaux

```
┌─────────────────────────────────────────────────────────────┐
│                    FRONTEND (Next.js)                        │
│  Port: 3000 (+ PORT_OFFSET)                                  │
│  - UI Dashboard (Tailwind v4 + shadcn/ui)                   │
│  - Routes: /instances, /observability, /workbench, /models  │
│  - SSE: /events/stream (instances + action_logs)            │
└───────────────────────┬─────────────────────────────────────┘
                        │ HTTP (same-origin /api/backend/*)
                        │ Session JWT (cookie)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              INVENTIV-API (Product Plane)                   │
│  Port: 8003 (internal Docker network)                       │
│  - Synchronous HTTP API                                      │
│  - Session auth (JWT cookie)                                 │
│  - Publie CMD:* dans Redis                                   │
│  - Proxy /internal/worker/* → orchestrator                 │
│  - OpenAI-compatible proxy (/v1/*)                          │
└───────────────┬───────────────────────┬─────────────────────┘
                │                       │
                │ PostgreSQL            │ Redis Pub/Sub
                │ (State)               │ (Events)
                ▼                       ▼
┌─────────────────────────────────────────────────────────────┐
│         INVENTIV-ORCHESTRATOR (Control Plane)               │
│  Port: 8001 (internal, non exposé publiquement)              │
│  - Asynchronous jobs + state machine                        │
│  - Écoute Redis (CMD:*)                                     │
│  - Gère providers (Scaleway/Mock)                           │
│  - Health checks, provisioning, termination                 │
└───────────────┬─────────────────────────────────────────────┘
                │ Provider API
                ▼
┌─────────────────────────────────────────────────────────────┐
│              SCALEWAY / MOCK PROVIDER                        │
│  - Création/suppression instances GPU                        │
│  - Gestion volumes, IPs                                     │
└───────────────┬─────────────────────────────────────────────┘
                │ Worker Agent (SSH bootstrap)
                ▼
┌─────────────────────────────────────────────────────────────┐
│              INVENTIV-WORKER (Agent Sidecar)                │
│  - Python agent (agent.py)                                   │
│  - vLLM (OpenAI-compatible server)                         │
│  - Endpoints: /healthz, /readyz, /metrics, /info, /logs    │
│  - Heartbeat → /internal/worker/heartbeat                  │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Séparation des responsabilités (CQRS)

- **API (Product Plane)**: 
  - Synchronous, Request/Response
  - Gère auth, business logic, validation
  - Publie des **commandes** (CMD:*) dans Redis
  - Lit directement la DB pour les queries

- **Orchestrator (Control Plane)**:
  - Asynchronous, Event-driven
  - Écoute Redis (CMD:*)
  - Exécute les opérations IaaS (provisioning, termination)
  - Met à jour l'état technique dans PostgreSQL
  - **N'expose aucun endpoint public**

---

## 2. Bus d'événements Redis

### 2.1 Channels

- **`orchestrator_events`**: Commandes CMD:* publiées par l'API
- **`finops_events`**: Événements EVT:* pour FinOps (coûts, tokens)

### 2.2 Commandes (CMD:*)

| Commande | Source | Handler | Description |
|----------|--------|---------|-------------|
| `CMD:PROVISION` | API (`POST /deployments`) | `services::process_provisioning` | Créer une instance |
| `CMD:TERMINATE` | API (`DELETE /instances/:id`) | `services::process_termination` | Supprimer une instance |
| `CMD:REINSTALL` | API (`POST /instances/:id/reinstall`) | `services::process_reinstall` | Réinstaller le worker |
| `CMD:SYNC_CATALOG` | API (`POST /catalog/sync`) | `services::process_catalog_sync` | Synchroniser le catalogue |
| `CMD:RECONCILE` | API (`POST /reconcile`) | `services::process_full_reconciliation` | Réconciliation manuelle |

**Garanties**: Redis Pub/Sub est **non-durable** → requeue si orchestrator down (via `provisioning_job`).

### 2.3 Événements FinOps (EVT:*)

- `EVT:INSTANCE_COST_START`: Instance démarrée (facturation commence)
- `EVT:INSTANCE_COST_STOP`: Instance arrêtée (facturation arrêtée)
- `EVT:TOKENS_CONSUMED`: Tokens consommés (futur)
- `EVT:API_KEY_CREATED/REVOKED`: Gestion des clés API (futur)

---

## 3. Jobs background (orchestrator)

### 3.1 Jobs périodiques (tokio::spawn)

| Job | Intervalle | Rôle | Module |
|-----|------------|------|--------|
| **job-health-check** | 10s | Transition `booting` → `ready` / `startup_failed` | `health_check_job::run` |
| **job-terminator** | 10s | Instances `terminating` → confirmation suppression → `terminated` | `terminator_job::run` |
| **job-watch-dog** | 10s | Instances `ready` → vérifie existence chez provider (orphan detection) | `watch_dog_job::run` |
| **job-provisioning** | 10s | Instances `provisioning` "stuck" → requeue (Redis non-durable) | `provisioning_job::run` |
| **job-recovery** | 10s | Récupération instances bloquées dans divers états | `recovery_job::run` |

**Pattern SKIP LOCKED**: Tous les jobs utilisent `FOR UPDATE SKIP LOCKED` pour permettre plusieurs orchestrators en parallèle.

### 3.2 Tâches background (non-*_job.rs)

- **Scaling Engine Loop**: `scaling_engine_loop(...)` (futur: autoscaling basé sur signaux)
- **Event Listener Redis**: Subscriber sur `orchestrator_events` qui spawn des handlers `services::*`

---

## 4. State Machine & Progress Tracking

### 4.1 États des instances

```
provisioning → booting → ready → terminating → terminated → archived
                ↓
         startup_failed
```

**Transitions explicites** dans `inventiv-orchestrator/src/state_machine.rs`:
- `booting_to_ready`: Health check réussi
- `booting_to_startup_failed`: Timeout ou erreur critique
- `terminating_to_terminated`: Suppression confirmée
- `mark_provider_deleted`: Orphan detection

**Historique**: Toutes les transitions sont enregistrées dans `instance_state_history`.

### 4.2 Progress Tracking (0-100%)

Calcul automatique dans `inventiv-api/src/progress.rs` basé sur les actions complétées:

- **provisioning (0-20%)**: Request created (5%), Provider create (20%)
- **booting (20-100%)**: 
  - Provider start (30%)
  - IP assigned (40%)
  - SSH install (50%)
  - vLLM HTTP (60%)
  - Model loaded (75%)
  - Warmup (90%)
  - Health check (95%)
  - Ready (100%)

---

## 5. Worker Agent

### 5.1 Endpoints

| Endpoint | Rôle | Auth |
|----------|------|------|
| `GET /healthz` | Liveness (toujours 200) | - |
| `GET /readyz` | Readiness (200 si vLLM ready, 503 sinon) | - |
| `GET /metrics` | Prometheus metrics (GPU, système, queue depth) | - |
| `GET /info` | Agent info (version, checksum SHA256, build date) | - |
| `GET /logs` | Structured event logs (JSON lines, diagnostics) | - |

### 5.2 Communication avec Control Plane

**Via API Gateway** (pas directement orchestrator):
- `POST /internal/worker/register`: Bootstrap (génère token si absent)
- `POST /internal/worker/heartbeat`: Heartbeat périodique (4s par défaut)

**Auth**: Token par instance (`Authorization: Bearer <token>`), hashé dans DB (`worker_auth_tokens`).

### 5.3 Version Management

- **Constantes**: `AGENT_VERSION`, `AGENT_BUILD_DATE` dans `agent.py`
- **Checksum**: SHA256 calculé automatiquement (`_get_agent_checksum()`)
- **Vérification**: Script SSH bootstrap vérifie le checksum si `WORKER_AGENT_SHA256` défini
- **Monitoring**: Heartbeats incluent `agent_info` (version/checksum)

---

## 6. Endpoints internes /internal/worker/*

### 6.1 Proxy Gateway (API → Orchestrator)

L'API expose `/internal/worker/*` et **proxy** vers l'orchestrator:

```rust
// inventiv-api/src/main.rs
.route("/internal/worker/register", post(proxy_worker_register))
.route("/internal/worker/heartbeat", post(proxy_worker_heartbeat))
```

**Fonctionnement**:
1. Worker appelle `CONTROL_PLANE_URL` (ex: `http://api:8003`)
2. API vérifie l'auth (token worker)
3. API proxy vers `ORCHESTRATOR_INTERNAL_URL` (ex: `http://orchestrator:8001`)
4. Orchestrator traite la requête

**Avantages**:
- Orchestrator non exposé publiquement
- `CONTROL_PLANE_URL` stable (API domain) en dev/staging/prod
- Centralisation de l'auth côté API

---

## 7. Base de données

### 7.1 Tables principales

| Table | Rôle |
|------|------|
| `instances` | État des instances GPU (status, IP, provider, zone, type) |
| `providers` | Catalogue providers (Scaleway, Mock) |
| `regions` / `zones` | Hiérarchie géographique |
| `instance_types` | Types d'instances (GPU count, VRAM, CPU, RAM, coût) |
| `instance_type_zones` | Associations zone ↔ instance type |
| `models` | Catalogue modèles LLM (model_id, VRAM requis, data_volume_gb) |
| `users` | Utilisateurs (username, email, password_hash, role) |
| `worker_auth_tokens` | Tokens workers (hash SHA256, par instance) |
| `action_logs` | Logs d'actions (provisioning, termination, sync, etc.) |
| `instance_state_history` | Historique transitions d'état |
| `instance_volumes` | Tracking volumes attachés (delete_on_terminate) |
| `finops.cost_*_minute` | TimescaleDB tables pour coûts (actual, forecast, cumulative) |

### 7.2 Migrations

- **SQLx Migrations**: `sqlx-migrations/` (exécutées automatiquement au boot)
- **Format**: `YYYYMMDDHHMMSS_description.sql`
- **Checksum**: Validé pour éviter modifications accidentelles

### 7.3 Seeds

- **Catalog seeds**: `seeds/catalog_seeds.sql` (providers, regions, zones, instance_types)
- **Auto-seed**: `AUTO_SEED_CATALOG=1` en dev
- **Idempotent**: Utilise `ON CONFLICT` pour réexécution sûre

---

## 8. Tooling (Makefile)

### 8.1 Images (build/push/promotion)

```bash
make images-build [IMAGE_TAG=<sha>]
make images-push  [IMAGE_TAG=<sha>]
make images-promote-stg|prod IMAGE_TAG=<sha|vX.Y.Z>  # Promotion par digest
make images-publish-stg|prod  # Build+push v$(VERSION) puis retag
```

**Tags immutables**:
- SHA: `ghcr.io/<org>/<service>:<sha>`
- Version: `ghcr.io/<org>/<service>:v0.4.8`
- Latest: `ghcr.io/<org>/<service>:latest`

### 8.2 Dev local

```bash
make up|down|ps|logs          # docker-compose.yml (hot reload)
make nuke                      # Wipe DB/Redis volumes
make ui                        # Start Next.js UI
make api-expose                # Expose API on loopback (tunnels)
```

### 8.3 Staging/Prod remote (Scaleway)

```bash
make stg-provision             # Provision VM
make stg-bootstrap              # Install docker/compose
make stg-secrets-sync          # Sync secrets to VM
make stg-create|start|stop    # Deploy stack
make stg-cert                   # Generate/renew SSL (lego)
```

---

## 9. Flux complets

### 9.1 Provisioning d'une instance

```
1. UI: POST /deployments (model_id, instance_type_id, zone_id)
   ↓
2. API: Validation → INSERT instances (status='provisioning')
   ↓
3. API: Publie CMD:PROVISION dans Redis (orchestrator_events)
   ↓
4. Orchestrator: Reçoit CMD:PROVISION → spawn services::process_provisioning
   ↓
5. Orchestrator: Appel provider (Scaleway/Mock) → create_server
   ↓
6. Orchestrator: UPDATE instances (provider_instance_id, status='booting')
   ↓
7. Orchestrator: SSH bootstrap → install worker agent
   ↓
8. Worker: Register → /internal/worker/register (via API proxy)
   ↓
9. Orchestrator: Génère token → UPDATE worker_auth_tokens
   ↓
10. Worker: Heartbeat périodique → /internal/worker/heartbeat
    ↓
11. job-health-check: Vérifie /readyz → transition booting → ready
    ↓
12. UI: SSE /events/stream → refresh instances table
```

### 9.2 Termination d'une instance

```
1. UI: DELETE /instances/:id
   ↓
2. API: UPDATE instances (status='terminating')
   ↓
3. API: Publie CMD:TERMINATE dans Redis
   ↓
4. Orchestrator: Reçoit CMD:TERMINATE → spawn services::process_termination
   ↓
5. Orchestrator: Découvre volumes attachés → list_attached_volumes
   ↓
6. Orchestrator: Supprime volumes (si delete_on_terminate=true)
   ↓
7. Orchestrator: Appel provider → delete_server
   ↓
8. Orchestrator: UPDATE instances (status='terminated')
   ↓
9. Orchestrator: Publie EVT:INSTANCE_COST_STOP (FinOps)
   ↓
10. UI: SSE → refresh instances table
```

### 9.3 OpenAI Proxy (requête d'inférence)

```
1. Client: POST /v1/chat/completions (Authorization: Bearer <api_key>)
   ↓
2. API: Auth (session JWT OU API key)
   ↓
3. API: Résout model_id → instances ready pour ce modèle
   ↓
4. API: Load balancing → sélectionne worker (least outstanding requests)
   ↓
5. API: Proxy → Worker (http://<instance_ip>:8000/v1/chat/completions)
   ↓
6. Worker: vLLM traite la requête → streaming SSE
   ↓
7. API: Extrait tokens (prompt_tokens, completion_tokens) → INSERT metrics
   ↓
8. API: Stream réponse → Client
```

---

## 10. Points d'extension

### 10.1 Providers

**Architecture modulaire** (`inventiv-providers` package):
- Trait `CloudProvider` (create_server, delete_server, list_instances, etc.)
- Implémentations:
  - `mock.rs`: Mock provider (Docker Compose runtime management)
  - `scaleway.rs`: Scaleway provider (real API integration)

**Ajout d'un provider**:
1. Implémenter `CloudProvider` trait
2. Ajouter feature flag dans `Cargo.toml`
3. Configurer dans `provider_manager.rs`

### 10.2 Jobs background

**Ajout d'un job**:
1. Créer `*_job.rs` dans `inventiv-orchestrator/src/`
2. Fonction `run(pool, redis_client)` avec loop + interval
3. Spawn dans `main.rs` avec `tokio::spawn`

### 10.3 Endpoints API

**Ajout d'un endpoint**:
1. Route dans `inventiv-api/src/main.rs`
2. Handler dans module dédié (ex: `mod my_feature`)
3. Auth: `middleware::from_fn_with_state(state.db.clone(), auth::require_user_session)`
4. Documentation: Ajouter à `api_docs.rs` (OpenAPI)

---

## 11. Remarques & Incohérences détectées

### 11.1 ✅ Points forts

1. **Architecture CQRS claire**: Séparation API/Orchestrator bien définie
2. **State machine explicite**: Transitions documentées et historisées
3. **Progress tracking**: Système de progression 0-100% bien pensé
4. **Provider abstraction**: Architecture modulaire pour ajouter des providers
5. **Worker auth**: Token par instance avec bootstrap sécurisé
6. **Tooling complet**: Makefile bien organisé pour dev/staging/prod

### 11.2 ⚠️ Points d'attention / Dettes techniques

1. **Redis Pub/Sub non-durable**: 
   - ✅ Mitigé par `provisioning_job` (requeue instances stuck)
   - 💡 À considérer: Redis Streams pour durabilité future

2. **SSE basé sur polling DB**:
   - Actuel: Polling DB toutes les N secondes
   - 💡 Amélioration: NOTIFY/LISTEN PostgreSQL ou Redis Streams

3. **Mock provider routing**:
   - Test E2E override `instances.ip_address` vers `mock-vllm` (hack local)
   - 💡 À remplacer: Mécanisme propre (IP routable mock ou param upstream_base_url)

4. **Router séparé non présent**:
   - Actuel: API expose `/v1/*` directement
   - Doc mentionne Router comme roadmap
   - 💡 Clarifier: Router = futur service séparé ou intégré dans API?

5. **Docker CLI version**:
   - Orchestrator utilise Docker CLI 27.4.0 (API 1.44+)
   - 💡 Documenter prérequis Docker dans docs

6. **Observabilité**:
   - Métriques système/GPU implémentées
   - 💡 Manquant: Stack Prometheus/Grafana/OTel end-to-end + alerting

### 11.3 🔍 Incohérences / Questions

1. **Version actuelle**:
   - README: `v0.4.8`
   - Objectif: `v0.5.0` pour déploiement Scaleway
   - 💡 Vérifier: État réel du repo vs version déclarée

2. **FinOps service**:
   - `inventiv-finops` existe mais statut "Experimental" dans README
   - 💡 Clarifier: Service actif ou en développement?

3. **Multi-tenant**:
   - Tables `organizations`, `organization_memberships` présentes
   - MVP implémenté selon TODO.md
   - 💡 Vérifier: Isolation complète par `organization_id` ou partielle?

4. **Agent version management**:
   - Tooling Makefile présent (`agent-checksum`, `agent-version-bump`)
   - CI/CD workflow présent
   - 💡 Vérifier: Intégration complète ou partielle?

---

## 12. Actions recommandées pour v0.5.0

### 12.1 Priorité haute (déploiement Scaleway)

1. **Tests E2E Scaleway**:
   - Valider provisioning complet (instance → ready)
   - Valider termination (volumes supprimés)
   - Valider health checks (SSH + /readyz)
   - Valider progress tracking (0-100%)

2. **Monitoring**:
   - Vérifier logs structurés (JSON)
   - Vérifier métriques worker (GPU, système)
   - Vérifier action_logs (provisioning, termination)

3. **Secrets management**:
   - Vérifier `stg-secrets-sync` / `prod-secrets-sync`
   - Vérifier montage secrets dans containers (`SECRETS_DIR`)

4. **Certificats SSL**:
   - Vérifier `stg-cert` / `prod-cert` (lego)
   - Vérifier export/import volumes (`deploy/certs/`)

### 12.2 Priorité moyenne (robustesse)

1. **Récupération d'erreurs**:
   - Valider `recovery_job` (instances stuck)
   - Valider `provisioning_job` (requeue)
   - Valider `watch_dog_job` (orphan detection)

2. **Worker auth**:
   - Valider bootstrap token flow
   - Valider rotation token (si implémenté)
   - Valider revocation token

3. **Storage management**:
   - Valider découverte volumes (boot + data)
   - Valider suppression volumes (termination)
   - Valider volumes persistants (`delete_on_terminate=false`)

### 12.3 Priorité basse (améliorations futures)

1. **Observabilité**:
   - Stack Prometheus/Grafana
   - Tracing OTel
   - Alerting (instances stuck, health check failures)

2. **Performance**:
   - Optimisation load balancing (sticky, health scoring)
   - Auto-scaling (scale-up/scale-down)
   - Queue management (rate limiting, backpressure)

---

## 13. Carte mentale des flux

```
┌─────────────────────────────────────────────────────────────────┐
│                         FLUX PRINCIPAUX                          │
└─────────────────────────────────────────────────────────────────┘

┌──────────┐
│   UI     │
└────┬─────┘
     │ HTTP (session JWT)
     ▼
┌──────────┐      ┌──────────┐      ┌──────────┐
│   API    │─────▶│  Redis   │─────▶│Orchestrator│
│          │      │  Pub/Sub │      │           │
└────┬──────┘      └──────────┘      └─────┬─────┘
     │                                        │
     │ PostgreSQL                            │ Provider API
     ▼                                        ▼
┌──────────┐                          ┌──────────┐
│   DB     │                          │ Scaleway│
└──────────┘                          └─────┬─────┘
                                           │ SSH
                                           ▼
                                    ┌──────────┐
                                    │  Worker  │
                                    │  Agent   │
                                    └─────┬─────┘
                                          │
                                          │ Heartbeat
                                          ▼
                                    ┌──────────┐
                                    │   API    │
                                    │ (proxy)  │
                                    └──────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    JOBS BACKGROUND (10s loop)                    │
└─────────────────────────────────────────────────────────────────┘

job-health-check:    booting → ready / startup_failed
job-terminator:     terminating → terminated
job-watch-dog:      ready → orphan detection
job-provisioning:   provisioning stuck → requeue
job-recovery:       stuck instances → recovery

┌─────────────────────────────────────────────────────────────────┐
│                    COMMANDES REDIS (CMD:*)                      │
└─────────────────────────────────────────────────────────────────┘

CMD:PROVISION   → services::process_provisioning
CMD:TERMINATE   → services::process_termination
CMD:REINSTALL   → services::process_reinstall
CMD:SYNC_CATALOG → services::process_catalog_sync
CMD:RECONCILE   → services::process_full_reconciliation

┌─────────────────────────────────────────────────────────────────┐
│                    ÉVÉNEMENTS FINOPS (EVT:*)                     │
└─────────────────────────────────────────────────────────────────┘

EVT:INSTANCE_COST_START → inventiv-finops (calcul coûts)
EVT:INSTANCE_COST_STOP  → inventiv-finops (arrêt facturation)
```

---

## 14. Conclusion

L'architecture Inventiv-Agents est **bien structurée** avec une séparation claire des responsabilités (CQRS), une state machine explicite, et un système de jobs background robuste.

**Points forts**:
- Architecture modulaire (providers, jobs)
- Tooling complet (Makefile, scripts)
- Documentation détaillée
- Système de progress tracking et versioning agent

**Points à améliorer**:
- Observabilité end-to-end (Prometheus/Grafana)
- Durabilité Redis (Streams vs Pub/Sub)
- Tests E2E Scaleway complets

**Prochaines étapes**:
1. Valider tests E2E Scaleway (provisioning → ready → termination)
2. Vérifier monitoring/logs en staging
3. Préparer déploiement prod (secrets, certs, DNS)

---

**Document généré automatiquement lors de la session init**  
**Pour mise à jour**: Voir `docs/architecture.md`, `docs/domain_design.md`, `docs/specification_generale.md`

