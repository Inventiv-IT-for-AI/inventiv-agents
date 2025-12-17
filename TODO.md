# Roadmap & TODO (état repo + backlog)

Ce fichier reflète l’état **réel** du repo (code + migrations + UI) et la suite (priorisée).

---

## ✅ Réalisé (livré dans le code)

### Control-plane & provisioning
- **Provisioning Scaleway** (orchestrator): création VM + volume data, poweron, récupération IP, transitions d’état.
- **State machine + jobs**: provisioning/health-check/terminator/watch-dog + requeue.
- **Auto-install worker**: bootstrap via SSH avec phases `::phase::…`, logs enrichis dans `action_logs.metadata`.
- **Sizing stockage par modèle**: taille recommandée depuis la table `models` (fallbacks contrôlés).
- **HF token**: support `WORKER_HF_TOKEN_FILE` (secret file) + alias `HUGGINGFACE_TOKEN`.

### Modèles & readiness
- **Catalogue `models`**: champs `is_active`, `data_volume_gb`, metadata (seed enrichi).
- **Sélecteur de modèle obligatoire** côté UI + **enforcement API** (`model_id` requis pour créer une instance).
- **Readiness industrialisée**: actions `WORKER_VLLM_HTTP_OK`, `WORKER_MODEL_LOADED`, `WORKER_VLLM_WARMUP`.
- **Modes vLLM**: `mono` (1 vLLM) / `multi` (1 vLLM par GPU derrière HAProxy sticky).

### OpenAI-compatible API + API keys
- **OpenAI proxy** (inventiv-api): `/v1/models`, `/v1/chat/completions` (streaming), `/v1/completions`, `/v1/embeddings`.
- **API keys (client)**: CRUD + auth `Authorization: Bearer <key>` (séparé des tokens workers).
- **Live capacity**: `/v1/models` reflète les modèles réellement servis par des workers “fresh” (avec tolérance staleness).

### Runtime models dashboard + Workbench
- **Runtime models**: endpoint + page UI `/models` (instances, GPUs, VRAM, requests, failed).
- **Workbench**: page UI `/workbench` (base URL, snippets, test chat via API key).

### Temps réel (UI)
- **SSE**: `GET /events/stream` (topics instances/actions) + hook frontend `useRealtimeEvents` (refresh instances + action logs).
- **VirtualizedDataTable persistence**: préférences colonnes persistées pour la pop-in “Actions de l’instance”.

### Dev ergonomics
- **PORT_OFFSET** (worktrees) + UI-only exposée.
- **`make api-expose`**: proxy loopback pour tunnels (cloudflared) sans modifier `docker-compose.yml`.
- **DB/Redis stateful**: `make down` garde volumes, `make nuke` wipe.

---

## 🐛 Bugs connus / dettes techniques (à suivre)

- **SSE**: implémentation actuelle basée sur polling DB (efficace mais pas “event-sourced” → à améliorer via NOTIFY/LISTEN ou Redis streams).
- **Observabilité**: pas encore de stack métriques/traces end-to-end (Prometheus/Grafana/OTel).
- **FinOps**: coûts OK, mais pas encore de **comptage tokens in/out** (voir backlog).
- **Docs**: certains documents restent “vision” (router, bare-metal) vs “implémenté”.

---

## 🚧 À faire (backlog)

### Déploiement & DNS
- **Staging**: déploiement sur `studio-stg.inventiv-agents.fr` (routing API + edge + certs).
- **Production**: déploiement sur `studio-prd.inventiv-agents.fr`.

### UX / API
- **System Prompt configurable** (Inventiv-Agents): UI + API + persistence (par modèle / par tenant / par key).
- **Streaming**: améliorer streaming E2E (Workbench + proxy + UI) + UX (annulation, TTFT, tokens/sec).

### Observability / Monitoring
- **Metrics**: `/metrics` sur API/orchestrator/worker + dashboards.
- **Tracing**: OTel (optionnel au début) + corrélation `correlation_id`.
- **Monitoring infra**: GPU util, queue depth, vLLM health, erreurs, SLOs.

### FinOps “full features”
- **Comptage tokens in/out** par Worker / API_KEY / User / Tenant / Model.
- **Validation**: consolidation dashboards + exports + séries temporelles.

### Multi-tenant & sécurité
- **Tenants**: entité + isolation.
- **Users / access management**: passer du full-admin actuel à un modèle multi-rôles.
- **Droits par module** + **RLS** PostgreSQL (à concevoir).

### Data plane / perf
- **Optimisation load-balancing** (sticky, health scoring, failover, retry policy).
- **Auto scale-up / auto scale-down**.
- **Support other Cloud Providers** (AWS/GCP/etc).
- **Support on-prem / private / shared bare metal servers**.

---

## 🎯 Next steps (3–7 priorités)

1) **Deploy Staging + DNS** (`studio-stg.inventiv-agents.fr`) avec routing propre UI/API + certs  
2) **Streaming Workbench** (UX + robustesse)  
3) **Observability** (metrics + dashboards minimum viable)  
4) **FinOps tokens** (in/out) + agrégations par API_KEY/User/Model  
5) **Tenants + RBAC** (premier cut)  
6) **LB hardening** + signaux worker (queue depth / TTFT)  
7) **Autoscaling MVP** (politiques + cooldowns)
