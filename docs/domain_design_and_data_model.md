# Domain Design & Data Structures (DDD)

**Last updated**: 2025-01-XX  
**Vision**: Multi-tenant with Users first-class + Organizations + RBAC + Double Activation

---

## 1. Ubiquitous Language

### Infrastructure & Compute
*   **Provider**: An infrastructure provider (e.g., Scaleway, AWS, Mock).
*   **Instance (Node)**: A virtual machine or bare-metal provided by a Provider. It has an IP and GPU resources.
*   **Worker**: The process (Container) that runs on an Instance to serve models.
*   **Model**: A specific LLM model (e.g., `llama-3-70b-instruct`) with technical prerequisites.
*   **Deployment**: The association of a Model on an Instance.

### Multi-Tenant & Workspace
*   **Workspace**: The active context of a user (Personal or Organization).
  *   **Personal**: User mode without organization (`current_organization_id = NULL`)
  *   **Organization**: User mode with organization (`current_organization_id != NULL`)
*   **Session**: A user session with a specific workspace (can have multiple simultaneous sessions with different workspaces).

### Account & Subscription Plans
*   **Account Plan (User)**: User subscription plan (`free` | `subscriber`).
  *   **Free**: Free account (`account_plan = 'free'`)
  *   **Subscriber**: Subscribed account (`account_plan = 'subscriber'`)
*   **Subscription Plan (Organization)**: Organization subscription plan (`free` | `subscriber`).
  *   **Free**: Free organization (`subscription_plan = 'free'`)
  *   **Subscriber**: Subscribed organization (`subscription_plan = 'subscriber'`)

**Important rule**: The plan applies according to the **active workspace (session)**:
- Personal Session → `users.account_plan` applies
- Organization A Session → `organizations.subscription_plan` (org A) applies
- Organization B Session → `organizations.subscription_plan` (org B) applies
- If workspace switches, the plan changes immediately

### Wallet & Billing
*   **Wallet User**: Personal token balance (`users.wallet_balance_eur`).
*   **Wallet Organization**: Organization token balance (`organizations.wallet_balance_eur`).

**Important rule**: The wallet used depends on the **active workspace (session)**:
- Personal Session → debit from `users.wallet_balance_eur`
- Organization A Session → debit from `organizations.wallet_balance_eur` (org A)
- Organization B Session → debit from `organizations.wallet_balance_eur` (org B)

### Organization Roles (RBAC)
*   **Owner**: Owner (`organization_role = 'owner'`) - Can do everything, must do double activation explicitly.
*   **Admin**: Technical administrator (`organization_role = 'admin'`) - Manages infrastructure, instances, models, can activate tech only.
*   **Manager**: Financial manager (`organization_role = 'manager'`) - Manages finances, prices, authorizations, can activate eco only.
*   **User**: User (`organization_role = 'user'`) - Uses resources, no administration permissions.

### Model Visibility & Access
*   **Visibility**: Who can *see* the offering (`public` | `unlisted` | `private`).
  *   **Public**: Visible to all (`visibility = 'public'`)
  *   **Unlisted**: Not listed but accessible if authorized (`visibility = 'unlisted'`)
  *   **Private**: Visible only to org members (`visibility = 'private'`)
*   **Access Policy**: Under what conditions one can *use* the offering (`free` | `subscription_required` | `request_required` | `pay_per_token` | `trial`).
  *   **Free**: Free usage (`access_policy = 'free'`)
  *   **Subscription Required**: Reserved for subscribers (`access_policy = 'subscription_required'`)
  *   **Request Required**: Access request required (`access_policy = 'request_required'`)
  *   **Pay Per Token**: Token-based billing (`access_policy = 'pay_per_token'`)
  *   **Trial**: Free until date/quota (`access_policy = 'trial'`)

### Double Activation
*   **Tech Activation**: Technical activation (`tech_activated_by`, `tech_activated_at`) - Admin/Owner only.
*   **Eco Activation**: Economic activation (`eco_activated_by`, `eco_activated_at`) - Manager/Owner only.
*   **Operational**: Operational resource (`is_operational = true`) - Requires both activations.

**Important rule**: Even if Owner has both roles (Admin + Manager), they must do double activation explicitly. This is a governance rule to avoid errors.

## 2. Domain Entities (Rust Structs)

These structures will be defined in `inventiv-common`.

### A. Core Entities

#### `LlmModel` (Aggregate Root)
Defines a model available in the catalog.
```rust
pub struct LlmModel {
    pub id: Uuid,
    pub name: String,             // e.g., "Llama 3 70B"
    pub model_id: String,         // e.g., "meta-llama/Meta-Llama-3-70B-Instruct" (HuggingFace ID)
    pub updated_at: DateTime<Utc>,
    // Hardware Requirements (Value Object)
    pub required_vram_gb: i32,
    pub context_length: i32,
}
```

#### `Instance` (Entity)
Represents a provisioned compute resource.
```rust
pub struct Instance {
    pub id: Uuid,
    pub provider_id: String,      // ID côté Cloud Provider (ex: i-123456)
    pub provider_name: String,    // "scaleway", "aws"
    pub ip_address: Option<String>,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    // Hardware Specs (Value Object)
    pub gpu_profile: GPUProfile,  // e.g. { name: "H100", vram: 80 }
}
```

#### `InstanceStatus` (Enum/State Machine)
Rigorous lifecycle with explicit transitions.

**Main states**:
*   `Provisioning`: Requested from provider, pending.
*   `Booting`: Instance being created, not yet started.
*   `Installing`: Instance up, but Worker being installed.
*   `Starting`: Instance up and running, but Worker still finalizing (model download, warming, etc.).
*   `Ready`: Worker ready to receive traffic (Healthcheck OK).
*   `Draining`: Shutting down, no longer accepting new requests.
*   `Terminating`: Being deleted at provider.
*   `Terminated`: Destroyed at provider.
*   `Archived`: Archived (removed from active view).

**Error/recovery states**:
*   `Unavailable`: Instance inaccessible or unavailable, to reconnect and diagnose to return to Ready or decommission.
*   `ProvisioningFailed`: Failure during instance creation at provider.
*   `StartupFailed`: Failure during startup or worker configuration.
*   `Failed`: Generic failure state.

**Transitions**: Managed by explicit functions in `inventiv-orchestrator/src/state_machine.rs`.

> **See**: [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) for complete details on transitions, history, and progress tracking.

## 3. Storage Strategy

We separate "Cold Storage" (Configuration/History) from "Hot Storage" (Real-time Routing).

### A. PostgreSQL (System of Record - Orchestrator)
Management of ground truth and history.

```sql
-- users (multi-tenant: users first-class)
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    username TEXT NOT NULL,
    role VARCHAR(50) DEFAULT 'admin',  -- User global role (admin|user)
    account_plan TEXT DEFAULT 'free' NOT NULL,  -- Account plan (free|subscriber)
    account_plan_updated_at TIMESTAMPTZ,
    wallet_balance_eur NUMERIC(10,2) DEFAULT 0 NOT NULL,  -- Personal wallet
    first_name TEXT,
    last_name TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT users_account_plan_check CHECK (account_plan IN ('free', 'subscriber'))
);

-- organizations (multi-tenant workspaces)
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    subscription_plan TEXT DEFAULT 'free' NOT NULL,  -- Subscription plan (free|subscriber)
    subscription_plan_updated_at TIMESTAMPTZ,
    wallet_balance_eur NUMERIC(10,2) DEFAULT 0 NOT NULL,  -- Organization wallet
    sidebar_color TEXT,  -- Configurable sidebar color (anti-error UX)
    created_by_user_id UUID REFERENCES users(id),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT organizations_subscription_plan_check CHECK (subscription_plan IN ('free', 'subscriber'))
);

-- organization_memberships (RBAC)
CREATE TABLE organization_memberships (
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'user',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (organization_id, user_id),
    CONSTRAINT organization_memberships_role_check CHECK (role IN ('owner', 'admin', 'manager', 'user'))
);

-- user_sessions (multi-sessions with workspace)
CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    current_organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,
    organization_role TEXT CHECK (organization_role IN ('owner', 'admin', 'manager', 'user')),
    session_token_hash TEXT NOT NULL,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);

-- providers (catalog)
CREATE TABLE providers (
    id UUID PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL, -- "scaleway", "aws"
    description TEXT
);

-- regions
CREATE TABLE regions (
    id UUID PRIMARY KEY,
    provider_id UUID REFERENCES providers(id),
    name VARCHAR(50) NOT NULL, -- "fr-par", "us-east-1"
    UNIQUE(provider_id, name)
);

-- zones
CREATE TABLE zones (
    id UUID PRIMARY KEY,
    region_id UUID REFERENCES regions(id),
    name VARCHAR(50) NOT NULL, -- "fr-par-1"
    UNIQUE(region_id, name)
);

-- instance_types (catalog capabilities)
CREATE TABLE instance_types (
    id UUID PRIMARY KEY,
    provider_id UUID REFERENCES providers(id),
    name VARCHAR(50) NOT NULL, -- "H100-1-80G"
    gpu_count INT NOT NULL,
    vram_per_gpu_gb INT NOT NULL,
    UNIQUE(provider_id, name)
);

-- instance_availability (linking types to zones)
CREATE TABLE instance_availability (
    instance_type_id UUID REFERENCES instance_types(id),
    zone_id UUID REFERENCES zones(id),
    PRIMARY KEY(instance_type_id, zone_id)
);

-- ssh_keys (per provider)
CREATE TABLE ssh_keys (
    id UUID PRIMARY KEY,
    name VARCHAR(50) NOT NULL,
    public_key TEXT NOT NULL,
    provider_id UUID REFERENCES providers(id),
    provider_key_id VARCHAR(255), -- Remote ID at provider
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- models
CREATE TABLE models (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    model_id VARCHAR(255) UNIQUE NOT NULL, -- "llama-3-8b"
    required_vram_gb INT NOT NULL,
    is_active BOOLEAN DEFAULT true
);

-- instances (org-scoped with double activation)
CREATE TABLE instances (
    id UUID PRIMARY KEY,
    provider_id UUID REFERENCES providers(id),
    zone_id UUID REFERENCES zones(id),
    instance_type_id UUID REFERENCES instance_types(id),
    organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,  -- Org-scoped
    
    provider_instance_id VARCHAR(255),  -- Remote ID
    ip_address INET,
    
    api_key VARCHAR(255), -- Key to call the worker securely
    
    status VARCHAR(50) NOT NULL,
    
    -- Double activation (tech + eco)
    tech_activated_by UUID REFERENCES users(id),
    tech_activated_at TIMESTAMPTZ,
    eco_activated_by UUID REFERENCES users(id),
    eco_activated_at TIMESTAMPTZ,
    is_operational BOOLEAN GENERATED ALWAYS AS (
        tech_activated_by IS NOT NULL AND eco_activated_by IS NOT NULL
    ) STORED,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    terminated_at TIMESTAMPTZ,
    gpu_profile JSONB NOT NULL -- Specs snapshot
);

-- models (org-scoped with double activation)
CREATE TABLE models (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    model_id VARCHAR(255) UNIQUE NOT NULL, -- "llama-3-8b"
    required_vram_gb INT NOT NULL,
    organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,  -- Org-scoped (NULL = public)
    
    -- Double activation (tech + eco)
    tech_activated_by UUID REFERENCES users(id),
    tech_activated_at TIMESTAMPTZ,
    eco_activated_by UUID REFERENCES users(id),
    eco_activated_at TIMESTAMPTZ,
    is_operational BOOLEAN GENERATED ALWAYS AS (
        tech_activated_by IS NOT NULL AND eco_activated_by IS NOT NULL
    ) STORED,
    
    is_active BOOLEAN DEFAULT true
);

-- organization_models (offerings published by orgs)
CREATE TABLE organization_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    model_id UUID NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    code TEXT NOT NULL,  -- Org-scoped identifier (e.g., "sales-bot")
    visibility TEXT NOT NULL DEFAULT 'private',  -- public|unlisted|private
    access_policy TEXT NOT NULL DEFAULT 'free',  -- free|subscription_required|request_required|pay_per_token|trial
    is_active BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(organization_id, code),
    CONSTRAINT organization_models_visibility_check CHECK (visibility IN ('public', 'unlisted', 'private')),
    CONSTRAINT organization_models_access_policy_check CHECK (access_policy IN ('free', 'subscription_required', 'request_required', 'pay_per_token', 'trial'))
);

-- organization_model_shares (provider→consumer contracts)
CREATE TABLE organization_model_shares (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    consumer_organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    organization_model_id UUID NOT NULL REFERENCES organization_models(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'active',  -- active|paused|revoked
    pricing JSONB NOT NULL,  -- { "version": 1, "type": "per_1k_tokens", "eur_per_1k": 0.2 }
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT organization_model_shares_distinct_orgs CHECK (provider_organization_id <> consumer_organization_id),
    CONSTRAINT organization_model_shares_status_check CHECK (status IN ('active', 'paused', 'revoked'))
);

-- api_keys (user-owned ou org-owned)
CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,  -- NULL = user-owned, NOT NULL = org-owned
    name TEXT NOT NULL,
    key_prefix TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    scopes JSONB,  -- Restrictions (allowlist offerings, max spend, expiry)
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);
```

### B. Redis (Real-time Discovery - Router & Orchestrator)
The Router must read this data in < 1ms without touching Postgres.

#### Pattern: Set & Hash
*   **Discovery Set**: List of active instances for a given model.
    *   Key: `catalog:model:{model_id}:instances` (Set)
    *   Value: `{instance_id}`

*   **Instance State**: Technical details for routing.
    *   Key: `instance:{instance_id}` (Hash)
    *   Fields:
        *   `ip`: "192.168.1.10"
        *   `port`: "8000"
        *   `status`: "READY"
        *   `current_load`: "12" (Number of active requests - updated by router/worker)
        *   `last_heartbeat`: Timestamp (for automatic expiration)

## 4. CQRS & Event-Driven Architecture (v0.3.2)
The architecture has evolved to strictly separate responsibilities (CQRS) and isolate the Orchestrator.

### A. Data Flow
*   **Frontend**: Never communicates with the Orchestrator. It only talks to the Backend (Gateway).
*   **Backend (Product Plane)**:
    *   **Read Model**: Reads directly from DB (Postgres) for queries (GET /instances).
    *   **Write Model**: Validates requests and publishes **Commands** in Redis (`orchestrator_events`).
*   **Orchestrator (Control Plane)**:
    *   Acts as a **Worker**.
    *   Listens to Redis events (`CMD:PROVISION`, `CMD:TERMINATE`).
    *   Executes IaaS operations (Scaleway, AWS).
    *   Updates "Ground Truth" in Postgres.

### B. API Contracts & Documentation
The Backend exposes a documented API via **Swagger/OpenAPI**.
*   Local URL: `http://localhost:8003/swagger-ui`
*   JSON Spec: `http://localhost:8003/api-docs/openapi.json`

### C. Workflows

#### 4.1. Provisioning (Command)
1.  **User**: `POST /deployments` (Backend).
2.  **Backend**: Publishes `CMD:PROVISION` in Redis. Returns `200 Accepted`.
3.  **Orchestrator**: Receives `CMD:PROVISION`. Creates instance (Scaleway).
4.  **Orchestrator**: INSERT `instances` (Status: Booting) -> DB.
5.  **Frontend**: Polling `GET /instances` -> Sees "Booting".

#### 4.2. Termination (Command)
1.  **User**: `DELETE /instances/:id` (Backend).
2.  **Backend**: Publishes `CMD:TERMINATE` in Redis.
3.  **Orchestrator**: Receives `CMD:TERMINATE`. Deletes instance (Scaleway).
4.  **Orchestrator**: UPDATE `instances` SET status='Terminated' -> DB.

#### 4.3. Monitoring (Query)
1.  **User**: Dashboard (Frontend).
2.  **Frontend**: `GET /api/backend/instances`.
3.  **Backend**: `SELECT * FROM instances WHERE organization_id = $1` (Postgres) - Filtered by workspace.

---

## 5. Multi-Tenant Data Model (Target Vision)

### 5.1 Workspace Scoping

**Fundamental rule**: The **active workspace (session)** determines the context of all business operations.

**Examples**:
- `GET /instances` → Filter by `organization_id = current_organization_id` if org workspace
- `POST /deployments` → Create instance with `organization_id = current_organization_id` if org workspace
- `GET /models` → Filter by `organization_id = current_organization_id` OR `organization_id IS NULL` (public)
- `GET /finops/cost/current` → Filter by `organization_id = current_organization_id` if org workspace

### 5.2 Plan & Wallet by Workspace

**Plan**:
- Personal Session → `users.account_plan` determines accessible models
- Org A Session → `organizations.subscription_plan` (org A) determines accessible models
- Org B Session → `organizations.subscription_plan` (org B) determines accessible models

**Wallet**:
- Personal Session → Debit from `users.wallet_balance_eur`
- Org A Session → Debit from `organizations.wallet_balance_eur` (org A)
- Org B Session → Debit from `organizations.wallet_balance_eur` (org B)

### 5.3 RBAC by Organization Role

**Permissions by role** (see `docs/syntheses/RBAC_ANALYSIS.md` for details):
- **Owner**: All permissions (but must do double activation explicitly)
- **Admin**: Technical management (instances, models, infrastructure, tech activation)
- **Manager**: Financial management (prices, authorizations, dashboards, eco activation)
- **User**: Resource usage (read-only on instances/models)

### 5.4 Double Activation (Tech + Eco)

**Rule**: A resource (instance, model, API key, etc.) is **operational** only if:
- `tech_activated_by IS NOT NULL` (technical activation by Admin/Owner)
- `eco_activated_by IS NOT NULL` (economic activation by Manager/Owner)

**Permissions**:
- Owner can activate tech + eco (but must do both activations explicitly)
- Admin can activate tech only
- Manager can activate eco only
- User cannot activate anything

**UX**: If a resource is not operational, display "non-operational" state + alert indicating missing flag.

### 5.5 Model Visibility & Access Policy

**Visibility**:
- `public`: Visible to all users (platform)
- `unlisted`: Not listed but accessible via direct identifier if authorized
- `private`: Visible only to provider org members

**Access Policy**:
- `free`: Free usage
- `subscription_required`: Reserved for subscribers (org or user plan according to workspace)
- `request_required`: Access request + approval required
- `pay_per_token`: Token-based billing (debit from wallet according to workspace)
- `trial`: Free until date/quota

**Resolution**:
- Accessible models = Union of:
  - Org models (`organization_id = current_organization_id`) if org workspace
  - Public models (`organization_id IS NULL`) according to workspace plan
  - Shared models (`organization_model_shares` active) if org workspace

### 5.6 Index & Performance

**Recommended indexes**:
```sql
-- Performance workspace scoping
CREATE INDEX idx_instances_org ON instances(organization_id) WHERE organization_id IS NOT NULL;
CREATE INDEX idx_models_org ON models(organization_id) WHERE organization_id IS NOT NULL;
CREATE INDEX idx_api_keys_org ON api_keys(organization_id) WHERE organization_id IS NOT NULL;

-- Performance RBAC
CREATE INDEX idx_organization_memberships_org_role ON organization_memberships(organization_id, role);
CREATE INDEX idx_organization_memberships_user ON organization_memberships(user_id, organization_id);

-- Performance double activation
CREATE INDEX idx_instances_operational ON instances(organization_id, is_operational) WHERE is_operational = true;
CREATE INDEX idx_models_operational ON models(organization_id, is_operational) WHERE is_operational = true;
```

---

## 6. Migration Strategy

**Approach**: Clean model from the start (no legacy).

**SQL Migrations**:
1. Enrich `users` with `account_plan`, `wallet_balance_eur`
2. Enrich `organizations` with `subscription_plan`, `wallet_balance_eur`, `sidebar_color`
3. Add `organization_id` to scoped tables (`instances`, `models`, `api_keys`, etc.)
4. Add double activation columns (`tech_activated_by`, `eco_activated_by`, `is_operational`)
5. Create performance indexes

**Seed data**:
- Default admin user (`account_plan = 'free'` by default)
- Default organization "Inventiv IT" (`subscription_plan = 'free'` by default)
- Admin user = Owner of "Inventiv IT"

---

## 7. Consistency Rules

1. **Workspace = Scope**: All business operations are scoped according to active workspace
2. **Plan by Workspace**: The plan (user or org) applies according to active workspace
3. **Wallet by Workspace**: The wallet (user or org) applies according to active workspace
4. **Double Activation**: Owner must do both activations explicitly (even if they have both roles)
5. **No Legacy**: Clean model from the start, no legacy data migration
6. **Users First-Class**: A user without org remains "first-class" and can use the platform
7. **Multi-Sessions**: A user can have multiple simultaneous sessions with different workspaces
