# Roadmap & TODO (repo state + backlog)

This file reflects the **actual** state of the repo (code + migrations + UI) and what's next (prioritized).

---

## ✅ Completed (delivered in code)

### Control-plane & provisioning
- ✅ **Scaleway Provisioning** (orchestrator): VM creation with image only, automatic Block Storage (20GB), expansion to 200GB via CLI, poweron, IP retrieval, Security Groups, SSH accessible (~20s), state transitions. **Validated for L4-1-24G**.
- ✅ **Mock Provisioning** (inventiv-providers): automatic Docker Compose runtime management, IP retrieval, state transitions.
- ✅ **Modular provider architecture**: `inventiv-providers` package with `CloudProvider` trait, orchestrator/providers separation.
- ✅ **State machine + jobs**: provisioning/health-check/terminator/watch-dog + requeue.
- ✅ **Auto-install worker**: bootstrap via SSH with phases `::phase::…`, enriched logs in `action_logs.metadata`.
- ✅ **Storage sizing by model**: recommended size from `models` table (controlled fallbacks).
- ✅ **HF token**: support `WORKER_HF_TOKEN_FILE` (secret file) + alias `HUGGINGFACE_TOKEN`.
- ✅ **Scaleway Block Storage**: Validated sequence - automatic creation with image (20GB bootable), expansion to 200GB before startup, SSH operational after ~20 seconds.

### Models & readiness
- **`models` catalog**: fields `is_active`, `data_volume_gb`, metadata (enriched seed).
- **Mandatory model selector** in UI + **API enforcement** (`model_id` required to create an instance).
- **Industrialized readiness**: actions `WORKER_VLLM_HTTP_OK`, `WORKER_MODEL_LOADED`, `WORKER_VLLM_WARMUP`.
- **vLLM modes**: `mono` (1 vLLM) / `multi` (1 vLLM per GPU behind HAProxy sticky).

### OpenAI-compatible API + API keys
- **OpenAI proxy** (inventiv-api): `/v1/models`, `/v1/chat/completions` (streaming), `/v1/completions`, `/v1/embeddings`.
- **API keys (client)**: CRUD + auth `Authorization: Bearer <key>` (separate from worker tokens).
- **Live capacity**: `/v1/models` reflects models actually served by "fresh" workers (with staleness tolerance).
- ✅ **HuggingFace model resolution**: Fixed logic to avoid false positives with offering ids (`org_slug/model_code`)

### Runtime models dashboard + Workbench
- **Runtime models**: endpoint + UI page `/models` (instances, GPUs, VRAM, requests, failed).
- **Workbench**: UI page `/workbench` (base URL, snippets, test chat via API key).

### Real-time (UI)
- **SSE**: `GET /events/stream` (topics instances/actions) + frontend hook `useRealtimeEvents` (refresh instances + action logs).
- **IADataTable persistence**: persisted column preferences (sort/width/order/visibility) for IA tables (including the "Instance Actions" pop-in).

### UI / Design system (monorepo)
- **Internal packages**:
  - `inventiv-ui/ia-designsys` (centralized UI primitives)
  - `inventiv-ui/ia-widgets` (higher-level widgets, `IA*` prefix)
- **Tailwind v4 (CSS-first)**: added `@source` to workspace packages (`ia-widgets`, `ia-designsys`) to avoid any class purging.
- **IADataTable**: reusable virtualized table (in `ia-widgets`) + **resize via dedicated separators** (5px) between columns.
- **Dev ergonomics**: `make ui-down` and `make ui-local-down` (stop UI Docker / kill UI host).
- ✅ **Version Display**: Discrete badge under application title with popover on hover/click showing FE, BE version and build timestamp.
- ✅ **CI/CD Pipeline**: Complete GitHub Actions pipeline (automatic CI, automatic staging deployment, manual production deployment). Axum 0.8 standardization across all projects. Fixed unused imports and clippy/lint errors.

### Dev ergonomics
- **PORT_OFFSET** (worktrees) + UI-only exposed.
- **`make api-expose`**: loopback proxy for tunnels (cloudflared) without modifying `docker-compose.yml`.
- **DB/Redis stateful**: `make down` keeps volumes, `make nuke` wipes.

### Multi-tenant (MVP)
- **Organizations**: creation + membership + "current organization" selection (switcher UX).
- **DB pre-wiring "model sharing + token chargeback"** (non-breaking): tables `organization_models` + `organization_model_shares` + `finops.inference_usage` extension.

---

## 🐛 Known bugs / technical debt (to track)

- **SSE**: current implementation based on DB polling (efficient but not "event-sourced" → to improve via NOTIFY/LISTEN or Redis streams).
- **Observability**: no end-to-end metrics/traces stack yet (Prometheus/Grafana/OTel) + alerting.
- ✅ **FinOps**: costs OK + **token counting in/out** implemented (see "FinOps full features" section).
- **Docs**: some documents remain "vision" (router, bare-metal) vs "implemented".
- **Mock provider routing**: E2E test OpenAI proxy overrides `instances.ip_address` to `mock-vllm` (local hack). To replace with proper mechanism (see backlog).
- **Docker CLI version**: orchestrator uses Docker CLI 27.4.0 (compatible API 1.44+). To document Docker prerequisites in docs.
- ✅ **"starting" progress**: Fixed - "starting" instances now display correct progress
- ✅ **"starting" health checks**: Fixed - "starting" instances are now checked by health check job
- ✅ **Public model resolution**: Fixed - public HuggingFace models work without organization
- ⚠️ **Unreleased volumes**: Some instance terminations do not properly release associated block storage (see "Worker & Instance Reliability" section).
- ⚠️ **Remaining clippy warnings**: 37 non-blocking clippy style errors (equality checks, redundant closures, etc.) - to fix progressively to improve code quality.
- ⚠️ **Remaining frontend warnings**: 10 non-blocking ESLint warnings - to fix progressively to improve code quality.
- ✅ **Production deployment fixes**: Fixed secrets synchronization (SECRETS_DIR preservation), permissions (644 for Docker), and VM disk sizing (40GB staging, 100GB prod).

---

## 🚧 To do (backlog)

### Worker & Instance Reliability (Priority)

#### 1. Dead Worker Detection
- [ ] Create `job-worker-watchdog.rs` to detect workers without recent heartbeat (> 5 min)
- [ ] Automatic transition `ready` → `worker_dead` if heartbeat > configurable threshold
- [ ] Option for automatic reinstallation for dead workers
- [ ] Unit and E2E tests

#### 2. Health Check Improvements
- [ ] Implement exponential backoff for failed health checks
- [ ] Reduce default timeouts (configurable via env vars)
- [ ] Add health check result cache (< 30s)
- [ ] Health check latency metrics

#### 3. Job Recovery Extension
- [ ] Detect `installing` / `starting` stuck > configurable threshold
- [ ] Add alerts (structured logs) for stuck instances
- [ ] Circuit breaker for instances with too many consecutive failures

#### 4. Volume Reconciliation (IN PROGRESS)
- [ ] Create `job-volume-reconciliation.rs` to detect orphaned volumes
- [ ] Detect volumes in DB but not at provider (clean DB)
- [ ] Detect volumes at provider but not in DB (track and delete)
- [ ] Automatic retry with backoff for failed deletions
- [ ] Check volumes marked `deleted_at` but still exist at provider
- [ ] E2E tests to validate reconciliation

#### 5. Metrics & Observability
- [ ] Expose Prometheus metrics for all jobs (latency, failure rate, instances processed)
- [ ] Grafana dashboard (optional)
- [ ] Alert system based on metrics (stuck instances, dead workers, orphaned volumes)
- [ ] Extend `correlation_id` usage for end-to-end tracing

### Scaleway Provider - Validated Sequence Implementation
- [ ] **Adapt Scaleway Provider code** to use validated sequence:
  - Create instance with image only (no volumes)
  - Detect and expand automatically created Block Storage (20GB → 200GB) via CLI
  - Configure Security Groups (ports 22, 8000, 8080)
  - Verify SSH accessible before worker installation
- [ ] **Update generic state machine** to support new steps:
  - `PROVIDER_VOLUME_RESIZE` (25%)
  - `PROVIDER_SECURITY_GROUP` (45%)
  - `WORKER_SSH_ACCESSIBLE` (50%)
- [ ] **Test with other instance types**: L40S, H100 (sequence should be identical)
- [ ] **Documentation**: Update user guides with new sequence

## 🚧 To do (backlog)

### Deployment & DNS
- ✅ **Staging**: deployment on `studio-stg.inventiv-agents.fr` (API + edge routing + certs) - operational.
- ✅ **Production**: deployment on `studio-prd.inventiv-agents.fr` - operational.
- ✅ **VM Disk Sizing**: Support for custom root volume sizes (40GB staging, 100GB prod) via `SCW_ROOT_VOLUME_SIZE_GB`.
- ✅ **Secrets Management**: Fixed secrets synchronization to preserve environment-specific `SECRETS_DIR` and correct permissions (644) for Docker containers.

### UX / API
- **Configurable System Prompt** (Inventiv-Agents): UI + API + persistence (per model / per tenant / per key).
- **Streaming**: improve E2E streaming (Workbench + proxy + UI) + UX (cancellation, TTFT, tokens/sec).

### Observability / Monitoring
- ✅ **Metrics**: `/metrics` on API/orchestrator/worker + dashboards (CPU/Mem/Disk/Net + GPU per-index) + SLOs.
  - Implemented: system metrics (CPU/Mem/Disk/Net) and GPU in Observability dashboard
  - Implemented: request and token metrics per instance (`GET /instances/:instance_id/metrics`)
- ✅ **Progress Tracking**: 0-100% progress system based on completed actions
  - Implemented: automatic calculation in `inventiv-api/src/progress.rs`
  - Implemented: display in UI with dedicated column
  - Implemented: granular steps (SSH install, vLLM HTTP, model loaded, warmup, health check)
  - ✅ **Validated Scaleway sequence**: Specific steps added (PROVIDER_VOLUME_RESIZE 25%, PROVIDER_SECURITY_GROUP 45%, WORKER_SSH_ACCESSIBLE 50%)
  - ✅ **"installing" and "starting" statuses**: Added intermediate statuses for granular tracking
  - ✅ **Multi-status progress management**: Progress calculation fixed for "installing" and "starting"
  - ✅ **Multi-status health checks**: Health check job now checks "booting", "installing", and "starting"
- ✅ **Agent Version Management**: Versioning and SHA256 checksum for `agent.py`
  - Implemented: `AGENT_VERSION` and `AGENT_BUILD_DATE` constants in agent.py
  - Implemented: `/info` endpoint to expose version/checksum
  - Implemented: checksum verification in SSH bootstrap script
  - Implemented: Makefile tooling (`agent-checksum`, `agent-version-bump`, etc.)
  - Implemented: CI/CD integration (automatic verification, bump workflow)
  - Implemented: monitoring in health checks and heartbeats
- ✅ **Storage Management**: Automatic volume lifecycle management
  - Implemented: automatic discovery of attached volumes (`list_attached_volumes`)
  - Implemented: tracking in `instance_volumes` with `delete_on_terminate`
  - Implemented: automatic deletion on termination
  - Implemented: detection of automatically created boot volumes
- ✅ **State Machine**: Explicit transitions and history
  - Implemented: explicit functions in `state_machine.rs`
  - Implemented: history in `instance_state_history`
  - Implemented: structured logging with metadata
  - ✅ **Intermediate statuses**: Added "installing" and "starting" for granular tracking
  - ✅ **Multi-status transitions**: Support for transitions from "booting" or "installing" to "starting"
- ✅ **Worker Event Logging**: Structured logging system on worker for diagnostics
  - Implemented: `_log_event()` function in `agent.py` with automatic rotation (10MB, 10k lines)
  - Implemented: `/logs` endpoint to retrieve logs via HTTP (`?tail=N&since=ISO8601`)
  - Implemented: logged events (agent_started, register_start/success/failed, heartbeat_success/failed/exception, vllm_ready/not_ready, etc.)
  - Implemented: orchestrator integration (`fetch_worker_logs()`) to analyze logs before retrying SSH install
  - Implemented: container state verification via SSH (`check_containers_via_ssh()`) before retry
  - Implemented: diagnostic logs (`WORKER_CONTAINER_CHECK`, `WORKER_LOG_ERRORS`, `WORKER_LOG_FETCH`) in orchestrator
- **Tracing**: OTel (optional initially) + `correlation_id` correlation (API ↔ orchestrator ↔ worker ↔ upstream).
  - Partially: `correlation_id` added in API logs, to extend to other services
- **Infra monitoring**: GPU util, queue depth, vLLM health, errors, saturation, load-balancing quality.
- **E2E test chain (mock)**: extend test to also validate OpenAI routing without DB hack (see "mock provider routing" item).

### Mock provider / tests
- ✅ **Automatic Mock runtime management**: creation/deletion via Docker Compose in `inventiv-providers/src/mock.rs`.
- ✅ **Synchronization scripts**: `mock_runtime_sync.sh` to synchronize runtimes with active instances.
- ✅ **Multi-instance E2E tests**: `test_worker_observability_mock_multi.sh` to validate serial and parallel provisioning.
- ✅ **Docker CLI/Compose in orchestrator**: Docker CLI 27.4.0 + Docker Compose plugin v2.27.1 installed in `Dockerfile.rust`.
- ✅ **Explicit Docker network**: `CONTROLPLANE_NETWORK_NAME` configured in `docker-compose.yml` to avoid network errors.
- **OpenAI proxy routing in mock**: make upstream reachable without mutating `instances.ip_address` (options: routable mock IP, or "upstream_base_url" param per instance in DB, or "service name" resolution on API side when provider=mock).
- **Contractual tests**: add tests (Rust) for `register/heartbeat` payloads (schema/validation) + retro compat (old heartbeat payload without `system_samples`).
- **Mock provider documentation**: create `docs/providers.md` with architecture and usage guide.

### FinOps "full features"
- ✅ **Token counting in/out** per Worker / API_KEY / User / Tenant / Model.
  - Implemented: token extraction from streaming/non-streaming responses, storage in `instance_request_metrics` and `finops.inference_usage`
  - Endpoint: `GET /instances/:instance_id/metrics`
  - Dashboard: metrics displayed in Observability (`/observability`)
- **Validation**: consolidate dashboards + exports + time series.

### Secrets & credentials
- **AUTO_SEED_PROVIDER_CREDENTIALS**: clearly document the model "secrets in /run/secrets → encrypted pgcrypto provider_settings" + rotation/rollback + key conventions (`SCALEWAY_PROJECT_ID`, `SCALEWAY_SECRET_KEY_ENC`) + threat (logs/backup).

### Multi-tenant & security
- ✅ **Organizations (MVP)**: creation + membership + "current organization" selection (switcher UX).
- ✅ **DB pre-wiring "model sharing + chargeback"** (non-breaking):
  - `organizations` + `organization_memberships` + `users.current_organization_id`
  - `organization_models` (offering published by org)
  - `organization_model_shares` (provider→consumer contracts, `pricing` JSONB)
  - `finops.inference_usage` extension to attribute `provider_organization_id` / `consumer_organization_id` + `unit_price_eur_per_1k_tokens` + `charged_amount_eur`
- ✅ **RBAC Foundation**: RBAC module with Owner/Admin/Manager/User roles, delegation rules, double activation (tech/eco).
- ✅ **Member Management**: Endpoints to list/change role/remove members with "last owner" invariant.
- ✅ **Default Org Bootstrap**: Automatic creation of "Inventiv IT" org with admin as owner.
- ✅ **Password Reset Flow**: Scaleway TEM SMTP integration, secure token generation, reset emails, complete API endpoints.
- ✅ **Code Reorganization**: Major refactoring of `main.rs` (~3500 lines → ~86 lines), extraction into `config/`, `setup/`, `routes/`, `handlers/` modules for better maintainability.
- ✅ **Integration Tests**: Integration test infrastructure with `axum-test`, tests for auth, deployments, instances (Mock provider only to avoid cloud costs).
- ✅ **Axum 0.8 Upgrade**: Migration to `axum 0.8` and `axum-test 18.0`, fixes for `async_trait`, `SwaggerUi`, `FromRequestParts`, OpenAPI compatibility with `utoipa 5.4`.
- ✅ **Multi-Org Session Architecture**: `user_sessions` table created, migrations applied, GET/POST /auth/sessions endpoints implemented, SessionsDialog UI created, integration tests added (see `docs/syntheses/archives/SESSION_IMPLEMENTATION_STATUS.md`).
- ⏳ **Scoping Instances**: Isolate instances by `organization_id` + RBAC.
- ⏳ **Scoping Models**: Isolate models by `organization_id` + public/private visibility.
- ⏳ **Invitations**: Invite users by email to an organization.
- ⏳ **Scoping API Keys**: Isolate API keys by `organization_id`.
- ⏳ **Scoping Users**: Filter user list by workspace.
- ⏳ **Scoping FinOps**: Filter financial dashboards by workspace.
- ⏳ **Frontend Module Migration**: Hide/show modules by workspace + role.
- ⏳ **Double Activation**: Technical activation (Admin) + economic activation (Manager) per resource.
- ⏳ **Model Sharing & Billing**: Share models between orgs with token-based billing.

📄 Doc: `docs/syntheses/MULTI_TENANT_MODEL_SHARING_BILLING.md` (pricing v1 = **€/1k tokens**)
- **Tenants v1 (Org isolation)**:
  - Isolate "business" resources by `organization_id` (at minimum: instances, workbench_runs, action_logs, api_keys).
  - Introduce notion of **mandatory current org** for business endpoints (401/409 if not selected).
  - Clarify org RBAC: `owner|admin|manager|user` + policy per endpoint.
  - RBAC rules:
    - Invitations: Owner/Admin/Manager
    - Last Owner non-revocable
    - Immutable audit logs (no delete)
  - "Double activation":
    - Admin activates technically (providers/regions/zones/types/models/api_keys/users/plan)
    - Manager activates economically (providers/regions/zones/types/models/api_keys/users/plan)
    - Operational only if both activations are OK (per resource)
    - UX: display "non-operational" state + alert indicating missing flag (tech/eco)
  - (Later) **PostgreSQL RLS** once model is stabilized.
  - Anti-error UX: **sidebar color configurable per organization** (visual "scope changed").

📄 Target roadmap: `docs/syntheses/MULTI_TENANT_ROADMAP.md` (users first-class + org workspaces + community offerings + entitlements + token billing)

- **Org-owned API keys (planned)**:
  - Activate `api_keys.organization_id` (currently nullable) + data migration (if needed).
  - "Consumer org" resolution via API key (priority) or session (current org).

- **Inter-org model sharing (provider→consumer)**:
  - CRUD `organization_models` (publish/unpublish).
  - CRUD `organization_model_shares` (grant/pause/revoke + pricing JSONB).
  - "Virtual model" identifier convention: `org_slug/model_code` (OpenAI proxy side).
  - Clarify `visibility`: `public | unlisted | private` (private = org-only; unlisted = not listed but accessible if authorized).
  - Add "consumer org discovery prefs" (allow/hide public/paid/paid-with-contract).

- **Token chargeback (v1)**:
  - Ingestion/persistence of `finops.inference_usage` events with:
    - `consumer_organization_id`, `provider_organization_id`, `organization_model_id`
    - pricing v1: `eur_per_1k_tokens`, calculate `charged_amount_eur`
  - Expose dashboards/exports "consumption per org / provider / consumer".

### Data plane / perf
- **Load-balancing optimization** (sticky, health scoring, failover, retry policy).
- **Auto scale-up / auto scale-down**.
- **Support other Cloud Providers** (AWS/GCP/etc).
- **Support on-prem / private / shared bare metal servers**.

---

## 🎯 Next steps Multi-Tenant (priorities)

**Immediate Phase (Sprint 1)**:
1) **Multi-Org Session Architecture**: `user_sessions` table, `current_organization_id` migration, enrich JWT with `session_id` + `organization_role`  
2) **PK/FK Migration**: Apply migration `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql`

**Short Term (Sprint 2-3)**:
3) **Scoping Instances**: SQL migration + API + UI + Tests to isolate instances by `organization_id`  
4) **Scoping Models**: SQL migration + API + UI + Tests to isolate models by `organization_id`  
5) **Invitations**: SQL migration + API + UI + Tests to invite users by email

**Medium Term (Sprint 4-6)**:
6) **Scoping API Keys**: API + UI + Tests  
7) **Scoping Users**: API + UI + Tests  
8) **Scoping FinOps**: API + UI + Tests  
9) **Frontend Module Migration**: Hide/show by workspace + role

**Long Term (Sprint 7+)**:
10) **Double Activation**: Tech (Admin) + Eco (Manager) per resource  
11) **Model Sharing & Billing**: Share models between orgs with token-based billing

**Other priorities**:
- **Deploy Staging + DNS** (`studio-stg.inventiv-agents.fr`) with proper UI/API routing + certs  
- **Observability** (minimum viable metrics + dashboards)  
- **LB hardening** + worker signals (queue depth / TTFT)  
- **Autoscaling MVP** (policies + cooldowns)

---

## 🧪 Tests & Validation (new features)

### Progress Tracking
- ✅ **Scaleway E2E Test**: Validated with script `test-scaleway/test_complete_validation.rs` - all steps work
- [ ] **Unit test**: Verify progress calculation for each step
- [ ] **Mock E2E test**: Validate simulated progress for Mock instances
- [ ] **UI test**: Verify progress column display in table
- [ ] **SSE test**: Verify real-time progress update

### Agent Version Management
- [ ] **Checksum test**: Verify checksum is calculated correctly
- [ ] **Verification test**: Validate bootstrap script detects invalid checksums
- [ ] **/info endpoint test**: Verify `/info` returns correct information
- [ ] **Heartbeat test**: Validate `agent_info` is included in heartbeats
- [ ] **Health check test**: Verify health check retrieves and logs agent info
- [ ] **CI/CD test**: Validate `make agent-version-check` fails if version not updated
- [ ] **GitHub workflow test**: Validate `agent-version-bump` workflow works
- [ ] **Version mismatch test**: Simulate incorrect version and verify detection
- [ ] **Checksum mismatch test**: Simulate invalid checksum and verify bootstrap failure

### Storage Management
- [ ] **Volume discovery test**: Validate `list_attached_volumes` discovers all volumes
- [ ] **Creation test**: Verify volumes are tracked immediately after creation
- [ ] **Termination test**: Validate all volumes are deleted on termination
- [ ] **Boot volumes test**: Verify automatically created boot volumes are tracked
- [ ] **Persistent volumes test**: Validate `delete_on_terminate=false` preserves volumes
- [ ] **Deletion error test**: Simulate deletion error and verify logging
- [ ] **Local volumes test**: Validate detection and rejection of local volumes for L40S/L4
- [ ] **Recovery test**: Verify non-deleted volumes can be manually cleaned up

### State Machine
- [ ] **Transition test**: Validate each state transition (booting→ready, booting→startup_failed, etc.)
- [ ] **Idempotence test**: Verify transitions are idempotent
- [ ] **History test**: Validate `instance_state_history` records all transitions
- [ ] **Recovery test**: Verify automatic recovery (STARTUP_TIMEOUT → booting)
- [ ] **Specific error test**: Validate transitions to `startup_failed` with specific error codes

### Monitoring & Observability
- [ ] **Health check agent_info test**: Verify health check retrieves `/info`
- [ ] **Metadata test**: Validate `agent_info` is stored in `worker_metadata`
- [ ] **Logs test**: Verify agent metadata is included in health check logs
- [ ] **Problem detection test**: Simulate problems (incorrect version, invalid checksum) and verify detection
- [ ] **Rate limiting test**: Validate health check log rate limiting (5min success, 1min failure)

### Integration
- [ ] **Complete cycle test**: Provision Scaleway instance and validate:
  - Volume discovery
  - Agent checksum verification
  - 0-100% progress
  - Health checks with agent_info
  - Termination and volume deletion
- [ ] **Mock provider test**: Validate all features work with Mock
- [ ] **Multi-instance test**: Validate with multiple instances in parallel
- [ ] **Recovery test**: Validate recovery after errors (timeout, checksum mismatch, etc.)

### Documentation
- [ ] **README update**: Add references to new documents
- [ ] **Docs validation**: Verify all code examples work
- [ ] **User guide**: Create guide for using new features

---

## 🚀 Implementation plan (step-by-step, testable) — RBAC + org scoping

### Phase 1 — RBAC foundation (backend + tests) → commit
- **DB (migrations)**:
  - Normalize `organization_memberships.role` to: `owner|admin|manager|user`
  - Backfill: `member` → `user` (if present)
  - `CHECK` constraint + `DEFAULT 'user'`
- **Backend (Rust)**:
  - RBAC module (enum + helpers): org role, assignment rules (Owner/Admin/Manager), double activation (tech/eco)
  - Unit tests on RBAC matrix (without DB)
- **Tests**:
  - `cargo check -p inventiv-api`
  - `cargo test -p inventiv-api`

### Phase 2 — Roles associated with users (membership lifecycle) + tests → commit
- **API (org-scoped)**:
  - `GET /organizations/members`
  - `PUT /organizations/members/:user_id/role` (rules: Owner all; Manager ↔ User; Admin ↔ User)
  - `DELETE /organizations/members/:user_id` + "last Owner non-revocable" invariant
- **Audit logs**: log role changes and removals (immutable)
- **Tests**: last owner, forbidden escalations, etc.

### Phase 3 — Invitations + Users management + tests → commit
- **DB**: `organization_invitations` (email, token, expiry, role, invited_by, accepted_at)
- **API**:
  - `POST /organizations/invitations`
  - `GET /organizations/invitations`
  - `POST /organizations/invitations/:token/accept` (existing user or creation)
- **UI**: invite, view pending, accept (flow)

### Phase 4 — Org-scoped settings + double activation + tests → commit(s)
- Providers/regions/zones/types/models/settings scoped to org
- Double activation **per resource**:
  - Admin = tech only, Manager = eco only, Owner = both
  - UI: "non-operational" state + missing flag alert

### Phase 5 — Org-scoped instances + RBAC + tests → commit(s)
- Admin/Owner: ops (provision/terminate/reinstall/scheduling/scaling)
- Manager: finance gating + dashboards
- User: usage / read-only per policy

### Phase 6 — Models/Offerings + RBAC + tests → commit(s)
- Admin: technical config + publication
- Manager: pricing + economic activation + sharing
- Owner: all
