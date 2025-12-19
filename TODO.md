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
- **IADataTable persistence**: préférences colonnes persistées (tri/largeur/ordre/visibilité) pour les tables IA (dont la pop-in “Actions de l’instance”).

### UI / Design system (monorepo)
- **Packages internes**:
  - `inventiv-ui/ia-designsys` (primitives UI centralisées)
  - `inventiv-ui/ia-widgets` (widgets de plus haut niveau, préfixe `IA*`)
- **Tailwind v4 (CSS-first)**: ajout des `@source` vers les packages workspaces (`ia-widgets`, `ia-designsys`) pour éviter toute purge de classes.
- **IADataTable**: table virtualisée réutilisable (dans `ia-widgets`) + **resize via séparateurs dédiés** (5px) entre colonnes.
- **Ergonomie dev**: `make ui-down` et `make ui-local-down` (stop UI Docker / kill UI host).

### Dev ergonomics
- **PORT_OFFSET** (worktrees) + UI-only exposée.
- **`make api-expose`**: proxy loopback pour tunnels (cloudflared) sans modifier `docker-compose.yml`.
- **DB/Redis stateful**: `make down` garde volumes, `make nuke` wipe.

### Multi-tenant (MVP)
- **Organisations**: création + membership + sélection “organisation courante” (switcher UX).
- **Pré-câblage DB “model sharing + chargeback tokens”** (non-breaking): tables `organization_models` + `organization_model_shares` + extension `finops.inference_usage`.

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
- ✅ **Organisations (MVP)**: création + membership + sélection “organisation courante” (switcher UX).
- ✅ **Pré-câblage DB “model sharing + chargeback”** (non-breaking):
  - `organizations` + `organization_memberships` + `users.current_organization_id`
  - `organization_models` (offering publié par org)
  - `organization_model_shares` (contrats provider→consumer, `pricing` JSONB)
  - extension `finops.inference_usage` pour attribuer `provider_organization_id` / `consumer_organization_id` + `unit_price_eur_per_1k_tokens` + `charged_amount_eur`

📄 Doc: `docs/MULTI_TENANT_MODEL_SHARING_BILLING.md` (pricing v1 = **€/1k tokens**)
- **Tenants v1 (Org isolation)**:
  - Isoler les ressources “métier” par `organization_id` (au minimum: instances, workbench_runs, action_logs, api_keys).
  - Introduire une notion d’**org courante obligatoire** pour les endpoints métier (401/409 si non sélectionnée).
  - Clarifier RBAC org: `owner|admin|member` + policy par endpoint.
  - (Plus tard) **RLS PostgreSQL** une fois le modèle stabilisé.

📄 Roadmap cible: `docs/MULTI_TENANT_ROADMAP.md` (users first-class + org workspaces + community offerings + entitlements + billing tokens)

- **API keys org-owned (prévu)**:
  - Activer `api_keys.organization_id` (actuellement nullable) + migration data (si besoin).
  - Résolution “consumer org” via API key (prioritaire) ou session (org courante).

- **Partage de modèles inter-org (provider→consumer)**:
  - CRUD `organization_models` (publish/unpublish).
  - CRUD `organization_model_shares` (grant/pause/revoke + pricing JSONB).
  - Convention d’identifiant “virtual model”: `org_slug/model_code` (côté OpenAI proxy).
  - Clarifier `visibility`: `public | unlisted | private` (private = org-only; unlisted = non listé mais accessible si autorisé).
  - Ajouter “consumer org discovery prefs” (autoriser/masquer public/payant/payant-with-contract).

- **Chargeback tokens (v1)**:
  - Ingestion/persistence des events `finops.inference_usage` avec:
    - `consumer_organization_id`, `provider_organization_id`, `organization_model_id`
    - pricing v1: `eur_per_1k_tokens`, calcul `charged_amount_eur`
  - Exposer dashboards/exports “consommation par org / provider / consumer”.

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
