# Phase 0.2.x — Worker + Router (Data Plane)

Ce document décrit le **plan 0.2.x** pour rendre des instances GPU réellement utilisables pour l’inférence:
un **Worker** (vLLM + agent sidecar) puis un **Router** (OpenAI-compatible) avec load balancing.

Découpage proposé:
- **0.2.1 — Worker ready** (priorité): readiness fiable + heartbeats/capacity + health-check HTTP côté orchestrator.
- **0.2.2 — Router MVP**: OpenAI-compatible + load balancing + failover.

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
  - `CONTROL_PLANE_URL` (recommandé: URL de l’API/Gateway, ex `https://api.<domain>` ou `http://api:8003` en local)
  - `WORKER_AUTH_TOKEN` (optionnel: si vide, bootstrap automatique au `register`)
  - `WORKER_AUTH_TOKEN_FILE` (optionnel: chemin fichier pour persister/recharger le token après bootstrap)
- **Model runtime**
  - `MODEL_ID` (ex: `meta-llama/Llama-3.1-8B-Instruct`)
  - `TENSOR_PARALLEL_SIZE`
  - `GPU_MEMORY_UTILIZATION`

### Enrôlement (MVP)
1. Worker démarre vLLM + agent.
2. L’agent POST `register` au control plane:
   - si `WORKER_AUTH_TOKEN` est vide et qu’aucun token n’existe encore en DB pour `instance_id`, l’orchestrator peut renvoyer un `bootstrap_token`.
   - l’agent conserve ensuite ce token (en mémoire et optionnellement via `WORKER_AUTH_TOKEN_FILE`).
3. Heartbeat périodique (10s) avec:
   - status: `starting|ready|draining`
   - queue_depth
   - gpu_util
   - model_id

#### Stockage & sécurité (MVP)
- Le token est stocké **hashé** dans la table `worker_auth_tokens` (clé = `instance_id`).
- En staging/prod, le worker passe par l’API/Gateway (proxy vers orchestrator) afin de ne pas exposer l’orchestrator publiquement.

### Déploiement multi-machines (Docker Compose)
Pour le scénario “simple” (quelques machines), Docker Compose est utilisé **par machine**.
Comme Compose ne gère pas l’overlay multi-host, on utilise un réseau privé type **Tailscale/WireGuard** afin que:
- le control-plane contacte les workers,
- et/ou que les workers envoient heartbeats/metrics au control-plane.

### Test local (sans GPU)
Un scénario local “worker-ready” existe:
- `mock-vllm` sert `GET /v1/models`
- `worker-agent` parle au control plane via `CONTROL_PLANE_URL=http://api:8003` (proxy → orchestrator)

```bash
bash scripts/dev_worker_local.sh
```

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


