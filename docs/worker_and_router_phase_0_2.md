# Phase 0.2.0 — Workers + Router (Data Plane)

Ce document décrit le **MVP** visé pour rendre des instances GPU réellement utilisables pour l’inférence:
un **Worker** (vLLM + agent sidecar) + un **Router** (OpenAI-compatible) avec load balancing.

---

## 1) Worker (vLLM + Agent)

### Objectif
Sur chaque VM GPU provisionnée par l’orchestrator, un conteneur Worker doit:
- lancer **vLLM** en mode OpenAI server,
- exposer une surface **health/readiness** fiable,
- publier des **heartbeats + métriques** au control plane.

### Endpoints (MVP)
- `GET /healthz` (liveness): le process répond
- `GET /readyz` (readiness): modèle chargé + vLLM prêt (pas juste “port ouvert”)
- `GET /metrics` (Prometheus): GPU util, queue depth, req/s, ttft, p95, etc.
- vLLM OpenAI-compatible:
  - `POST /v1/chat/completions`
  - `POST /v1/completions`

### Variables d’environnement (proposition)
- **Identity**
  - `INSTANCE_ID` (uuid inventiv)
  - `WORKER_ID` (uuid worker/agent)
  - `PROVIDER_INSTANCE_ID` (uuid provider)
- **Control plane**
  - `INVENTIV_API_URL` (ex: `http://api:8003`)
  - `INVENTIV_API_KEY` (clé d’agent, rotation possible)
- **Model runtime**
  - `MODEL_ID` (ex: `meta-llama/Llama-3.1-8B-Instruct`)
  - `TENSOR_PARALLEL_SIZE`
  - `GPU_MEMORY_UTILIZATION`

### Enrôlement (MVP)
1. Worker démarre vLLM + agent.
2. Une fois `readyz` OK, l’agent POST un `register`.
3. Heartbeat périodique (10s) avec:
   - status: `starting|ready|draining`
   - queue_depth
   - gpu_util
   - model_id

---

## 2) Router (Data Plane)

### Objectif
Fournir un point d’entrée **OpenAI-compatible** + un routage “intelligent”.

### Endpoints (MVP)
- `POST /v1/chat/completions`
- `POST /v1/completions`

### Responsabilités
- **Auth** API keys (cache Redis)
- **Routing** vers un worker “ready”
- **Load balancing**
  - priorité: `least_outstanding_requests` (LOR)
  - fallback: `lowest_queue_depth`
- **Failover**
  - retry sur autre worker si timeout/5xx
  - circuit-breaker simple

### Source de vérité
- Redis comme cache/coordination:
  - set workers ready par model_id
  - stats (queue_depth, req_inflight)

---

## 3) Orchestrator / API (impacts)

### Health check
Passer progressivement de “SSH:22” à:
- `readyz` du worker (meilleure définition de “ready”)

### Scaling (phase 0.2.x)
Définir un loop qui scale via:
- `queue_depth` global par model
- p95 latence / ttft
- GPU util


