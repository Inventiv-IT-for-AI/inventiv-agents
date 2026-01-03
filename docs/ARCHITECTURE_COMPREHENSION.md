# Compréhension de l'Architecture Inventiv-Agents

> **Note** : Ce document décrit la compréhension initiale de l'architecture. Pour les fonctionnalités récentes, voir :
> - [State Machine & Progress Tracking](STATE_MACHINE_AND_PROGRESS.md)
> - [Agent Version Management](AGENT_VERSION_MANAGEMENT.md)
> - [Storage Management](STORAGE_MANAGEMENT.md)

## Vue d'ensemble

**Inventiv-Agents** est une plateforme d'orchestration LLM (control-plane + data-plane) qui gère le cycle de vie complet des instances GPU pour l'inférence de modèles de langage.

### Stack technique

- **Backend (Rust)**: `inventiv-api` (API HTTP synchrone), `inventiv-orchestrator` (control-plane asynchrone), `inventiv-finops` (calculs de coûts)
- **Worker (Python)**: Agent sidecar déployé sur instances GPU, gère vLLM + heartbeats
- **Frontend (Next.js)**: Dashboard UI avec Tailwind + shadcn/ui
- **Infrastructure**: PostgreSQL (TimescaleDB) + Redis (Pub/Sub) + Docker Compose

---

## Carte Mentale des Flux

### 1. Flux de Provisioning (Instance Creation)

```
┌─────────────┐
│   Frontend  │
│  (Next.js)  │
└──────┬──────┘
       │ POST /deployments
       │ {model_id, zone, instance_type}
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-api (main.rs:create_deployment)              │
│  - Valide requête                                       │
│  - INSERT instances (status='provisioning')            │
│  - LOG: REQUEST_CREATE                                  │
│  - Publie CMD:PROVISION dans Redis                      │
└──────┬──────────────────────────────────────────────────┘
       │ Redis Pub/Sub: orchestrator_events
       │ {type: "CMD:PROVISION", instance_id, zone, ...}
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-orchestrator (main.rs:event_listener)        │
│  - Reçoit CMD:PROVISION                                 │
│  - Spawn services::process_provisioning()              │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  services::process_provisioning()                      │
│  - ProviderManager::get_provider(code)                  │
│  - provider.create_instance(zone, type, model)          │
│  - UPDATE instances SET provider_instance_id, ip        │
│  - UPDATE instances SET status='booting'                 │
│  - LOG: PROVIDER_CREATE_SUCCESS                         │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  job-health-check (health_check_job.rs)                 │
│  - Loop toutes les 10s                                  │
│  - SELECT instances WHERE status='booting'               │
│  - FOR UPDATE SKIP LOCKED (claim)                       │
│  - health_check_flow::check_and_transition_instance()    │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  health_check_flow::check_and_transition_instance()     │
│  - Vérifie SSH:22 OU /readyz (worker)                  │
│  - Vérifie /info (agent version/checksum)               │
│  - Priorise heartbeat récent (< 30s)                    │
│  - Si ready: UPDATE status='ready'                      │
│  - Si timeout: UPDATE status='startup_failed'           │
│  - Calcule progress_percent (0-100%)                    │
└─────────────────────────────────────────────────────────┘

> **Voir** : [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) pour les détails sur les health checks et le progress tracking.
```

### 2. Flux Worker Registration & Heartbeat

```
┌─────────────────────────────────────────────────────────┐
│  inventiv-worker (agent.py)                             │
│  - Démarre vLLM + agent HTTP server                     │
│  - Loop: register_worker_once() puis send_heartbeat()   │
└──────┬──────────────────────────────────────────────────┘
       │ POST /internal/worker/register
       │ {instance_id, model_id, vllm_port, ip_address}
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-api (main.rs:proxy_worker_register)           │
│  - Proxy vers orchestrator_internal_url()                │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-orchestrator (main.rs:worker_register)        │
│  - Vérifie auth (token ou bootstrap)                     │
│  - Si bootstrap: issue_worker_token() → retourne token  │
│  - UPDATE instances SET worker_status, worker_model_id   │
│  - Retourne bootstrap_token (si nouveau)                │
└─────────────────────────────────────────────────────────┘
       │
       │ POST /internal/worker/heartbeat (toutes les 4-10s)
       │ {instance_id, status, queue_depth, gpu_util, agent_info, ...}
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-orchestrator (main.rs:worker_heartbeat)        │
│  - Vérifie auth (Bearer token)                           │
│  - UPDATE instances SET worker_last_heartbeat, ...       │
│  - INSERT gpu_samples (métriques GPU par index)          │
│  - INSERT system_samples (CPU/Mem/Disk/Net)             │
└─────────────────────────────────────────────────────────┘
```

### 3. Flux OpenAI Proxy (Inference Requests)

```
┌─────────────┐
│   Client    │
│  (curl/UI)  │
└──────┬──────┘
       │ POST /v1/chat/completions
       │ Authorization: Bearer <api_key>
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-api (main.rs:openai_proxy_chat_completions)   │
│  - auth::require_user_or_api_key()                      │
│  - Sélectionne worker "ready" pour model_id             │
│  - openai_worker_stale_seconds_db() (tolérance stale)   │
│  - worker_routing::select_ready_worker()                 │
└──────┬──────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────┐
│  worker_routing::select_ready_worker()                   │
│  - SELECT instances WHERE:                               │
│    * status='ready'                                      │
│    * worker_status='ready'                               │
│    * worker_model_id = requested_model                  │
│    * worker_last_heartbeat > NOW() - stale_seconds       │
│  - Load balancing: least queue_depth                     │
└──────┬──────────────────────────────────────────────────┘
       │
       │ POST http://<worker_ip>:<vllm_port>/v1/chat/completions
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-worker (vLLM)                                │
│  - Traite requête d'inférence                           │
│  - Retourne streaming SSE ou JSON                       │
└──────┬──────────────────────────────────────────────────┘
       │
       │ Response (avec tokens usage)
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-api (extraction tokens)                       │
│  - Extrait prompt_tokens, completion_tokens              │
│  - INSERT instance_request_metrics                      │
│  - INSERT finops.inference_usage                        │
└─────────────────────────────────────────────────────────┘
```

### 4. Flux Background Jobs (Orchestrator)

```
┌─────────────────────────────────────────────────────────┐
│  inventiv-orchestrator (main.rs)                        │
│  - Démarre 4 jobs en tokio::spawn:                      │
└──────┬──────────────────────────────────────────────────┘
       │
       ├─► job-health-check (health_check_job.rs)
       │   - Loop 10s
       │   - Traite instances status='booting'
       │   - Transition → 'ready' ou 'startup_failed'
       │
       ├─► job-terminator (terminator_job.rs)
       │   - Loop 10s
       │   - Traite instances status='terminating'
       │   - Appelle provider.terminate_instance()
       │   - Transition → 'terminated'
       │
       ├─► job-watch-dog (watch_dog_job.rs)
       │   - Loop 10s
       │   - Traite instances status='ready'
       │   - Vérifie existence chez provider (orphan detection)
       │   - Marque deleted_by_provider=true si absent
       │
       └─► job-provisioning (provisioning_job.rs)
           - Loop 10s
           - Traite instances status='provisioning' (stuck)
           - Requeue CMD:PROVISION (Redis Pub/Sub non durable)
```

### 5. Flux Event Listener (Redis Subscriber)

```
┌─────────────────────────────────────────────────────────┐
│  inventiv-api (main.rs)                                 │
│  - Publie commandes dans Redis:                         │
│    * CMD:PROVISION (create_deployment)                  │
│    * CMD:TERMINATE (terminate_instance)                  │
│    * CMD:SYNC_CATALOG (manual_catalog_sync_trigger)     │
│    * CMD:RECONCILE (manual_reconcile_trigger)            │
└──────┬──────────────────────────────────────────────────┘
       │ Redis channel: orchestrator_events
       ▼
┌─────────────────────────────────────────────────────────┐
│  inventiv-orchestrator (main.rs:event_listener)         │
│  - Subscribe orchestrator_events                        │
│  - Match event_type:                                     │
│    * CMD:PROVISION → services::process_provisioning()    │
│    * CMD:TERMINATE → services::process_termination()     │
│    * CMD:REINSTALL → services::process_reinstall()       │
│    * CMD:SYNC_CATALOG → services::process_catalog_sync() │
│    * CMD:RECONCILE → services::process_full_reconciliation()
└─────────────────────────────────────────────────────────┘
```

### 6. Flux Scaling Engine (Future)

```
┌─────────────────────────────────────────────────────────┐
│  inventiv-orchestrator (main.rs:scaling_engine_loop)    │
│  - Loop 60s (actuellement placeholder)                  │
│  - Analyse signaux: queue_depth, GPU util, latence      │
│  - Décide scale-up/down                                 │
│  - Publie CMD:PROVISION ou CMD:TERMINATE                │
└─────────────────────────────────────────────────────────┘
```

---

## Points d'Extension Identifiés

### 1. **Provider Adapters** (`inventiv-providers`)
- Architecture modulaire via trait `CloudProvider`
- Implémentations: `mock.rs`, `scaleway.rs`
- Facilite l'ajout de nouveaux providers (AWS, GCP, bare-metal)

### 2. **Worker Flavors** (`inventiv-worker/flavors/`)
- Configurations par provider/environnement
- Permet adaptation runtime selon hardware

### 3. **FinOps Events** (`finops_events.rs`)
- Events: `EVT:INSTANCE_COST_START`, `EVT:INSTANCE_COST_STOP`
- Extension prévue: `EVT:TOKENS_CONSUMED`, `EVT:API_KEY_CREATED`

### 4. **Multi-tenant** (MVP implémenté)
- Tables: `organizations`, `organization_memberships`
- Pré-câblage: `organization_models`, `organization_model_shares`
- Roadmap: RBAC, isolation, chargeback tokens

---

## Observations & Incohérences

### ✅ Points Forts

1. **Séparation CQRS claire**: API (synchronisé) vs Orchestrator (asynchrone)
2. **Jobs robustes**: Utilisation de `FOR UPDATE SKIP LOCKED` pour éviter les conflits
3. **Idempotence**: Migrations et seeds idempotents (`ON CONFLICT`)
4. **Observabilité**: Métriques GPU/système stockées en time-series (TimescaleDB)
5. **Worker auth**: Bootstrap token + hash SHA-256 en DB

### ⚠️ Points d'Attention

1. **Redis Pub/Sub non durable**
   - **Impact**: Si orchestrator down pendant publish, événement perdu
   - **Mitigation**: `job-provisioning` requeue les instances "stuck"
   - **Recommandation**: Considérer Redis Streams (durable) ou DB queue

2. **SSE basé sur polling DB**
   - **État actuel**: `GET /events/stream` poll la DB (efficace mais pas event-sourced)
   - **Recommandation**: Migrer vers `NOTIFY/LISTEN` PostgreSQL ou Redis Streams

3. **Scaling Engine placeholder**
   - **État**: Loop 60s qui log seulement le count d'instances
   - **Recommandation**: Implémenter logique de scaling (queue_depth, GPU util, latence)

4. **Mock provider routing hack**
   - **Problème**: Test E2E override `instances.ip_address` vers `mock-vllm` container IP
   - **Recommandation**: Mécanisme propre (param `upstream_base_url` en DB ou résolution service name)

5. **main.rs volumineux**
   - **inventiv-api/src/main.rs**: 3907 lignes
   - **Recommandation**: Extraire handlers dans modules dédiés (déjà partiellement fait, continuer)

6. **Documentation Router**
   - **État**: Router prévu mais non présent (API expose déjà `/v1/*`)
   - **Recommandation**: Clarifier roadmap ou supprimer références obsolètes

### 🔧 Actions de Réalignement Proposées

1. **Court terme**:
   - Extraire handlers restants de `main.rs` vers modules (`handlers/`, `routes/`)
   - Documenter le "hack" mock routing et planifier solution propre
   - Implémenter scaling engine MVP (basique queue_depth threshold)

2. **Moyen terme**:
   - Migrer SSE vers `NOTIFY/LISTEN` PostgreSQL
   - Considérer Redis Streams pour événements durables
   - Compléter RBAC multi-tenant (isolation ressources par org)

3. **Long terme**:
   - Router service dédié (si nécessaire) ou documenter que API fait le routing
   - Stack observabilité complète (Prometheus/Grafana/OTel)
   - Support providers additionnels (AWS, GCP, bare-metal)

---

## Conventions & Patterns

### Naming
- **Jobs**: `*_job.rs` (health_check_job, terminator_job, watch_dog_job, provisioning_job)
- **Services**: `services.rs` (handlers pour CMD:*)
- **Events**: `CMD:*` (commands), `EVT:*` (domain events)

### State Machine (Instance Status)
```
provisioning → booting → ready → terminating → terminated
                ↓           ↓
         startup_failed  draining
```

### Database
- **Migrations**: `sqlx-migrations/` (timestamped, checksum validated)
- **Seeds**: `seeds/catalog_seeds.sql` (idempotent via `ON CONFLICT`)
- **Time-series**: Tables `gpu_samples`, `system_samples` (TimescaleDB)

### Secrets
- **Mount**: `SECRETS_DIR` → `/run/secrets` (not committed)
- **Admin**: `default_admin_password` (bootstrap au démarrage)
- **Worker tokens**: Hash SHA-256 en DB (`worker_auth_tokens`)

---

## Tooling & Déploiement

### Local Dev
- `make up`: Stack complet (docker-compose.yml)
- `make ui`: Frontend Next.js (Docker ou host)
- `make api-expose`: Expose API sur loopback (tunnels)

### Staging/Prod (Scaleway)
- `make stg-provision`: Provision VM
- `make stg-bootstrap`: Install Docker/Compose
- `make stg-create`: Deploy stack (nginx + lego)
- `make stg-cert`: Génère/renew certs wildcard (Let's Encrypt)

### Images
- **Tagging**: SHA (`ghcr.io/...:<sha>`), Version (`v0.4.5`), Latest
- **Promotion**: Par digest (immutable) vers `:staging` ou `:prod`

---

## Conclusion

Architecture solide avec séparation claire des responsabilités (CQRS), jobs robustes, et observabilité intégrée. Points d'amélioration identifiés (durabilité Redis, scaling engine, refactoring main.rs) sont documentés et planifiés dans le backlog.

Le code suit les principes de clean code (SRP, modules dédiés) et maintient une bonne maintenabilité malgré la croissance du projet.

