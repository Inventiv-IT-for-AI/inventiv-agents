# Inventiv Agents

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![GHCR (build + promote)](https://github.com/Inventiv-IT-for-AI/inventiv-agents/actions/workflows/ghcr.yml/badge.svg)](https://github.com/Inventiv-IT-for-AI/inventiv-agents/actions/workflows/ghcr.yml)
[![Version](https://img.shields.io/badge/version-0.4.1-blue.svg)](VERSION)

**Control-plane + data-plane pour exécuter des agents/instances IA** — Infrastructure d'inférence LLM scalable, modulaire et performante, écrite en **Rust**.

## TL;DR (30 secondes)

**Inventiv Agents** est une plateforme open-source (AGPL v3) qui orchestre le cycle de vie complet des instances GPU pour l'inférence LLM : provisioning automatique, health-check, scaling, monitoring FinOps, et gestion multi-provider (Scaleway, Mock).

**Pourquoi c'est utile** : Permet de déployer et scaler des modèles LLM (vLLM) de manière standardisée, avec suivi financier intégré et contrôle granulaire sur les ressources cloud.

📘 **Documentation détaillée** : [Architecture](docs/architecture.md) | [Domain Design & CQRS](docs/domain_design.md) | [Spécifications Générales](docs/specification_generale.md) | [UI Design System](docs/ui_design_system.md) | [`ia-widgets`](docs/ia_widgets.md) | [Engineering Guidelines](docs/engineering_guidelines.md)

## Fonctionnalités clés

- ✅ **Provisioning / Termination** : Création et destruction automatique d'instances GPU via providers (Scaleway, Mock)
- ✅ **Health-check & Reconciliation** : Surveillance continue des instances, détection d'orphans, retry automatique
- ✅ **Bus d'événements Redis** : Architecture event-driven avec `CMD:*` (commandes) et `EVT:*` (événements)
- ✅ **Orchestrator (jobs + state machine)** : Gestion asynchrone du cycle de vie (booting → ready → terminating → terminated)
- ✅ **Worker (agent runtime)** : Agent Python déployé sur instances GPU, heartbeat, readiness (`/readyz`), métriques
- ✅ **FinOps (coûts/forecast)** : Tracking des coûts réels et prévisionnels par instance/type/région/provider, fenêtres temporelles (minute/heure/jour/30j/365j)
- ✅ **Frontend (console web)** : Dashboard Next.js avec monitoring FinOps, gestion des instances, settings (providers/zones/types), action logs
- ✅ **Auth (session JWT + users)** : Authentification par session cookie, gestion des utilisateurs, bootstrap admin automatique
- ✅ **Worker Auth (token par instance)** : Authentification sécurisée des workers avec tokens hashés en DB, bootstrap automatique

## Architecture (vue d'ensemble)

```
┌─────────────┐
│   Frontend  │ (Next.js :3000)
│  (UI/Login) │
└──────┬──────┘
       │ HTTP (session JWT)
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

### Composants (repo layout)

- **`inventiv-api`** (Rust) : API HTTP synchrone, endpoints protégés par session, Swagger UI
- **`inventiv-orchestrator`** (Rust) : Control plane asynchrone, jobs de fond, state machine
- **`inventiv-finops`** (Rust) : Service de calcul des coûts réels et prévisionnels (tables TimescaleDB)
- **`inventiv-worker`** (Python) : Agent sidecar déployé sur instances GPU, heartbeat, readiness
- **`inventiv-frontend`** (Next.js) : UI dashboard avec Tailwind + shadcn/ui
- **`inventiv-common`** (Rust) : Bibliothèque partagée (types, DTOs, événements)

**Références** :
- [Architecture détaillée](docs/architecture.md)
- [Domain Design & CQRS](docs/domain_design.md)
- [Worker & Router Phase 0.2](docs/worker_and_router_phase_0_2.md)
- [Multi-tenant: Organisations + partage de modèles + billing tokens](docs/MULTI_TENANT_MODEL_SHARING_BILLING.md)

## Prérequis

- **Docker** & **Docker Compose** (pour la stack complète)
- **Rust toolchain** (si build local des services Rust)
- **Node.js** (v18+) et **npm** (si frontend local)
- **Make** (optionnel, pour l'automatisation)
- **Accès provider** (ex: Scaleway) si test infra réel

## Quickstart (dev local)

### 1. Configuration

```bash
# Créer le fichier d'env local (non commité)
cp env/dev.env.example env/dev.env

# Créer le secret admin (non commité)
mkdir -p deploy/secrets
echo "<your-admin-password>" > deploy/secrets/default_admin_password
```

> Note: si tu utilises un modèle Hugging Face **privé**, préfère `WORKER_HF_TOKEN_FILE` (secret file) plutôt qu’un token en clair dans `env/*.env`.

### 2. Lancement de la stack

```bash
# Compiler et lancer tous les services (Postgres, Redis, API, Orchestrator, FinOps)
make up
```

**URLs locales** :
- **Frontend (UI)** : `http://localhost:3000` (ou `3000 + PORT_OFFSET`, voir étape 3)
- **API / Orchestrator / DB / Redis** : **non exposés sur le host par défaut** (communication via réseau Docker)

Si tu as besoin d’accéder à l’API depuis le host (ex: tunnel Cloudflare), utilise :

```bash
make api-expose   # expose l’API en loopback 127.0.0.1:(8003 + PORT_OFFSET)
```

Pour arrêter la stack **sans perdre l’état DB/Redis** :

```bash
make down
```

Pour repartir de zéro (**wipe les volumes db/redis**) :

```bash
make nuke
```

### 3. Lancer le Frontend (UI)

**Option recommandée** (via Makefile) :

```bash
make ui
```

Cela démarre Next.js dans Docker, exposé sur `http://localhost:3000` (ou `3000 + PORT_OFFSET`).
Les appels backend passent via des routes same-origin `/api/backend/*` côté frontend (proxy server-side vers `API_INTERNAL_URL=http://api:8003` dans le réseau Docker).

> Note (monorepo): les packages JS/TS (ex: `inventiv-frontend`, `inventiv-ui/ia-widgets`) cohabitent avec les services Rust/Python.
> Le repo utilise **npm workspaces** pour gérer uniquement ces dossiers — le reste (Rust/Python/infra) n’est pas impacté.

## UI / Design system

Nous maintenons un design system basé sur **Tailwind v4 + shadcn/ui**, avec une règle simple:
**pas de nouveaux widgets/components inventés sans validation du besoin et du style**.

- Charte & conventions: [UI Design System](docs/ui_design_system.md)
- Primitives UI centralisées (shadcn-style): `inventiv-ui/ia-designsys` (import: `ia-designsys`)
- Widgets réutilisables: [`ia-widgets`](docs/ia_widgets.md) (`inventiv-ui/ia-widgets`, import: `ia-widgets`)

## Clean code / maintenabilité

Important: éviter de transformer les fichiers pivots (`main.rs`, `page.tsx`, …) en “god files”.
On applique SRP (*un fichier / un module / une mission*) et on garde les entrypoints “thin” pour rendre le code lisible et testable.

Référence: [Engineering Guidelines](docs/engineering_guidelines.md)

**Option “UI sur le host” (debug)** :

```bash
# 0) Démarrer la stack (API dans Docker)
make up

# 1) Exposer l’API en loopback (si tu veux lancer l’UI hors Docker)
make api-expose

# 2) Installer les dépendances JS (monorepo) à la racine
npm install --no-audit --no-fund

# 3) Démarrer Next.js (host) en mode webpack (watch fiable workspaces)
API_INTERNAL_URL="http://127.0.0.1:8003" \
  npm -w inventiv-frontend run dev -- --webpack --port 3000
```

Arrêter rapidement l’UI :

```bash
make ui-down        # stop UI dans Docker
make ui-local-down  # kill process local sur le port UI
```

### 4. Authentification

- **Login** : Accéder à `http://localhost:3000/login`
- **Bootstrap admin** : Un utilisateur `admin` est créé automatiquement au démarrage si absent
  - Username : `admin` (ou `DEFAULT_ADMIN_USERNAME`)
  - Email : `admin@inventiv.local` (ou `DEFAULT_ADMIN_EMAIL`)
  - Password : lu depuis `deploy/secrets/default_admin_password` (ou `DEFAULT_ADMIN_PASSWORD_FILE`)

### 5. Seeding (catalogue)

En dev local, le seeding automatique peut être activé via :

```bash
# Dans env/dev.env
AUTO_SEED_CATALOG=1
SEED_CATALOG_PATH=/app/seeds/catalog_seeds.sql
```

**Manuel** :

```bash
docker compose --env-file env/dev.env exec -T db \
  psql -U postgres -d llminfra -f /app/seeds/catalog_seeds.sql
```

> Le seed est **idempotent** (via `ON CONFLICT`) et peut être re-joué.

## Configuration (env vars)

### Fichiers de référence

Les fichiers d'exemple sont dans `env/*.env.example` :
- `env/dev.env.example` : développement local
- `env/staging.env.example` : environnement staging
- `env/prod.env.example` : production

### URLs API

Voir [docs/API_URL_CONFIGURATION.md](docs/API_URL_CONFIGURATION.md) pour la configuration détaillée du frontend.

**Frontend** : `NEXT_PUBLIC_API_URL` dans `inventiv-frontend/.env.local`

### Secrets

Les secrets runtime sont montés dans les conteneurs via `SECRETS_DIR` → `/run/secrets` :

- `default_admin_password` : mot de passe admin (bootstrap)
- `jwt_secret` : secret JWT pour les sessions (optionnel, fallback dev)
- `scaleway_secret_key` : clé secrète Scaleway (si provider réel)

**En dev local** : créer `deploy/secrets/` et y placer les fichiers secrets.

**En staging/prod** : utiliser `make stg-secrets-sync` / `make prod-secrets-sync` pour synchroniser depuis la VM.

### Modes (dev / staging / prod)

- **Dev** : `make dev-*` utilise `env/dev.env` (obligatoire)
- **Staging** : `make stg-*` utilise `env/staging.env`
- **Prod** : `make prod-*` utilise `env/prod.env`

### Scaleway (provisioning réel)

Pour activer le provisioning Scaleway réel :

```bash
# Dans env/dev.env (local) ou env/staging.env / env/prod.env (remote)
SCALEWAY_PROJECT_ID=<your-project-id>
# Recommandé (secret file monté dans les conteneurs)
SCALEWAY_SECRET_KEY_FILE=/run/secrets/scaleway_secret_key
#
# Alternative (moins recommandé): secret en clair via env var
SCALEWAY_SECRET_KEY=<your-secret-key>
# Alias supportés (si tu utilises déjà ces noms ailleurs):
# - SCALEWAY_API_TOKEN
# - SCW_SECRET_KEY
# Optionnel selon besoin
SCALEWAY_ACCESS_KEY=<your-access-key>
```

En staging/prod, les secrets sont synchronisés sur la VM via `SECRETS_DIR` (voir `make stg-secrets-sync` / `make prod-secrets-sync`).

## Modèle de données (DB)

### Tables principales

- **`instances`** : État des instances GPU (status, IP, provider, zone, type)
- **`providers`** / **`regions`** / **`zones`** / **`instance_types`** : Catalogue des ressources disponibles
- **`instance_type_zones`** : Associations zone ↔ type d'instance
- **`users`** : Utilisateurs (username, email, password_hash, role)
- **`worker_auth_tokens`** : Tokens d'authentification des workers (hashé, par instance)
- **`action_logs`** : Logs d'actions (provisioning, termination, sync, etc.)
- **`finops.cost_*_minute`** : Tables TimescaleDB pour les coûts (actual, forecast, cumulative)

### Migrations

**Migrations SQLx** : `sqlx-migrations/` (exécutées automatiquement au boot par `inventiv-api` et `inventiv-orchestrator`)

**Principe** :
- Chaque migration est un fichier SQL avec timestamp : `YYYYMMDDHHMMSS_description.sql`
- Les migrations sont appliquées automatiquement au démarrage des services Rust
- Checksum validé pour éviter les modifications accidentelles

**Migrations récentes** :
- `20251215000000_add_worker_heartbeat_columns.sql` : Colonnes heartbeat pour instances
- `20251215001000_add_finops_forecast_horizons.sql` : Horizons de forecast FinOps (1h, 365j)
- `20251215002000_finops_use_eur.sql` : Conversion USD → EUR pour tous les champs FinOps
- `20251215010000_create_worker_auth_tokens.sql` : Table tokens workers
- `20251215020000_users_add_first_last_name.sql` : Champs first_name/last_name users
- `20251215021000_users_add_username.sql` : Username unique pour login

### Seeds

**Seeds catalogue** : `seeds/catalog_seeds.sql` (providers, regions, zones, instance_types, associations)

**Automatique (dev)** : activer via `AUTO_SEED_CATALOG=1` dans `env/dev.env`

**Manuel** :

```bash
psql "postgresql://postgres:password@localhost:5432/llminfra" -f seeds/catalog_seeds.sql
```

## Événements & jobs background (orchestrator)

### Bus Redis

**Canaux** :
- `orchestrator_events` : commandes `CMD:*` publiées par l'API
- `finops_events` : événements `EVT:*` pour FinOps (coûts, tokens)

**Garanties** : Pub/Sub non durable → requeue si orchestrator down

**Commandes** :
- `CMD:PROVISION` : Provisionner une instance
- `CMD:TERMINATE` : Terminer une instance
- `CMD:SYNC_CATALOG` : Synchroniser le catalogue (providers)
- `CMD:RECONCILE` : Réconciliation manuelle

### Jobs (orchestrator)

- **Health-check loop** : Transition `booting` → `ready` (check SSH:22 ou `/readyz` worker)
- **Provisioning** : Gestion des instances "stuck", retry automatique
- **Terminator** : Nettoyage des instances en `terminating`
- **Watch-dog** : Détection d'instances "orphan" (supprimées par le provider)

**Handlers** : `services::*` + state machine (voir [docs/specification_generale.md](docs/specification_generale.md))

## API (inventiv-api)

### Auth

**Session JWT** (cookie) :
- `POST /auth/login` : Login (username ou email)
- `POST /auth/logout` : Logout
- `GET /auth/me` : Profil utilisateur
- `PUT /auth/me` : Mise à jour profil
- `PUT /auth/me/password` : Changement mot de passe

**Gestion users** (admin uniquement) :
- `GET /users` : Liste des utilisateurs
- `POST /users` : Créer un utilisateur
- `GET /users/:id` : Détails utilisateur
- `PUT /users/:id` : Mettre à jour utilisateur
- `DELETE /users/:id` : Supprimer utilisateur

### Endpoints internes (worker)

**Proxy vers orchestrator** (via API domain) :
- `POST /internal/worker/register` : Enregistrement worker (bootstrap token)
- `POST /internal/worker/heartbeat` : Heartbeat worker (token requis)

**Auth worker** : Token par instance (`Authorization: Bearer <token>`), vérifié en DB (`worker_auth_tokens`)

### Endpoints métier (protégés par session)

**Instances** :
- `GET /instances` : Liste (filtre `archived`)
- `GET /instances/:id` : Détails
- `DELETE /instances/:id` : Terminer (status `terminating` + event)
- `PUT /instances/:id/archive` : Archiver

**Deployments** :
- `POST /deployments` : Créer une instance (publie `CMD:PROVISION`)
  - `model_id` est **obligatoire** (la requête est rejetée sinon)

**Settings** :
- `GET/PUT /providers`, `/regions`, `/zones`, `/instance_types`
- `GET/PUT /instance_types/:id/zones` : Associations zone ↔ type
- `GET /zones/:zone_id/instance_types` : Types disponibles pour une zone

**Action logs** :
- `GET /action_logs` : Liste (filtrage, limit)
- `GET /action_logs/search` : Recherche paginée + stats (UI virtualisée)
- `GET /action_types` : Catalogue des types d'actions (badge/couleur/icon)

**FinOps** :
- `GET /finops/cost/current` : Coût actuel
- `GET /finops/dashboard/costs/summary` : Résumé dashboard (allocation, totals)
- `GET /finops/dashboard/costs/window` : Détails par fenêtre (minute/heure/jour/30j/365j)
- `GET /finops/cost/actual/minute` : Série temporelle coûts réels
- `GET /finops/cost/cumulative/minute` : Série temporelle coûts cumulatifs

**Commands** :
- `POST /reconcile` : Réconciliation manuelle
- `POST /catalog/sync` : Synchronisation catalogue manuelle

### Documentation API

Par défaut l’API n’est **pas exposée** sur le host en dev (UI-only). Pour consulter Swagger depuis le navigateur :

```bash
make api-expose
```

Puis :

- **Swagger UI** : `http://127.0.0.1:8003/swagger-ui` (ou `8003 + PORT_OFFSET`)

- **OpenAPI spec** : `http://127.0.0.1:8003/api-docs/openapi.json` (ou `8003 + PORT_OFFSET`)

### Exemples curl

```bash
# (Option 1) Depuis le host : exposer l'API en loopback
make api-expose

# Login (session cookie)
curl -X POST http://127.0.0.1:8003/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@inventiv.local","password":"<password>"}' \
  -c cookies.txt

# Créer une instance (avec session cookie)
curl -X POST http://127.0.0.1:8003/deployments \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{"instance_type_id":"<uuid>","zone_id":"<uuid>","model_id":"<uuid>"}'

# Lister les instances
curl http://127.0.0.1:8003/instances -b cookies.txt

# Terminer une instance
curl -X DELETE http://127.0.0.1:8003/instances/<id> -b cookies.txt
```

### OpenAI-compatible API (proxy)

L’API expose un proxy OpenAI-compatible (sélection d’un worker READY pour le modèle demandé) :

- `GET /v1/models`
- `POST /v1/chat/completions` (streaming supporté)
- `POST /v1/completions`
- `POST /v1/embeddings`

Auth:
- session user **ou**
- API key (Bearer)

## Worker (inventiv-worker)

### Rôle

Agent Python déployé sur instances GPU qui :
- Expose des endpoints HTTP : `/healthz`, `/readyz`, `/metrics`
- Gère le moteur d'inférence (vLLM)
- Communique avec le control-plane via `/internal/worker/register` et `/internal/worker/heartbeat`

### Auth token

**Bootstrap** : Au premier `register`, l'orchestrator génère un token et le renvoie (plaintext uniquement dans la réponse).

**Stockage** : Token hashé en DB (`worker_auth_tokens`), utilisé ensuite via `Authorization: Bearer <token>`.

### Exécution locale (sans GPU)

Un harness local est disponible pour valider "Worker ready" sans GPU :

```bash
bash scripts/dev_worker_local.sh
```

**Composants** :
- `mock-vllm` : Mock serveur vLLM (sert `GET /v1/models`)
- `worker-agent` : Agent Python qui expose `/healthz`, `/readyz`, `/metrics` et parle au control-plane

**Notes** :
- Par défaut le script reset les volumes (migrations déterministes). Pour éviter : `RESET_VOLUMES=0 bash scripts/dev_worker_local.sh`
- Le worker contacte le control-plane via l'API (`CONTROL_PLANE_URL=http://api:8003`) qui proxy `/internal/worker/*` vers l'orchestrator

### Flavors / Providers

Dossier `inventiv-worker/flavors/` : configurations par provider/environnement.

## Frontend (inventiv-frontend)

### Stack UI

- **Next.js** (App Router)
- **Tailwind CSS** (styling)
- **shadcn/ui** (composants : Card, Tabs, Button, etc.)
- **React Hooks** : `useFinops`, `useInstances`, etc.

### Configuration API

Le navigateur parle **uniquement** à l’UI, qui proxy ensuite vers le backend via `/api/backend/*`.

- **Mode recommandé (UI dans Docker)** : pas besoin de `NEXT_PUBLIC_API_URL`, l’UI utilise `API_INTERNAL_URL=http://api:8003`.
- **Mode UI sur le host** : définir `NEXT_PUBLIC_API_URL` et exposer l’API via `make api-expose`.

Voir [docs/API_URL_CONFIGURATION.md](docs/API_URL_CONFIGURATION.md).

### Dev

```bash
cd inventiv-frontend
npm install          # Première fois
npm run dev -- --port 3000
```

**Via Makefile** : `make ui` (crée `.env.local` si absent)

## Déploiement (dev/dev-edge/staging/prod)

### Déploiement local "prod-like" (edge)

**Fichier** : `deploy/docker-compose.nginx.yml`

**Composants** :
- Nginx (reverse proxy + SSL via Let's Encrypt)
- Services : `inventiv-api`, `inventiv-orchestrator`, `inventiv-finops`, `postgres`, `redis`

**Commandes** :

```bash
make edge-create    # Créer la stack edge
make edge-start     # Démarrer
make edge-stop      # Arrêter
make edge-cert      # Générer/renew certificats SSL
```

### Remote (Scaleway)

**Staging** :

DNS cible (prévu) : `https://studio-stg.inventiv-agents.fr`

```bash
make stg-provision      # Provisionner la VM
make stg-bootstrap      # Bootstrap initial
make stg-secrets-sync   # Synchroniser les secrets
make stg-create         # Créer la stack
make stg-start          # Démarrer
make stg-cert           # Générer/renew certificats
```

**Production** :

DNS cible (prévu) : `https://studio-prd.inventiv-agents.fr`

```bash
make prod-provision
make prod-bootstrap
make prod-secrets-sync
make prod-create
make prod-start
make prod-cert
```

### Certificats

**Lego volume** : Export/import via `deploy/certs/lego_data_*.tar.gz`

**Configuration** : Variables `ROOT_DOMAIN`, `LEGO_DOMAINS`, `LEGO_APPEND_ROOT_DOMAIN` dans `env/*.env`

### Images

**Stratégie de tags** :
- SHA : `ghcr.io/<org>/<service>:<sha>`
- Version : `ghcr.io/<org>/<service>:v0.3.0`
- Latest : `ghcr.io/<org>/<service>:latest`

**Promotion** : Par digest (SHA) pour garantir la reproductibilité

**GHCR login** : `make ghcr-login` (non-interactif via `scripts/ghcr_login.sh`)

## Observabilité & ops

### Logs

**Structurés** : JSON (ou texte selon configuration)

**Lire les logs** :

```bash
make logs              # Tous les services
make dev-logs          # Dev local
make stg-logs          # Staging remote
make prod-logs         # Production remote
```

**Services individuels** :

```bash
docker compose logs -f api
docker compose logs -f orchestrator
docker compose logs -f finops
```

### Healthchecks

**Orchestrator** : `GET http://localhost:8001/admin/status`

**API** : Swagger UI (`/swagger-ui`) + endpoints métier

**Worker** : `/healthz` (liveness), `/readyz` (readiness)

### Monitoring

Voir [docs/MONITORING_IMPROVEMENTS.md](docs/MONITORING_IMPROVEMENTS.md) pour les améliorations prévues.

**Action logs** : Endpoint `/action_logs/search` avec pagination et stats

**FinOps** : Dashboard frontend avec coûts réels/forecast/cumulatifs

### Tests E2E (mock) + nettoyage Docker

Un test d’intégration “mock” existe pour valider la chaîne **API → Orchestrator → Worker → API** (heartbeats + séries temporelles + proxy OpenAI):

```bash
make test-worker-observability [PORT_OFFSET=...]
```

Si ta DB Docker échoue avec `No space left on device`, tu peux nettoyer les ressources Docker **inutilisées** et **anciennes** (par défaut: > 7 jours) *scope projet compose*:

```bash
make docker-prune-old
```

Options:
- `OLDER_THAN_HOURS=168` (défaut = 7 jours)
- `CLEAN_ALL_UNUSED_IMAGES_OLD=1` (plus agressif: prune global des images inutilisées > N heures)

Commande “one-shot” (nettoyage + test):

```bash
make test-worker-observability-clean [PORT_OFFSET=...]
```

## Sécurité

### Gestion des secrets

- **Secrets files** : Montés via `SECRETS_DIR` → `/run/secrets` (non commités)
- **Env vars** : Variables sensibles dans `env/*.env` (non commitées)
- **Bootstrap admin** : Mot de passe depuis fichier secret (`DEFAULT_ADMIN_PASSWORD_FILE`)

### Tokens worker

- **Stockage** : Hash SHA-256 en DB (`worker_auth_tokens.token_hash`)
- **Bootstrap** : Token plaintext uniquement dans la réponse HTTP (jamais loggé)
- **Rotation** : Champs `rotated_at`, `revoked_at` présents (rotation non implémentée encore)

### Bonnes pratiques

- **X-Forwarded-For** : Gateway doit écraser ou ne faire confiance qu'au réseau interne
- **JWT secret** : Utiliser `JWT_SECRET` fort en prod (fallback dev insecure)
- **Cookie Secure** : Activer `COOKIE_SECURE=1` en prod (HTTPS requis)
- **Session TTL** : Configurable via `JWT_TTL_SECONDS` (défaut 12h)

Voir [SECURITY.md](SECURITY.md) pour les reports de sécurité.

## Contribution

### Dev setup

**Format / Lint** :

```bash
make check       # cargo check
make test        # Tests unitaires
```

**Conventions** :
- **Commits** : Conventional commits (`feat:`, `fix:`, `chore:`, etc.)
- **PR** : Description claire, référence issues si applicable

Voir [CONTRIBUTING.md](CONTRIBUTING.md) pour les guidelines détaillées.

## Roadmap / état du projet

### Stable

- ✅ Provisioning/Termination Scaleway réel
- ✅ Health-check & Reconciliation
- ✅ FinOps dashboard (coûts réels/forecast/cumulatifs en EUR)
- ✅ Auth session + gestion users
- ✅ Worker auth (token par instance)
- ✅ Action logs + recherche paginée

### Expérimental

- 🧪 Worker ready (harness local fonctionnel, déploiement réel en cours)
- 🧪 FinOps service (calculs automatiques, dépend de `inventiv-finops` running)

### À venir

- 🚧 **Router** (OpenAI-compatible) : Réintroduction prévue, non présent actuellement
- 🚧 **Autoscaling** : Basé sur signaux router/worker (queue depth, latence, GPU util)
- 🚧 **Tokens tracking** : Consommation et forecast de tokens (priorités 4-5 FinOps)
- 🚧 **RBAC fin** : Au-delà de `admin`, politiques d'accès par endpoint
- 🚧 **API Keys** : Gestion backend + router/gateway

Voir [TODO.md](TODO.md) pour le backlog détaillé.

### Compatibilité providers

- ✅ **Mock** : Provider local pour tests (stateful en DB)
- ✅ **Scaleway** : Intégration réelle (instances GPU)

## Licence

Ce projet est sous licence **AGPL v3**. Voir le fichier [LICENSE](LICENSE) pour plus de détails.

**Copyright** : © 2025 Inventiv Agents Contributors
