# Inventiv Agents

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![GHCR (build + promote)](https://github.com/Inventiv-IT-for-AI/inventiv-agents/actions/workflows/ghcr.yml/badge.svg)](https://github.com/Inventiv-IT-for-AI/inventiv-agents/actions/workflows/ghcr.yml)
[![Version](https://img.shields.io/badge/version-0.4.5-blue.svg)](VERSION)

**Control-plane + data-plane to run AI agents/instances** — Scalable, modular, and performant LLM inference infrastructure, written in **Rust**.

## TL;DR (30 seconds)

**Inventiv Agents** is an open-source platform (AGPL v3) that orchestrates the complete lifecycle of GPU instances for LLM inference: automatic provisioning, health-check, scaling, FinOps monitoring, and multi-provider management (Scaleway, Mock).

**Why it's useful**: Enables standardized deployment and scaling of LLM models (vLLM), with integrated financial tracking and granular control over cloud resources.

📘 **Detailed documentation**: [Architecture](docs/architecture.md) | [Domain Design & CQRS](docs/domain_design.md) | [General Specifications](docs/specification_generale.md) | [UI Design System](docs/ui_design_system.md) | [`ia-widgets`](docs/ia_widgets.md) | [Engineering Guidelines](docs/engineering_guidelines.md)

## Key Features

- ✅ **Provisioning / Termination**: Automatic creation and destruction of GPU instances via providers (Scaleway, Mock)
- ✅ **Health-check & Reconciliation**: Continuous monitoring of instances, orphan detection, automatic retry
- ✅ **Redis Event Bus**: Event-driven architecture with `CMD:*` (commands) and `EVT:*` (events)
- ✅ **Orchestrator (jobs + state machine)**: Asynchronous lifecycle management (booting → ready → terminating → terminated)
- ✅ **Worker (agent runtime)**: Python agent deployed on GPU instances, heartbeat, readiness (`/readyz`), metrics
- ✅ **FinOps (costs/forecast)**: Tracking of real and forecasted costs by instance/type/region/provider, time windows (minute/hour/day/30d/365d)
- ✅ **Frontend (web console)**: Next.js dashboard with FinOps monitoring, instance management, settings (providers/zones/types), action logs
- ✅ **Auth (JWT session + users)**: Cookie-based session authentication, user management, automatic admin bootstrap
- ✅ **Worker Auth (token per instance)**: Secure worker authentication with hashed tokens in DB, automatic bootstrap

## Architecture (Overview)

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

### Components (repo layout)

- **`inventiv-api`** (Rust): Synchronous HTTP API, session-protected endpoints, Swagger UI
- **`inventiv-orchestrator`** (Rust): Asynchronous control plane, background jobs, state machine
- **`inventiv-finops`** (Rust): Service for calculating real and forecasted costs (TimescaleDB tables)
- **`inventiv-worker`** (Python): Sidecar agent deployed on GPU instances, heartbeat, readiness
- **`inventiv-frontend`** (Next.js): UI dashboard with Tailwind + shadcn/ui
- **`inventiv-common`** (Rust): Shared library (types, DTOs, events)

**References**:
- [Detailed Architecture](docs/architecture.md)
- [Domain Design & CQRS](docs/domain_design.md)
- [Worker & Router Phase 0.2](docs/worker_and_router_phase_0_2.md)
- [Multi-tenant: Organizations + model sharing + billing tokens](docs/MULTI_TENANT_MODEL_SHARING_BILLING.md)

## Prerequisites

- **Docker** & **Docker Compose** v2.0+ (for the complete stack)
- **Git**
- **Make** (optional, but recommended for using `make` commands)

**Compatibility**: Windows (WSL2 recommended), Linux, macOS (Intel and Apple Silicon)

**Note**: Rust and Node.js are not required for local development — everything runs in Docker.

See [docs/DEVELOPMENT_SETUP.md](docs/DEVELOPMENT_SETUP.md) for the detailed configuration guide by platform.

## Quickstart (local dev)

### 1. Configuration

```bash
# Create local env file (not committed)
cp env/dev.env.example env/dev.env

# Create admin secret (not committed)
mkdir -p deploy/secrets
echo "<your-admin-password>" > deploy/secrets/default_admin_password
```

> Note: if you use a **private** Hugging Face model, prefer `WORKER_HF_TOKEN_FILE` (secret file) rather than a token in plain text in `env/*.env`.

### 2. Start the stack

```bash
# Compile and start all services (Postgres, Redis, API, Orchestrator, FinOps)
make up
```

**Local URLs**:
- **Frontend (UI)**: `http://localhost:3000` (or `3000 + PORT_OFFSET`, see step 3)
- **API / Orchestrator / DB / Redis**: **not exposed on host by default** (communication via Docker network)

If you need to access the API from the host (e.g., Cloudflare tunnel), use:

```bash
make api-expose   # exposes API on loopback 127.0.0.1:(8003 + PORT_OFFSET)
```

To stop the stack **without losing DB/Redis state**:

```bash
make down
```

To start from scratch (**wipe db/redis volumes**):

```bash
make nuke
```

### 3. Start the Frontend (UI)

**Recommended option** (via Makefile):

```bash
make ui
```

This starts Next.js in Docker, exposed on `http://localhost:3000` (or `3000 + PORT_OFFSET`).
Backend calls go through same-origin routes `/api/backend/*` on the frontend (server-side proxy to `API_INTERNAL_URL=http://api:8003` in the Docker network).

> Note (monorepo): JS/TS packages (e.g., `inventiv-frontend`, `inventiv-ui/ia-widgets`) coexist with Rust/Python services.
> The repo uses **npm workspaces** to manage only these directories — the rest (Rust/Python/infra) is not affected.

## UI / Design system

We maintain a design system based on **Tailwind v4 + shadcn/ui**, with a simple rule:
**no new widgets/components invented without validating the need and style**.

- Charter & conventions: [UI Design System](docs/ui_design_system.md)
- Centralized UI primitives (shadcn-style): `inventiv-ui/ia-designsys` (import: `ia-designsys`)
- Reusable widgets: [`ia-widgets`](docs/ia_widgets.md) (`inventiv-ui/ia-widgets`, import: `ia-widgets`)

## Clean code / maintainability

Important: avoid turning pivot files (`main.rs`, `page.tsx`, …) into "god files".
We apply SRP (*one file / one module / one mission*) and keep entrypoints "thin" to make code readable and testable.

Reference: [Engineering Guidelines](docs/engineering_guidelines.md)

**Option "UI on host" (debug)**:

```bash
# 0) Start the stack (API in Docker)
make up

# 1) Expose API on loopback (if you want to run UI outside Docker)
make api-expose

# 2) Install JS dependencies (monorepo) at root
npm install --no-audit --no-fund

# 3) Start Next.js (host) in webpack mode (reliable watch for workspaces)
API_INTERNAL_URL="http://127.0.0.1:8003" \
  npm -w inventiv-frontend run dev -- --webpack --port 3000
```

Quickly stop the UI:

```bash
make ui-down        # stop UI in Docker
make ui-local-down  # kill local process on UI port
```

### 4. Authentication

- **Login**: Access `http://localhost:3000/login`
- **Bootstrap admin**: An `admin` user is automatically created at startup if absent
  - Username: `admin` (or `DEFAULT_ADMIN_USERNAME`)
  - Email: `admin@inventiv.local` (or `DEFAULT_ADMIN_EMAIL`)
  - Password: read from `deploy/secrets/default_admin_password` (or `DEFAULT_ADMIN_PASSWORD_FILE`)

### 5. Seeding (catalog)

In local dev, automatic seeding can be enabled via:

```bash
# In env/dev.env
AUTO_SEED_CATALOG=1
SEED_CATALOG_PATH=/app/seeds/catalog_seeds.sql
```

**Manual**:

```bash
docker compose --env-file env/dev.env exec -T db \
  psql -U postgres -d llminfra -f /app/seeds/catalog_seeds.sql
```

> The seed is **idempotent** (via `ON CONFLICT`) and can be re-run.

## Configuration (env vars)

### Reference files

Example files are in `env/*.env.example`:
- `env/dev.env.example`: local development
- `env/staging.env.example`: staging environment
- `env/prod.env.example`: production

### API URLs

See [docs/API_URL_CONFIGURATION.md](docs/API_URL_CONFIGURATION.md) for detailed frontend configuration.

**Frontend**: `NEXT_PUBLIC_API_URL` in `inventiv-frontend/.env.local`

### Secrets

Runtime secrets are mounted in containers via `SECRETS_DIR` → `/run/secrets`:

- `default_admin_password`: admin password (bootstrap)
- `jwt_secret`: JWT secret for sessions (optional, dev fallback)
- `scaleway_secret_key`: Scaleway secret key (if real provider)

**In local dev**: create `deploy/secrets/` and place secret files there.

**In staging/prod**: use `make stg-secrets-sync` / `make prod-secrets-sync` to sync from the VM.

### Modes (dev / staging / prod)

- **Dev**: `make dev-*` uses `env/dev.env` (required)
- **Staging**: `make stg-*` uses `env/staging.env`
- **Prod**: `make prod-*` uses `env/prod.env`

### Scaleway (real provisioning)

To enable real Scaleway provisioning:

```bash
# In env/dev.env (local) or env/staging.env / env/prod.env (remote)
SCALEWAY_PROJECT_ID=<your-project-id>
# Recommended (secret file mounted in containers)
SCALEWAY_SECRET_KEY_FILE=/run/secrets/scaleway_secret_key
#
# Alternative (less recommended): secret in plain text via env var
SCALEWAY_SECRET_KEY=<your-secret-key>
# Supported aliases (if you already use these names elsewhere):
# - SCALEWAY_API_TOKEN
# - SCW_SECRET_KEY
# Optional as needed
SCALEWAY_ACCESS_KEY=<your-access-key>
```

In staging/prod, secrets are synchronized on the VM via `SECRETS_DIR` (see `make stg-secrets-sync` / `make prod-secrets-sync`).

## Data Model (DB)

### Main tables

- **`instances`**: State of GPU instances (status, IP, provider, zone, type)
- **`providers`** / **`regions`** / **`zones`** / **`instance_types`**: Catalog of available resources
- **`instance_type_zones`**: Zone ↔ instance type associations
- **`users`**: Users (username, email, password_hash, role)
- **`worker_auth_tokens`**: Worker authentication tokens (hashed, per instance)
- **`action_logs`**: Action logs (provisioning, termination, sync, etc.)
- **`finops.cost_*_minute`**: TimescaleDB tables for costs (actual, forecast, cumulative)

### Migrations

**SQLx Migrations**: `sqlx-migrations/` (automatically executed at boot by `inventiv-api` and `inventiv-orchestrator`)

**Principle**:
- Each migration is a SQL file with timestamp: `YYYYMMDDHHMMSS_description.sql`
- Migrations are automatically applied at Rust service startup
- Checksum validated to avoid accidental modifications

**Recent migrations**:
- `20251215000000_add_worker_heartbeat_columns.sql`: Heartbeat columns for instances
- `20251215001000_add_finops_forecast_horizons.sql`: FinOps forecast horizons (1h, 365d)
- `20251215002000_finops_use_eur.sql`: USD → EUR conversion for all FinOps fields
- `20251215010000_create_worker_auth_tokens.sql`: Worker tokens table
- `20251215020000_users_add_first_last_name.sql`: first_name/last_name fields for users
- `20251215021000_users_add_username.sql`: Unique username for login

### Seeds

**Catalog seeds**: `seeds/catalog_seeds.sql` (providers, regions, zones, instance_types, associations)

**Automatic (dev)**: enable via `AUTO_SEED_CATALOG=1` in `env/dev.env`

**Manual**:

```bash
psql "postgresql://postgres:password@localhost:5432/llminfra" -f seeds/catalog_seeds.sql
```

## Events & background jobs (orchestrator)

### Redis Bus

**Channels**:
- `orchestrator_events`: `CMD:*` commands published by the API
- `finops_events`: `EVT:*` events for FinOps (costs, tokens)

**Guarantees**: Non-durable Pub/Sub → requeue if orchestrator down

**Commands**:
- `CMD:PROVISION`: Provision an instance
- `CMD:TERMINATE`: Terminate an instance
- `CMD:SYNC_CATALOG`: Synchronize catalog (providers)
- `CMD:RECONCILE`: Manual reconciliation

### Jobs (orchestrator)

- **Health-check loop**: Transition `booting` → `ready` (check SSH:22 or `/readyz` worker)
- **Provisioning**: Management of "stuck" instances, automatic retry
- **Terminator**: Cleanup of instances in `terminating`
- **Watch-dog**: Detection of "orphan" instances (deleted by provider)

**Handlers**: `services::*` + state machine (see [docs/specification_generale.md](docs/specification_generale.md))

## API (inventiv-api)

### Auth

**JWT Session** (cookie):
- `POST /auth/login`: Login (username or email)
- `POST /auth/logout`: Logout
- `GET /auth/me`: User profile
- `PUT /auth/me`: Update profile
- `PUT /auth/me/password`: Change password

**User management** (admin only):
- `GET /users`: List users
- `POST /users`: Create a user
- `GET /users/:id`: User details
- `PUT /users/:id`: Update user
- `DELETE /users/:id`: Delete user

### Internal endpoints (worker)

**Proxy to orchestrator** (via API domain):
- `POST /internal/worker/register`: Worker registration (bootstrap token)
- `POST /internal/worker/heartbeat`: Worker heartbeat (token required)

**Worker auth**: Token per instance (`Authorization: Bearer <token>`), verified in DB (`worker_auth_tokens`)

### Business endpoints (session-protected)

**Instances**:
- `GET /instances`: List (filter `archived`)
- `GET /instances/:id`: Details
- `DELETE /instances/:id`: Terminate (status `terminating` + event)
- `PUT /instances/:id/archive`: Archive

**Deployments**:
- `POST /deployments`: Create an instance (publishes `CMD:PROVISION`)
  - `model_id` is **required** (request is rejected otherwise)

**Settings**:
- `GET/PUT /providers`, `/regions`, `/zones`, `/instance_types`
- `GET/PUT /instance_types/:id/zones`: Zone ↔ type associations
- `GET /zones/:zone_id/instance_types`: Available types for a zone

**Action logs**:
- `GET /action_logs`: List (filtering, limit)
- `GET /action_logs/search`: Paginated search + stats (virtualized UI)
- `GET /action_types`: Action types catalog (badge/color/icon)

**FinOps**:
- `GET /finops/cost/current`: Current cost
- `GET /finops/dashboard/costs/summary`: Dashboard summary (allocation, totals)
- `GET /finops/dashboard/costs/window`: Details by window (minute/hour/day/30d/365d)
- `GET /finops/cost/actual/minute`: Real costs time series
- `GET /finops/cost/cumulative/minute`: Cumulative costs time series

**Commands**:
- `POST /reconcile`: Manual reconciliation
- `POST /catalog/sync`: Manual catalog synchronization

### API Documentation

By default the API is **not exposed** on the host in dev (UI-only). To view Swagger from the browser:

```bash
make api-expose
```

Then:

- **Swagger UI**: `http://127.0.0.1:8003/swagger-ui` (or `8003 + PORT_OFFSET`)

- **OpenAPI spec**: `http://127.0.0.1:8003/api-docs/openapi.json` (or `8003 + PORT_OFFSET`)

### curl Examples

```bash
# (Option 1) From host: expose API on loopback
make api-expose

# Login (session cookie)
curl -X POST http://127.0.0.1:8003/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@inventiv.local","password":"<password>"}' \
  -c cookies.txt

# Create an instance (with session cookie)
curl -X POST http://127.0.0.1:8003/deployments \
  -H "Content-Type: application/json" \
  -b cookies.txt \
  -d '{"instance_type_id":"<uuid>","zone_id":"<uuid>","model_id":"<uuid>"}'

# List instances
curl http://127.0.0.1:8003/instances -b cookies.txt

# Terminate an instance
curl -X DELETE http://127.0.0.1:8003/instances/<id> -b cookies.txt
```

### OpenAI-compatible API (proxy)

The API exposes an OpenAI-compatible proxy (selects a READY worker for the requested model):

- `GET /v1/models`
- `POST /v1/chat/completions` (streaming supported)
- `POST /v1/completions`
- `POST /v1/embeddings`

Auth:
- session user **or**
- API key (Bearer)

## Worker (inventiv-worker)

### Role

Python agent deployed on GPU instances that:
- Exposes HTTP endpoints: `/healthz`, `/readyz`, `/metrics`
- Manages the inference engine (vLLM)
- Communicates with the control-plane via `/internal/worker/register` and `/internal/worker/heartbeat`

### Auth token

**Bootstrap**: On first `register`, the orchestrator generates a token and returns it (plaintext only in the response).

**Storage**: Token hashed in DB (`worker_auth_tokens`), then used via `Authorization: Bearer <token>`.

### Local execution (without GPU)

A local harness is available to validate "Worker ready" without GPU:

```bash
bash scripts/dev_worker_local.sh
```

**Components**:
- `mock-vllm`: Mock vLLM server (serves `GET /v1/models`)
- `worker-agent`: Python agent that exposes `/healthz`, `/readyz`, `/metrics` and talks to the control-plane

**Notes**:
- By default the script resets volumes (deterministic migrations). To avoid: `RESET_VOLUMES=0 bash scripts/dev_worker_local.sh`
- The worker contacts the control-plane via the API (`CONTROL_PLANE_URL=http://api:8003`) which proxies `/internal/worker/*` to the orchestrator

### Flavors / Providers

Folder `inventiv-worker/flavors/`: configurations by provider/environment.

## Frontend (inventiv-frontend)

### UI Stack

- **Next.js** (App Router)
- **Tailwind CSS** (styling)
- **shadcn/ui** (components: Card, Tabs, Button, etc.)
- **React Hooks**: `useFinops`, `useInstances`, etc.

### API Configuration

The browser talks **only** to the UI, which then proxies to the backend via `/api/backend/*`.

- **Recommended mode (UI in Docker)**: no need for `NEXT_PUBLIC_API_URL`, the UI uses `API_INTERNAL_URL=http://api:8003`.
- **UI on host mode**: set `NEXT_PUBLIC_API_URL` and expose the API via `make api-expose`.

See [docs/API_URL_CONFIGURATION.md](docs/API_URL_CONFIGURATION.md).

### Dev

```bash
cd inventiv-frontend
npm install          # First time
npm run dev -- --port 3000
```

**Via Makefile**: `make ui` (creates `.env.local` if absent)

## Deployment (dev/dev-edge/staging/prod)

### Local "prod-like" deployment (edge)

**File**: `deploy/docker-compose.nginx.yml`

**Components**:
- Nginx (reverse proxy + SSL via Let's Encrypt)
- Services: `inventiv-api`, `inventiv-orchestrator`, `inventiv-finops`, `postgres`, `redis`

**Commands**:

```bash
make edge-create    # Create edge stack
make edge-start     # Start
make edge-stop      # Stop
make edge-cert      # Generate/renew SSL certificates
```

### Remote (Scaleway)

**Staging**:

Target DNS (planned): `https://studio-stg.inventiv-agents.fr`

```bash
make stg-provision      # Provision VM
make stg-bootstrap      # Initial bootstrap
make stg-secrets-sync   # Sync secrets
make stg-create         # Create stack
make stg-start          # Start
make stg-cert           # Generate/renew certificates
```

**Production**:

Target DNS (planned): `https://studio-prd.inventiv-agents.fr`

```bash
make prod-provision
make prod-bootstrap
make prod-secrets-sync
make prod-create
make prod-start
make prod-cert
```

### Certificates

**Lego volume**: Export/import via `deploy/certs/lego_data_*.tar.gz`

**Configuration**: Variables `ROOT_DOMAIN`, `LEGO_DOMAINS`, `LEGO_APPEND_ROOT_DOMAIN` in `env/*.env`

### Images

**Tagging strategy**:
- SHA: `ghcr.io/<org>/<service>:<sha>`
- Version: `ghcr.io/<org>/<service>:v0.3.0`
- Latest: `ghcr.io/<org>/<service>:latest`

**Promotion**: By digest (SHA) to guarantee reproducibility

**GHCR login**: `make ghcr-login` (non-interactive via `scripts/ghcr_login.sh`)

## Observability & ops

### Logs

**Structured**: JSON (or text depending on configuration)

**Read logs**:

```bash
make logs              # All services
make dev-logs          # Local dev
make stg-logs          # Remote staging
make prod-logs         # Remote production
```

**Individual services**:

```bash
docker compose logs -f api
docker compose logs -f orchestrator
docker compose logs -f finops
```

### Healthchecks

**Orchestrator**: `GET http://localhost:8001/admin/status`

**API**: Swagger UI (`/swagger-ui`) + business endpoints

**Worker**: `/healthz` (liveness), `/readyz` (readiness)

### Monitoring

See [docs/MONITORING_IMPROVEMENTS.md](docs/MONITORING_IMPROVEMENTS.md) for planned improvements.

**Action logs**: `/action_logs/search` endpoint with pagination and stats

**FinOps**: Frontend dashboard with real/forecast/cumulative costs

### E2E Tests (mock) + Docker cleanup

A "mock" integration test exists to validate the **API → Orchestrator → Worker → API** chain (heartbeats + time series + OpenAI proxy):

```bash
RUST_LOG=info make test-worker-observability [PORT_OFFSET=...]
```

**Multi-instance test** (serial and parallel):

```bash
make test-worker-observability-multi [PORT_OFFSET=...]
```

Notes (important locally):
- **OpenAI proxy auth**: the test creates an **API key** (endpoint `POST /api_keys`) then calls `GET /v1/models` and `POST /v1/chat/completions` with `Authorization: Bearer <key>`.
- **OpenAI proxy with provider=mock**: in mock, the instance may have a "synthetic" IP not routable from the Docker network. The test therefore does a **local-only** override of `instances.ip_address` to the `mock-vllm` container IP to make `POST /v1/chat/completions` reachable.
- **Ports**: the API stack is exposed via `make api-expose` (loopback) to facilitate tunnels / worktrees.

#### Mock Provider and runtime management

The **Mock** provider automatically manages Docker Compose runtimes for each Mock instance created:

- **Automatic creation**: When provisioning a Mock instance, a Docker Compose runtime is automatically launched (`docker-compose.mock-runtime.yml`)
- **Shared network**: Mock runtimes connect to the control-plane Docker network to communicate with the API
- **IP per instance**: Each Mock instance gets its own IP on the Docker network (Option A)
- **Synchronization**: The `make mock-runtime-sync` command synchronizes runtimes with active instances in DB

**Manual management**:

```bash
# Start a Mock runtime for an instance
INSTANCE_ID=<uuid> make worker-attach

# Stop a Mock runtime
INSTANCE_ID=<uuid> make worker-detach

# Synchronize all Mock runtimes with active instances
make mock-runtime-sync
```

**Complete local stack** (control-plane + UI + Mock sync):

```bash
make local-up    # Start everything + synchronize Mock runtimes
make local-down   # Stop everything
```

#### Mock instance observability

The **Mock** provider automatically generates synthetic metrics for instances:

- Worker heartbeats (`worker_last_heartbeat`, `worker_status`)
- GPU metrics (`gpu_samples`: utilization, VRAM, temperature, power)
- System metrics (`system_samples`: CPU, memory, disk, network)

These metrics are generated by the Mock worker-agent and allow testing observability locally without a real GPU.

**Disable**:

- `MOCK_OBSERVABILITY_ENABLED=false` (not currently used, metrics always generated)

#### Real vLLM on Mock (optional)

The Mock provider can use a **real LLM model** (vLLM) instead of the simulated mock-vllm to test the **complete cycle** of provisioning:

```bash
# Enable real vLLM (CPU-only, works on Windows/Linux/macOS)
export MOCK_USE_REAL_VLLM=1
export MOCK_VLLM_MODEL="Qwen/Qwen2.5-0.5B-Instruct"

# Create a Mock instance (will use real vLLM)
```

**Objective**: Test the complete cycle (creation → routing → request processing → destruction) with real inference responses, **not to measure performance**.

**Performance**: vLLM runs in CPU-only mode locally (5-15s per request). Performance tests are done with real GPU VMs in production (Scaleway, etc.).

**Required resources**:
- RAM: 6-8GB (Docker)
- CPU: 4+ cores
- Compatible: Windows, Linux, macOS (Intel and Apple Silicon)

See [docs/MOCK_REAL_VLLM_USAGE.md](docs/MOCK_REAL_VLLM_USAGE.md) for more details.

If your Docker DB fails with `No space left on device`, you can clean up **unused** and **old** Docker resources (default: > 7 days) *project compose scope*:

```bash
make docker-prune-old
```

Options:
- `OLDER_THAN_HOURS=168` (default = 7 days)
- `CLEAN_ALL_UNUSED_IMAGES_OLD=1` (more aggressive: global prune of unused images > N hours)

"One-shot" command (cleanup + test):

```bash
make test-worker-observability-clean [PORT_OFFSET=...]
```

## Security

### Secret management

- **Secret files**: Mounted via `SECRETS_DIR` → `/run/secrets` (not committed)
- **Env vars**: Sensitive variables in `env/*.env` (not committed)
- **Bootstrap admin**: Password from secret file (`DEFAULT_ADMIN_PASSWORD_FILE`)
- **Seed provider credentials (optional)**: `AUTO_SEED_PROVIDER_CREDENTIALS=1` allows initializing `provider_settings` from secrets mounted in `/run/secrets` (e.g., Scaleway). See `env/staging.env.example` / `env/prod.env.example`.

### Worker tokens

- **Storage**: SHA-256 hash in DB (`worker_auth_tokens.token_hash`)
- **Bootstrap**: Plaintext token only in HTTP response (never logged)
- **Rotation**: `rotated_at`, `revoked_at` fields present (rotation not yet implemented)

### Best practices

- **X-Forwarded-For**: Gateway must override or only trust internal network
- **JWT secret**: Use strong `JWT_SECRET` in prod (insecure dev fallback)
- **Cookie Secure**: Enable `COOKIE_SECURE=1` in prod (HTTPS required)
- **Session TTL**: Configurable via `JWT_TTL_SECONDS` (default 12h)

See [SECURITY.md](SECURITY.md) for security reports.

## Contributing

### Dev setup

**Format / Lint**:

```bash
make check       # cargo check
make test        # Unit tests
```

**Conventions**:
- **Commits**: Conventional commits (`feat:`, `fix:`, `chore:`, etc.)
- **PR**: Clear description, reference issues if applicable

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for detailed guidelines and [docs/DEVELOPMENT_SETUP.md](docs/DEVELOPMENT_SETUP.md) for the multi-platform configuration guide.

## Roadmap / project status

### Stable

- ✅ Real Scaleway Provisioning/Termination
- ✅ Mock Provisioning/Termination with automatic Docker runtime management
- ✅ Health-check & Reconciliation
- ✅ FinOps dashboard (real/forecast/cumulative costs in EUR)
- ✅ Session auth + user management
- ✅ Worker auth (token per instance)
- ✅ Action logs + paginated search
- ✅ Modular provider architecture (`inventiv-providers` package)

### Experimental

- 🧪 Worker ready (local harness functional, real deployment in progress)
- 🧪 FinOps service (automatic calculations, depends on `inventiv-finops` running)

### Upcoming

- 🚧 **Router** (OpenAI-compatible): Reintroduction planned, not currently present
- 🚧 **Autoscaling**: Based on router/worker signals (queue depth, latency, GPU util)
- 🚧 **Token tracking**: Token consumption and forecast (priorities 4-5 FinOps)
- 🚧 **Fine RBAC**: Beyond `admin`, access policies per endpoint
- 🚧 **API Keys**: Backend management + router/gateway

See [TODO.md](TODO.md) for detailed backlog.

### Provider compatibility

- ✅ **Mock**: Local provider for tests (stateful in DB, automatic Docker Compose runtime management)
- ✅ **Scaleway**: Real integration (GPU instances)

**Provider architecture**:

Providers are implemented in the `inventiv-providers` package via the `CloudProvider` trait:
- `inventiv-providers/src/mock.rs`: Mock provider with Docker Compose management
- `inventiv-providers/src/scaleway.rs`: Scaleway provider (real API integration)

This architecture allows clear separation between the orchestrator (business logic) and providers (cloud specifics), facilitating the addition of new providers and testing.

## License

This project is licensed under **AGPL v3**. See the [LICENSE](LICENSE) file for more details.

**Copyright**: © 2025 Inventiv Agents Contributors
