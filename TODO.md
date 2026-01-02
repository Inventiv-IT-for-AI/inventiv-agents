# Roadmap & TODO (état repo + backlog)

Ce fichier reflète l’état **réel** du repo (code + migrations + UI) et la suite (priorisée).

---

## ✅ Réalisé (livré dans le code)

### Control-plane & provisioning
- **Provisioning Scaleway** (orchestrator): création VM + volume data, poweron, récupération IP, transitions d'état.
- **Provisioning Mock** (inventiv-providers): gestion automatique des runtimes Docker Compose, récupération IP, transitions d'état.
- **Architecture providers modulaire**: package `inventiv-providers` avec trait `CloudProvider`, séparation orchestrator/providers.
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
- **Observabilité**: pas encore de stack métriques/traces end-to-end (Prometheus/Grafana/OTel) + alerting.
- ✅ **FinOps**: coûts OK + **comptage tokens in/out** implémenté (voir section "FinOps full features").
- **Docs**: certains documents restent “vision” (router, bare-metal) vs “implémenté”.
- **Mock provider routing**: le test E2E OpenAI proxy override `instances.ip_address` vers `mock-vllm` (hack local). À remplacer par un mécanisme propre (voir backlog).
- **Docker CLI version**: orchestrator utilise Docker CLI 27.4.0 (compatible API 1.44+). À documenter les prérequis Docker dans la doc.

---

## 🚧 À faire (backlog)

### Déploiement & DNS
- **Staging**: déploiement sur `studio-stg.inventiv-agents.fr` (routing API + edge + certs).
- **Production**: déploiement sur `studio-prd.inventiv-agents.fr`.

### UX / API
- **System Prompt configurable** (Inventiv-Agents): UI + API + persistence (par modèle / par tenant / par key).
- **Streaming**: améliorer streaming E2E (Workbench + proxy + UI) + UX (annulation, TTFT, tokens/sec).

### Observability / Monitoring
- ✅ **Metrics**: `/metrics` sur API/orchestrator/worker + dashboards (CPU/Mem/Disk/Net + GPU per-index) + SLOs.
  - Implémenté: métriques système (CPU/Mem/Disk/Net) et GPU dans dashboard Observability
  - Implémenté: métriques requêtes et tokens par instance (`GET /instances/:instance_id/metrics`)
- **Tracing**: OTel (optionnel au début) + corrélation `correlation_id` (API ↔ orchestrator ↔ worker ↔ upstream).
  - Partiellement: `correlation_id` ajouté dans logs API, à étendre aux autres services
- **Monitoring infra**: GPU util, queue depth, vLLM health, erreurs, saturation, qualité du load-balancing.
- **E2E test chain (mock)**: étendre le test pour valider aussi le routing OpenAI sans hack DB (voir item "mock provider routing").

### Mock provider / tests
- ✅ **Gestion automatique des runtimes Mock**: création/suppression via Docker Compose dans `inventiv-providers/src/mock.rs`.
- ✅ **Scripts de synchronisation**: `mock_runtime_sync.sh` pour synchroniser les runtimes avec les instances actives.
- ✅ **Tests E2E multi-instances**: `test_worker_observability_mock_multi.sh` pour valider le provisionnement en série et parallèle.
- ✅ **Docker CLI/Compose dans orchestrator**: Docker CLI 27.4.0 + Docker Compose plugin v2.27.1 installés dans `Dockerfile.rust`.
- ✅ **Réseau Docker explicite**: `CONTROLPLANE_NETWORK_NAME` configuré dans `docker-compose.yml` pour éviter les erreurs de réseau.
- **Routage OpenAI proxy en mock**: rendre l'upstream joignable sans muter `instances.ip_address` (options: IP routable mock, ou param "upstream_base_url" par instance en DB, ou résolution "service name" côté API quand provider=mock).
- **Tests contractuels**: ajouter des tests (Rust) des payloads `register/heartbeat` (schema/validation) + compat rétro (old heartbeat payload sans `system_samples`).
- **Documentation Mock provider**: créer `docs/providers.md` avec architecture et guide d'utilisation.

### FinOps "full features"
- ✅ **Comptage tokens in/out** par Worker / API_KEY / User / Tenant / Model.
  - Implémenté: extraction tokens depuis réponses streaming/non-streaming, stockage dans `instance_request_metrics` et `finops.inference_usage`
  - Endpoint: `GET /instances/:instance_id/metrics`
  - Dashboard: métriques affichées dans Observability (`/observability`)
- **Validation**: consolidation dashboards + exports + séries temporelles.

### Secrets & credentials
- **AUTO_SEED_PROVIDER_CREDENTIALS**: documenter clairement le modèle “secrets in /run/secrets → provider_settings chiffré pgcrypto” + rotation/rollback + conventions de clés (`SCALEWAY_PROJECT_ID`, `SCALEWAY_SECRET_KEY_ENC`) + menace (logs/backup).

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
  - Clarifier RBAC org: `owner|admin|manager|user` + policy par endpoint.
  - Règles RBAC:
    - Invitations: Owner/Admin/Manager
    - Dernier Owner non révocable
    - Audit logs immuables (pas de delete)
  - “Double activation”:
    - Admin active techniquement (providers/regions/zones/types/models/api_keys/users/plan)
    - Manager active économiquement (providers/regions/zones/types/models/api_keys/users/plan)
    - Opérationnel uniquement si les 2 activations sont OK (par ressource)
    - UX: afficher un état “non opérationnel” + alerte indiquant le flag manquant (tech/eco)
  - (Plus tard) **RLS PostgreSQL** une fois le modèle stabilisé.
  - UX anti-erreur: **couleur de sidebar configurable par organisation** (visuel “scope changed”).

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

---

## 🚀 Plan d’implémentation (step-by-step, testable) — RBAC + scoping org

### Phase 1 — RBAC foundation (backend + tests) → commit
- **DB (migrations)**:
  - Normaliser `organization_memberships.role` sur: `owner|admin|manager|user`
  - Backfill: `member` → `user` (si présent)
  - Contrainte `CHECK` + `DEFAULT 'user'`
- **Backend (Rust)**:
  - Module RBAC (enum + helpers): rôle org, règles d’assignation (Owner/Admin/Manager), double activation (tech/eco)
  - Tests unitaires sur la matrice RBAC (sans DB)
- **Tests**:
  - `cargo check -p inventiv-api`
  - `cargo test -p inventiv-api`

### Phase 2 — Roles associés aux users (membership lifecycle) + tests → commit
- **API (org-scopé)**:
  - `GET /organizations/members`
  - `PUT /organizations/members/:user_id/role` (règles: Owner tout; Manager ↔ User; Admin ↔ User)
  - `DELETE /organizations/members/:user_id` + invariant “dernier Owner non révocable”
- **Audit logs**: loguer role changes et removals (immutables)
- **Tests**: dernier owner, escalations interdites, etc.

### Phase 3 — Invitations + Users management + tests → commit
- **DB**: `organization_invitations` (email, token, expiry, role, invited_by, accepted_at)
- **API**:
  - `POST /organizations/invitations`
  - `GET /organizations/invitations`
  - `POST /organizations/invitations/:token/accept` (user existant ou création)
- **UI**: inviter, voir pending, accepter (flow)

### Phase 4 — Settings org-scopés + double activation + tests → commit(s)
- Providers/regions/zones/types/models/settings scoppés org
- Double activation **par ressource**:
  - Admin = tech only, Manager = eco only, Owner = both
  - UI: état “non opérationnel” + alerte flag manquant

### Phase 5 — Instances org-scopées + RBAC + tests → commit(s)
- Admin/Owner: ops (provision/terminate/reinstall/scheduling/scaling)
- Manager: finance gating + dashboards
- User: usage / lecture selon politique

### Phase 6 — Models/Offerings + RBAC + tests → commit(s)
- Admin: config technique + publication
- Manager: pricing + activation économique + partage
- Owner: tout
