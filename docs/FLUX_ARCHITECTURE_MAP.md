# Carte Mentale des Flux - Inventiv Agents

**Date**: 2026-01-06  
**Objectif**: Visualiser les flux de données et les points d'extension du système

---

## 🗺️ Vue d'Ensemble des Flux

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           FRONTEND (Next.js)                             │
│  - Dashboard UI                                                         │
│  - SSE: GET /events/stream (instances, action_logs)                     │
│  - API calls: /api/backend/* → proxy → API                              │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │ HTTP (session JWT)
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        INVENTIV-API (:8003)                             │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │ Product Plane (Synchronous)                                     │  │
│  │                                                                   │  │
│  │ Endpoints:                                                        │  │
│  │  - Auth: /auth/login, /auth/logout, /auth/me                     │  │
│  │  - Instances: GET /instances, DELETE /instances/:id              │  │
│  │  - Deployments: POST /deployments                                │  │
│  │  - OpenAI Proxy: /v1/models, /v1/chat/completions                │  │
│  │  - Worker Internal: /internal/worker/register, /heartbeat       │  │
│  │                                                                   │  │
│  │ Actions:                                                          │  │
│  │  1. Valide requêtes métier                                       │  │
│  │  2. Lit/écrit PostgreSQL (state)                                 │  │
│  │  3. Publie CMD:* dans Redis (orchestrator_events)                │  │
│  │  4. Expose SSE pour temps-réel                                   │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    │ PostgreSQL            │ Redis Pub/Sub
                    │ (Cold State)         │ (Hot Events)
                    │                       │
                    ▼                       ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    INVENTIV-ORCHESTRATOR (:8001)                        │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │ Control Plane (Asynchronous)                                     │  │
│  │                                                                   │  │
│  │ Event Listener (Redis Subscriber):                               │  │
│  │   - CMD:PROVISION → services::process_provisioning              │  │
│  │   - CMD:TERMINATE → services::process_termination               │  │
│  │   - CMD:SYNC_CATALOG → services::process_catalog_sync           │  │
│  │   - CMD:RECONCILE → services::process_full_reconciliation       │  │
│  │                                                                   │  │
│  │ Background Jobs (tokio::spawn):                                  │  │
│  │   - job-health-check (10s): booting/installing/starting → ready │  │
│  │   - job-provisioning (10s): requeue stuck provisioning           │  │
│  │   - job-terminator (10s): terminating → terminated              │  │
│  │   - job-watch-dog (10s): orphan detection (ready instances)     │  │
│  │   - job-recovery (30s): recover stuck instances                   │  │
│  │   - scaling_engine_loop: autoscaling (future)                    │  │
│  │                                                                   │  │
│  │ Worker Endpoints:                                                 │  │
│  │   - POST /internal/worker/register (bootstrap token)            │  │
│  │   - POST /internal/worker/heartbeat (status, metrics)           │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    │ Provider API          │ Worker HTTP
                    │ (Scaleway/Mock)       │ (via API proxy)
                    │                       │
                    ▼                       ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    PROVIDERS (Cloud Infrastructure)                     │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │ Scaleway Provider                                                │  │
│  │   - create_instance()                                            │  │
│  │   - terminate_instance()                                        │  │
│  │   - check_instance_exists()                                     │  │
│  │   - list_attached_volumes()                                     │  │
│  │                                                                   │  │
│  │ Mock Provider                                                    │  │
│  │   - Docker Compose runtime management                            │  │
│  │   - Synthetic IP assignment                                      │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
                                │ VM/Container Provisioning
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    INVENTIV-WORKER (Agent Sidecar)                      │
│  ┌─────────────────────────────────────────────────────────────────┐  │
│  │ Python Agent (agent.py)                                        │  │
│  │                                                                   │  │
│  │ Endpoints:                                                        │  │
│  │   - GET /healthz (liveness)                                      │  │
│  │   - GET /readyz (readiness: vLLM ready)                          │  │
│  │   - GET /metrics (Prometheus: GPU, queue, system)                │  │
│  │   - GET /info (agent version, checksum)                          │  │
│  │   - GET /logs (structured event logs)                           │  │
│  │                                                                   │  │
│  │ Actions:                                                          │  │
│  │   1. Lance vLLM (OpenAI-compatible server)                       │  │
│  │   2. POST /internal/worker/register (bootstrap)                │  │
│  │   3. POST /internal/worker/heartbeat (periodic, 4-10s)          │  │
│  │   4. Logs événements structurés (/opt/inventiv-worker/...)       │  │
│  │                                                                   │  │
│  │ vLLM Server:                                                      │  │
│  │   - POST /v1/chat/completions                                    │  │
│  │   - POST /v1/completions                                         │  │
│  │   - GET /v1/models                                               │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 🔄 Flux Détaillés

### 1. Provisioning d'Instance

```
User (UI)
  │
  ├─► POST /deployments {instance_type_id, zone_id, model_id}
  │
  ▼
API (inventiv-api)
  │
  ├─► INSERT instances (status='provisioning')
  │
  ├─► PUBLISH Redis: CMD:PROVISION {instance_id, zone, instance_type}
  │
  └─► RETURN 200 Accepted
      │
      ▼
Orchestrator (Event Listener)
  │
  ├─► RECEIVE CMD:PROVISION
  │
  ├─► Spawn: services::process_provisioning()
  │
  ▼
services::process_provisioning()
  │
  ├─► Provider.create_instance()
  │   │
  │   ├─► Scaleway: API call → VM created
  │   └─► Mock: Docker Compose runtime
  │
  ├─► UPDATE instances SET provider_instance_id, status='booting'
  │
  ├─► Provider.start_instance() (poweron)
  │
  ├─► Provider.get_instance_ip()
  │
  ├─► UPDATE instances SET ip_address
  │
  └─► LOG action_logs: PROVIDER_CREATE, PROVIDER_START, PROVIDER_IP_ASSIGNED
      │
      ▼
job-health-check (10s loop)
  │
  ├─► SELECT instances WHERE status IN ('booting', 'installing', 'starting')
  │
  ├─► FOR EACH instance:
  │   │
  │   ├─► Check SSH (port 22) OR Worker /readyz
  │   │
  │   ├─► IF SSH accessible:
  │   │   ├─► Trigger SSH bootstrap (install worker)
  │   │   └─► UPDATE status='installing'
  │   │
  │   ├─► IF Worker /readyz OK:
  │   │   ├─► Check vLLM /v1/models
  │   │   └─► UPDATE status='ready'
  │   │
  │   └─► IF timeout (> 2h booting, > 30min installing):
  │       └─► UPDATE status='startup_failed'
      │
      ▼
Frontend (SSE)
  │
  └─► GET /events/stream → Real-time updates
```

### 2. Worker Registration & Heartbeat

```
Worker (agent.py)
  │
  ├─► Startup: Launch vLLM
  │
  ├─► POST /internal/worker/register
  │   {
  │     instance_id, worker_id, model_id,
  │     vllm_port, health_port, ip_address
  │   }
  │
  ▼
Orchestrator (worker_register handler)
  │
  ├─► IF no token exists:
  │   ├─► Generate token (wk_...)
  │   ├─► INSERT worker_auth_tokens (hash)
  │   └─► RETURN token (plaintext, only in response)
  │
  ├─► UPDATE instances SET worker_id, worker_vllm_port, ...
  │
  └─► RETURN 200 {token: "wk_..."}
      │
      ▼
Worker (agent.py)
  │
  ├─► Store token (memory + WORKER_AUTH_TOKEN_FILE)
  │
  ├─► Start heartbeat loop (every 4-10s)
  │
  └─► POST /internal/worker/heartbeat
      {
        instance_id, status, model_id,
        queue_depth, gpu_utilization,
        agent_info: {version, checksum}
      }
      │
      ▼
Orchestrator (worker_heartbeat handler)
  │
  ├─► Verify token (hash in DB)
  │
  ├─► UPDATE instances SET
  │     worker_last_heartbeat = NOW(),
  │     worker_status = status,
  │     worker_model_id = model_id,
  │     worker_queue_depth = queue_depth,
  │     worker_gpu_utilization = gpu_utilization,
  │     worker_metadata = {...agent_info}
  │
  └─► IF status='startup_failed' AND error_code='STARTUP_TIMEOUT':
      └─► RECOVER: UPDATE status='booting'
```

### 3. Termination d'Instance

```
User (UI)
  │
  ├─► DELETE /instances/:id
  │
  ▼
API (inventiv-api)
  │
  ├─► UPDATE instances SET status='terminating'
  │
  ├─► PUBLISH Redis: CMD:TERMINATE {instance_id}
  │
  └─► RETURN 200 Accepted
      │
      ▼
Orchestrator (Event Listener)
  │
  ├─► RECEIVE CMD:TERMINATE
  │
  ├─► Spawn: services::process_termination()
  │
  ▼
services::process_termination()
  │
  ├─► Provider.terminate_instance()
  │   │
  │   └─► Scaleway: API call → VM deletion started
  │
  └─► UPDATE instances SET last_reconciliation = NOW()
      │
      ▼
job-terminator (10s loop)
  │
  ├─► SELECT instances WHERE status='terminating'
  │
  ├─► FOR EACH instance:
  │   │
  │   ├─► Provider.check_instance_exists()
  │   │
  │   ├─► IF NOT EXISTS:
  │   │   ├─► Provider.list_attached_volumes()
  │   │   ├─► FOR EACH volume (delete_on_terminate=true):
  │   │   │   └─► Provider.delete_volume()
  │   │   ├─► UPDATE instance_volumes SET deleted_at
  │   │   └─► UPDATE instances SET status='terminated'
  │   │
  │   └─► IF EXISTS:
  │       └─► Provider.terminate_instance() (retry)
      │
      ▼
FinOps (finops_events)
  │
  └─► PUBLISH EVT:INSTANCE_COST_STOP
```

### 4. OpenAI Proxy (Inference)

```
Client
  │
  ├─► POST /v1/chat/completions
  │   Authorization: Bearer <api_key>
  │   {model: "meta-llama/...", messages: [...]}
  │
  ▼
API (inventiv-api, openai_proxy.rs)
  │
  ├─► Verify API key (or session)
  │
  ├─► SELECT instances WHERE
  │     status='ready' AND
  │     worker_model_id = model AND
  │     worker_last_heartbeat > NOW() - INTERVAL '5 minutes'
  │
  ├─► Load balancing: least queue_depth
  │
  ├─► POST http://{instance.ip}:{worker_vllm_port}/v1/chat/completions
  │
  ├─► Extract tokens from response (prompt_tokens, completion_tokens)
  │
  ├─► INSERT instance_request_metrics
  │
  ├─► INSERT finops.inference_usage
  │
  └─► RETURN response (streaming or JSON)
```

### 5. Watchdog & Orphan Detection

```
job-watch-dog (10s loop)
  │
  ├─► SELECT instances WHERE status='ready'
  │
  ├─► FOR EACH instance:
  │   │
  │   ├─► Provider.check_instance_exists()
  │   │
  │   ├─► IF NOT EXISTS:
  │   │   ├─► UPDATE instances SET status='provider_deleted'
  │   │   └─► PUBLISH EVT:INSTANCE_COST_STOP
  │   │
  │   ├─► IF EXISTS:
  │   │   ├─► Provider.list_attached_volumes()
  │   │   ├─► INSERT/UPDATE instance_volumes (discovery)
  │   │   └─► IF worker_model_id IS NULL:
  │   │       └─► Check vLLM /v1/models → UPDATE worker_model_id
```

---

## 🔌 Points d'Extension

### 1. Nouveaux Providers

**Fichier**: `inventiv-providers/src/{provider}.rs`

**Trait**: `CloudProvider`
```rust
pub trait CloudProvider {
    async fn create_instance(...) -> Result<String>;
    async fn terminate_instance(...) -> Result<bool>;
    async fn check_instance_exists(...) -> Result<bool>;
    async fn get_instance_ip(...) -> Result<Option<String>>;
    async fn list_attached_volumes(...) -> Result<Vec<AttachedVolume>>;
    async fn delete_volume(...) -> Result<bool>;
}
```

**Registration**: `provider_manager.rs` → `ProviderManager::get_provider()`

### 2. Nouveaux Jobs Background

**Pattern**:
1. Créer `{job_name}_job.rs` dans `inventiv-orchestrator/src/`
2. Fonction `pub async fn run(pool, redis_client)`
3. Loop avec `tokio::time::interval()`
4. Utiliser `FOR UPDATE SKIP LOCKED` pour éviter conflits
5. Spawn dans `main.rs`: `tokio::spawn(async move { job::run(...).await })`

**Exemple**: `job-worker-watchdog.rs` (à créer)

### 3. Nouveaux Événements Redis

**Channel**: `orchestrator_events` ou `finops_events`

**Format**:
```json
{
  "type": "CMD:NEW_COMMAND",
  "instance_id": "...",
  "correlation_id": "...",
  "payload": {...}
}
```

**Handler**: Ajouter dans `main.rs` → Event Listener → `match event_type`

### 4. Nouveaux Endpoints API

**Fichier**: `inventiv-api/src/main.rs` ou module dédié

**Pattern**:
1. Route dans `Router::new().route(...)`
2. Handler async avec `State(AppState)`
3. Auth middleware si nécessaire
4. Swagger docs via `#[derive(OpenApi)]`

### 5. Nouveaux États de State Machine

**Fichier**: `inventiv-orchestrator/src/state_machine.rs`

**Pattern**:
1. Fonction `pub async fn {from}_to_{to}(...)`
2. UPDATE instances SET status = ...
3. INSERT instance_state_history
4. Log action_logs
5. Appeler depuis jobs/services

---

## 🔍 Points d'Attention

### Concurrence
- ✅ Utiliser `FOR UPDATE SKIP LOCKED` pour éviter conflits entre orchestrators multiples
- ✅ Utiliser `tokio::spawn` pour paralléliser le traitement
- ⚠️ Attention aux race conditions sur `worker_auth_tokens` (bootstrap)

### Fiabilité
- ✅ Redis Pub/Sub non durable → requeue via jobs
- ✅ Health checks avec retry et backoff
- ⚠️ Pas de circuit breaker pour providers (à ajouter)

### Performance
- ✅ Limite de 50 instances par cycle de job (éviter surcharge)
- ✅ Cache des résultats de health checks (< 30s)
- ⚠️ Pas de rate limiting sur endpoints worker (à considérer)

### Observabilité
- ✅ Logging structuré dans `action_logs`
- ✅ Worker event logging (`/logs` endpoint)
- ⚠️ Pas de métriques Prometheus pour jobs (à ajouter)
- ⚠️ `correlation_id` partiellement implémenté (à étendre)

---

## 📚 Références

- [Architecture](architecture.md)
- [Domain Design](domain_design.md)
- [State Machine & Progress](STATE_MACHINE_AND_PROGRESS.md)
- [Worker & Router Phase 0.2](worker_and_router_phase_0_2.md)
- [Worker Reliability Analysis](WORKER_RELIABILITY_ANALYSIS.md)

