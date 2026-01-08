# État des Lieux - Inventiv Agents (7 janvier 2026)

## 📊 Vue d'ensemble

**Version actuelle**: `0.4.9`  
**Branche principale**: `main`  
**Dernier commit**: `f835ab3` - "feat: réconciliation des volumes orphelins et historique complet dans l'UI"

---

## 🔄 Status Git

### Branches actives
- `main` (branche principale)
- `feat/finops-dashboard`
- `feat/finops-eur-dashboard`
- `full-i18n`
- `worker-fixes`
- `wip/batch-20251218-1406`

### Fichiers modifiés (non commités)
**14 fichiers modifiés** avec **651 insertions, 91 suppressions**:

#### Backend (Rust)
- `inventiv-api/src/auth.rs` (+325 lignes) - **Gestion sessions multi-org**
- `inventiv-api/src/auth_endpoints.rs` (+85 lignes) - **Endpoints auth enrichis**
- `inventiv-api/src/bootstrap_admin.rs` (+132 lignes) - **Bootstrap admin amélioré**
- `inventiv-api/src/organizations.rs` (modifications) - **Gestion organisations**
- `inventiv-api/Cargo.toml` (+1 dépendance)

#### Frontend
- `inventiv-frontend/src/components/account/AccountSection.tsx` (modifications)
- `inventiv-frontend/src/lib/api.ts` (+3 lignes)

#### Infrastructure
- `Dockerfile.rust.prod` (modifications)
- `scripts/deploy_remote.sh` (modifications)
- `env/*.env.example` (modifications)

#### Nouveaux fichiers (non trackés)
- `docs/CHAT_SESSIONS_AND_INFERENCE.md`
- `docs/FRONTEND_401_REDIRECT.md`
- `docs/SESSION_ARCHITECTURE_PROPOSAL.md`
- `docs/SESSION_AUTH_ANALYSIS.md`
- `docs/SESSION_SUMMARY_CHAT_INFERENCE.md`
- `docs/TEST_PLAN_CHAT_SESSIONS.md`
- `inventiv-frontend/src/lib/api-client.ts`
- `sqlx-migrations/20251220131000_provider_credentials_settings.sql`
- `sqlx-migrations/20260107000000_create_user_sessions.sql`
- `sqlx-migrations/20260107000001_migrate_existing_sessions.sql`
- `sqlx-migrations/20260107000002_remove_current_org_from_users.sql`

### Commits récents (15 derniers)
1. `f835ab3` - feat: réconciliation des volumes orphelins et historique complet dans l'UI
2. `62e1885` - fix(orchestrator,api): add intermediate states and fix progress tracking
3. `1398db9` - fix: Utiliser vLLM v0.3.3 pour RENDER-S (P100 compatible)
4. `5dcc5dc` - fix: Correction compilation orchestrator + clarification versions vLLM
5. `f1ffc99` - feat: Résolution d'image vLLM par type d'instance/GPU
6. `508c460` - fix: Exclure RENDER-S de la vérification diskless boot
7. `252b825` - fix: Gestion complète des volumes Local Storage pour RENDER-S
8. `50aec85` - feat(worker): add structured event logging and diagnostic improvements
9. `e3a807f` - feat: add state machine, progress tracking, agent version management, and storage management
10. `91f7de4` - feat(api): add instance-level request and token metrics
11. `dfd33c7` - fix: remove manual TimescaleDB triggers (created automatically) and fix search_path
12. `0744d90` - fix: wrap TimescaleDB functions in DO blocks to handle errors gracefully
13. `08eacfd` - fix: use IF NOT EXISTS and correct schema for _sqlx_migrations table
14. `cc9941b` - fix: correct ON CONFLICT DO UPDATE syntax in touch_runtime_model_from_instance function
15. `9fefb63` - fix: remove empty CREATE VIEW statements causing syntax errors

---

## 🏗️ Architecture

### Composants principaux

#### 1. **Inventiv API** (Product Plane - Synchronous)
- **Port**: 8003
- **Rôle**: Interface HTTP transactionnelle
- **Responsabilités**:
  - API publique (hors inférence)
  - Authentification (sessions multi-org)
  - Billing / FinOps
  - Contrôle d'accès
  - SSE pour temps-réel (`GET /events/stream`)

#### 2. **Inventiv Orchestrator** (Control Plane - Asynchronous)
- **Port**: 8001 (interne)
- **Rôle**: Moteur d'exécution et surveillance
- **Responsabilités**:
  - Tâches asynchrones (provisioning, termination, health checks)
  - Jobs de fond:
    - `job-health-check` (booting → ready/startup_failed)
    - `job-terminator` (terminating → terminated)
    - `job-watch-dog` (orphan detection)
    - `job-provisioning` (requeue stuck instances)
  - Scaling engine loop
  - Event listener Redis (CMD:PROVISION|TERMINATE|SYNC_CATALOG|RECONCILE)

#### 3. **Inventiv FinOps**
- **Port**: 8005
- **Rôle**: Calculs de coûts et métriques financières
- **Fonctionnalités**: Coûts réels/forecast/cumulatifs en EUR

#### 4. **Inventiv Frontend** (Next.js)
- **Port**: 3000
- **Stack**: Next.js App Router + Tailwind v4 + shadcn/ui
- **Packages monorepo**:
  - `ia-designsys` (primitives UI)
  - `ia-widgets` (widgets réutilisables)

#### 5. **Inventiv Worker** (Agent Sidecar)
- **Déployé sur**: Instances GPU (Scaleway)
- **Rôle**: Pilote vLLM localement
- **Endpoints**: `/healthz`, `/readyz`, `/info`, `/logs`
- **Auth**: Token par instance (`worker_auth_tokens`)

### Communication & Flux

1. **Backend → Orchestrator**:
   - State (Cold): Écriture DB (`instances.status='provisioning'`)
   - Event (Hot): Redis Pub/Sub (`CMD:PROVISION_INSTANCE`)

2. **Orchestrator → Backend**:
   - Mise à jour DB (`Booting` → `Ready`)
   - SSE (`GET /events/stream`) pour UI temps-réel

3. **Worker → Control Plane**:
   - `POST /internal/worker/register`
   - `POST /internal/worker/heartbeat`
   - Via API Gateway (pas d'exposition directe orchestrator)

---

## 🗄️ Data Model

### Migrations récentes (non appliquées)

#### `20260107000000_create_user_sessions.sql`
**Nouvelle table**: `user_sessions`
- Support multi-sessions par utilisateur
- Contexte organisation (`current_organization_id`, `organization_role`)
- Sécurité: `session_token_hash` (SHA256) pour révocation
- Lifecycle: `created_at`, `last_used_at`, `expires_at`, `revoked_at`

#### `20260107000001_migrate_existing_sessions.sql`
Migration des sessions existantes vers `user_sessions`

#### `20260107000002_remove_current_org_from_users.sql`
Suppression de `users.current_organization_id` (remplacé par `user_sessions`)

#### `20251220131000_provider_credentials_settings.sql`
**Settings provider-scoped**:
- `SCALEWAY_PROJECT_ID` (text)
- `SCALEWAY_SECRET_KEY_ENC` (text, base64+pgp_sym_encrypt)
- `SCALEWAY_SECRET_KEY` (text, legacy)

### Migrations appliquées (récentes)

- `20260106010000_add_volume_reconciliation_timestamp.sql`
- `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql`
- `20260105200000_add_installing_starting_status.sql`
- `20260103200000_vllm_image_per_instance_type.sql`
- `20260105180000_update_vllm_image_to_v013.sql`
- `20260103170000_instance_volumes_unique_constraint.sql`
- `20260102000000_instance_request_metrics.sql`

### Tables principales

- `instances` (provisioning, status, IPs, volumes)
- `instances_state_history` (historique transitions)
- `instance_volumes` (tracking volumes attachés)
- `instance_request_metrics` (requêtes, tokens in/out)
- `organizations` + `organization_memberships` (multi-tenant)
- `user_sessions` (sessions multi-org) - **nouveau**
- `worker_auth_tokens` (auth par instance)
- `provider_settings` (credentials chiffrés)

---

## 🛠️ Tooling (Makefile)

### Commandes principales

#### CI locale
```bash
make ci-fast          # fmt-check + clippy + test + ui-lint + ui-build
make ci               # ci-fast + security-check + agent-version-check
make security-check   # Vérifie absence de clés privées dans fichiers trackés
```

#### Images Docker
```bash
make images-build [IMAGE_TAG=<sha>]
make images-push [IMAGE_TAG=<sha>]
make images-promote-stg IMAGE_TAG=<sha|vX.Y.Z>
make images-promote-prod IMAGE_TAG=<sha|vX.Y.Z>
```

#### Dev local
```bash
make up | down | ps | logs
make ui                # UI Docker sur http://localhost:3000
make dev-create        # Stack complète (hot reload)
make nuke              # Wipe DB/Redis volumes
```

#### Staging remote (Scaleway)
```bash
make stg-provision     # Créer/réutiliser VM + attach flex IP
make stg-bootstrap     # Install docker/compose + prepare dirs
make stg-secrets-sync  # Upload secrets to SECRETS_DIR
make stg-create        # Deploy complet (rsync + pull + cert + up)
make stg-update        # Pull + renew cert + up -d
make stg-status | stg-logs
```

#### Production remote (Scaleway)
```bash
make prod-provision
make prod-bootstrap
make prod-secrets-sync
make prod-create
make prod-update
make prod-status | prod-logs
```

#### Agent version management
```bash
make agent-checksum              # Calcul SHA256 de agent.py
make agent-version-get           # Affiche version actuelle
make agent-version-bump [VERSION=1.0.1]
make agent-version-check         # Vérifie version mise à jour si agent.py changé
```

### Variables importantes

- `IMAGE_REPO`: `ghcr.io/inventiv-it-for-ai/inventiv-agents` (défaut)
- `IMAGE_TAG`: `<sha12>` (défaut) ou `v<version>` ou `latest`
- `PORT_OFFSET`: Offset pour worktrees multiples (défaut: 0)
- `ORCHESTRATOR_FEATURES`: `provider-scaleway,provider-mock` (défaut)
- `REMOTE_DIR`: `/opt/inventiv-agents` (défaut)

---

## 🚀 CI/CD (GitHub Actions)

### Workflows

#### 1. **CI** (`.github/workflows/ci.yml`)
- **Déclenchement**: PR + push sur `main`
- **Jobs**:
  - Rust: `fmt-check`, `clippy`, `test`, `security-check`, `agent-version-check`
  - Frontend: `lint`, `build`
- **Reusable**: `workflow_call` pour gate aux déploiements

#### 2. **Deploy Staging** (`.github/workflows/deploy-staging.yml`)
- **Déclenchement**: push sur `main` (+ manuel `workflow_dispatch`)
- **Pipeline**:
  1. Exécute CI (reusable)
  2. Build + push images `:<sha12>` (**linux/arm64**)
  3. Promotion `:<sha12>` → `:staging` (même digest)
  4. `make stg-update` (remote deploy)

#### 3. **Deploy Production** (`.github/workflows/deploy-prod.yml`)
- **Déclenchement**: **manuel** (`workflow_dispatch`)
- **Input**: `image_tag` (sha12 ou vX.Y.Z)
- **Pipeline**:
  1. Promotion `image_tag` → `:prod` (même digest)
  2. `make prod-update` (remote deploy)
- **Protection**: Environment `production` avec approval requis

#### 4. **GHCR Build** (`.github/workflows/ghcr.yml`)
- **Déclenchement**: push tag `v*`
- **Pipeline**:
  1. Build + push `:<sha12>` (**linux/arm64**)
  2. Tag version `:<vX.Y.Z>` (même digest)
  3. Promotion optionnelle vers `:staging` ou `:prod` (manuel)

### Secrets GitHub requis

#### Environment `staging`
- `STG_REMOTE_HOST` (IP Flexible Scaleway)
- `STG_SECRETS_DIR` (ex: `/opt/inventiv/secrets-staging`)
- `STG_SSH_PRIVATE_KEY` (clé privée SSH, multi-ligne)
- `STG_POSTGRES_PASSWORD`
- `STG_WORKER_AUTH_TOKEN`
- `STG_ROOT_DOMAIN` (ex: `inventiv-agents.fr`)
- `STG_FRONTEND_DOMAIN` (ex: `studio-stg.inventiv-agents.fr`)
- `STG_API_DOMAIN` (ex: `api-stg.inventiv-agents.fr`)
- `STG_ACME_EMAIL`

#### Environment `production`
- `PROD_REMOTE_HOST`
- `PROD_SECRETS_DIR`
- `PROD_SSH_PRIVATE_KEY`
- `PROD_POSTGRES_PASSWORD`
- `PROD_WORKER_AUTH_TOKEN`
- `PROD_ROOT_DOMAIN`
- `PROD_FRONTEND_DOMAIN`
- `PROD_API_DOMAIN`
- `PROD_ACME_EMAIL`

#### Optionnels (valeurs par défaut)
- `STG_REMOTE_PORT` / `PROD_REMOTE_PORT` (défaut: 22)
- `STG_REMOTE_USER` / `PROD_REMOTE_USER` (défaut: `ubuntu`)
- `IMAGE_REPO` (défaut: `ghcr.io/<owner>/inventiv-agents`)
- `GHCR_USERNAME` (défaut: `<owner>`)

---

## 📚 Documentation

### Architecture & Design
- `docs/architecture.md` - Architecture générale (CQRS, Event-Driven)
- `docs/domain_design.md` - Design domain-driven
- `docs/specification_generale.md` - Spécifications générales
- `docs/worker_and_router_phase_0_2.md` - Worker & Router (phase 0.2)

### CI/CD & Déploiement
- `docs/CI_CD.md` - **Guide CI/CD complet** (local + GitHub Actions)
- `docs/SCALEWAY_PROVISIONING.md` - Provisioning Scaleway

### Features récentes
- `docs/STATE_MACHINE_AND_PROGRESS.md` - State machine + progress tracking
- `docs/STORAGE_MANAGEMENT.md` - Gestion volumes
- `docs/AGENT_VERSION_MANAGEMENT.md` - Versioning agent
- `docs/VOLUME_HISTORY_ENHANCEMENT.md` - Historique volumes

### Multi-tenant
- `docs/MULTI_TENANT_ROADMAP.md` - Roadmap multi-tenant
- `docs/MULTI_TENANT_MODEL_SHARING_BILLING.md` - Partage modèles + billing

### Sessions & Auth (nouveau)
- `docs/SESSION_ARCHITECTURE_PROPOSAL.md` - Architecture sessions multi-org
- `docs/SESSION_AUTH_ANALYSIS.md` - Analyse auth
- `docs/CHAT_SESSIONS_AND_INFERENCE.md` - Sessions chat

### UI & Design System
- `docs/ui_design_system.md` - Design system
- `docs/ia_widgets.md` - Widgets réutilisables
- `docs/INVENTIV_DATA_TABLE.md` - Table virtualisée

### Monitoring & Observabilité
- `docs/MONITORING_IMPROVEMENTS.md` - Améliorations monitoring
- `docs/OBSERVABILITY_ANALYSIS.md` - Analyse observabilité

---

## ✅ Fonctionnalités implémentées

### Control-plane & Provisioning
- ✅ Provisioning Scaleway (VM + Block Storage automatique)
- ✅ Provisioning Mock (Docker Compose)
- ✅ State machine complète (booting → installing → starting → ready)
- ✅ Progress tracking 0-100% (granulaire)
- ✅ Jobs background (health-check, terminator, watch-dog, provisioning)
- ✅ Auto-install worker (SSH bootstrap avec phases)
- ✅ Storage management (découverte volumes, suppression auto)
- ✅ Volume reconciliation (détection orphelins)

### Modèles & Readiness
- ✅ Catalogue `models` (is_active, data_volume_gb)
- ✅ Sélecteur modèle obligatoire (UI + API)
- ✅ Readiness industrialisée (WORKER_VLLM_HTTP_OK, WORKER_MODEL_LOADED, WORKER_VLLM_WARMUP)
- ✅ Résolution image vLLM par type d'instance/GPU

### OpenAI-compatible API
- ✅ Endpoints `/v1/models`, `/v1/chat/completions`, `/v1/completions`, `/v1/embeddings`
- ✅ API keys (CRUD + auth Bearer)
- ✅ Live capacity (modèles réellement servis)
- ✅ Streaming

### Auth & Sessions
- ✅ Sessions multi-org (`user_sessions` table)
- ✅ Auth worker (token par instance)
- ✅ Bootstrap admin (mot de passe depuis secret file)
- ✅ **En cours**: Migration sessions existantes

### Multi-tenant (MVP)
- ✅ Organisations (création + membership)
- ✅ Sélection organisation courante (switcher UX)
- ✅ Pré-câblage DB (model sharing + chargeback tokens)

### FinOps
- ✅ Coûts réels/forecast/cumulatifs (EUR)
- ✅ Comptage tokens in/out (par instance)
- ✅ Métriques requêtes (`instance_request_metrics`)

### Observabilité
- ✅ SSE temps-réel (`GET /events/stream`)
- ✅ Métriques système (CPU/Mem/Disk/Net)
- ✅ Métriques GPU (par index)
- ✅ Métriques requêtes/tokens (`GET /instances/:id/metrics`)
- ✅ Worker event logging (`/logs` endpoint)

---

## 🐛 Bugs connus / Dettes techniques

- ⚠️ **Volumes non libérés**: Certaines terminaisons ne libèrent pas correctement les block storage
- ⚠️ **SSE**: Implémentation basée sur polling DB (pas event-sourced)
- ⚠️ **Observabilité**: Pas encore de stack métriques/traces end-to-end (Prometheus/Grafana/OTel)
- ⚠️ **Mock provider routing**: Test E2E override `instances.ip_address` (hack local)

---

## 🚧 À faire (priorités)

### Court terme (avant déploiement staging/prod)
1. ✅ **CI/CD complète** (local + GitHub Actions) - **FAIT**
2. ✅ **Build ARM64** (Scaleway) - **FAIT**
3. ⚠️ **Migrations non appliquées** (user_sessions, provider_settings)
4. ⚠️ **Secrets GitHub** (environments staging/production)
5. ⚠️ **Provisioning VMs** (staging + prod)

### Moyen terme
- Déploiement staging (`studio-stg.inventiv-agents.fr`)
- Déploiement production (`studio-prd.inventiv-agents.fr`)
- Tests E2E sur staging
- Monitoring/alerting basique

### Long terme
- Autoscaling MVP
- RBAC fin (org-scoped)
- Partage modèles inter-org
- Chargeback tokens (v1)

---

## 📋 Prochaines Actions (Déploiement Staging/Prod)

### 1. Commit des changements en cours
```bash
# Vérifier les changements
git status
git diff

# Commiter les migrations + code sessions
git add sqlx-migrations/202601070000*.sql
git add inventiv-api/src/auth*.rs inventiv-api/src/bootstrap_admin.rs
git add inventiv-api/src/organizations.rs
git commit -m "feat: sessions multi-org + migrations"

# Commiter les autres changements
git add ...
git commit -m "..."
```

### 2. Vérifier les secrets GitHub
- Créer environments `staging` et `production`
- Configurer les secrets requis (voir section CI/CD)
- Configurer "required reviewers" pour `production`

### 3. Provisionner VMs Scaleway
```bash
# Staging
make stg-provision      # Créer/réutiliser VM + attach flex IP
make stg-bootstrap      # Install docker/compose
make stg-secrets-sync   # Upload secrets

# Production
make prod-provision
make prod-bootstrap
make prod-secrets-sync
```

### 4. Déployer Staging (via CI/CD)
```bash
# Push sur main déclenche automatiquement:
# 1. CI (fmt/clippy/test)
# 2. Build + push images :<sha12> (arm64)
# 3. Promotion → :staging
# 4. make stg-update (remote)
```

### 5. Déployer Production (manuel)
- Via GitHub Actions UI: `workflow_dispatch` sur `deploy-prod.yml`
- Input: `image_tag` (sha12 du commit staging validé)
- Approval requis (environment `production`)

---

## 🔍 Points d'attention

1. **Migrations non appliquées**: `user_sessions` et `provider_settings` doivent être appliquées avant déploiement
2. **Build ARM64**: Les workflows buildent maintenant en `linux/arm64` (compatible Scaleway)
3. **Secrets**: Vérifier que tous les secrets GitHub sont configurés
4. **SSH keys**: Les clés sont dans `.ssh/llm-studio-key` (gitignored, OK)
5. **Environments**: Créer les environments GitHub avec protection si nécessaire

---

**Dernière mise à jour**: 7 janvier 2026  
**Prochaine session**: Déploiement staging/prod

