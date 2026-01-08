# Multi-Tenant Status - January 2025

**Last updated**: 2025-01-06  
**Context**: After major refactoring, addition of tests, SMTP email management, password reset, API code reorganization

---

## 📊 Overview

### ✅ What's Done (Foundations)

#### 1. Database
- ✅ **Tables created**:
  - `organizations` (id, name, slug, created_by_user_id)
  - `organization_memberships` (organization_id, user_id, role)
  - `organization_models` (pre-wiring for model sharing)
  - `organization_model_shares` (pre-wiring for provider→consumer contracts)
  - `workbench_projects` (projects with organization_id)
  - `workbench_runs` (sessions with organization_id, shared_with_org)
- ✅ **Enriched columns**:
  - `users.current_organization_id` (nullable) → **⚠️ TO MIGRATE to `user_sessions`**
  - `api_keys.organization_id` (nullable) → ready for scoping
  - `finops.inference_usage` (provider_organization_id, consumer_organization_id, etc.)
- ⏳ **PK/FK Migration**: `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql` created but not yet applied

#### 2. Backend API (Rust)

**Organizations Module** (`inventiv-api/src/organizations.rs`):
- ✅ `GET /organizations` - List user's orgs
- ✅ `POST /organizations` - Create org (with automatic owner)
- ✅ `PUT /organizations/current` - Change current workspace
- ✅ `GET /organizations/current/members` - List members with roles
- ✅ `PUT /organizations/current/members/:user_id` - Change role (RBAC)
- ✅ `DELETE /organizations/current/members/:user_id` - Remove member (last owner invariant)
- ✅ `POST /organizations/current/leave` - Leave org

**RBAC Module** (`inventiv-api/src/rbac.rs`):
- ✅ `OrgRole` enum: `Owner`, `Admin`, `Manager`, `User`
- ✅ Permission functions:
  - `can_invite(role)` → Owner/Admin/Manager
  - `can_set_activation_flag(role, flag)` → Owner (tech+eco), Admin (tech), Manager (eco)
  - `can_assign_role(actor, from, to)` → delegation rules
- ✅ RBAC unit tests

**Bootstrap Module** (`inventiv-api/src/bootstrap_admin.rs`):
- ✅ Automatic creation of "Inventiv IT" org with admin as owner
- ✅ Idempotent (can be re-executed)

**Auth & Sessions** (`inventiv-api/src/auth.rs`, `auth_endpoints.rs`):
- ✅ JWT contains `current_organization_id`
- ⏳ **MISSING**: `current_organization_role` in JWT
- ⏳ **MISSING**: `user_sessions` table for multi-sessions
- ⏳ **MISSING**: `session_id` in JWT

#### 3. Frontend (Next.js/React)

**Components**:
- ✅ `AccountSection.tsx` - Workspace switcher (Personal vs Org)
- ✅ `OrganizationMembersDialog.tsx` - Member + role management
- ✅ `WorkspaceBanner.tsx` - Current workspace display
- ✅ `Sidebar.tsx` - Navigation with workspace badge

**TypeScript Types**:
- ✅ `Organization`, `OrganizationMember` in `lib/types.ts`
- ✅ `Me` type with `current_organization_id`, `current_organization_name`, `current_organization_slug`
- ⏳ **MISSING**: `current_organization_role` in `Me`

---

## ❌ What's Missing (Next Steps)

### 🔴 Critical (Blocks Other Phases)

#### 1. Multi-Organization Session Architecture
**Problem**: `current_organization_id` is in `users` → only one "current" per user  
**Solution**: Implement `user_sessions` table (see `docs/syntheses/archives/SESSION_ARCHITECTURE_PROPOSAL.md`)

**To do**:
- [ ] Create `user_sessions` table with `session_id`, `current_organization_id`, `organization_role`
- [ ] Remove `current_organization_id` from `users`
- [ ] Enrich JWT with `session_id`, `current_organization_role`
- [ ] Modify `login()` to create session in DB
- [ ] Modify `set_current_organization()` to update session in DB
- [ ] Modify `require_user()` to validate session in DB
- [ ] Add endpoints `/auth/sessions` (list/revoke)

**Impact**: Enables multiple simultaneous sessions with different orgs

---

### 🟡 High Priority (Core Features)

#### 2. Scoping Instances by Organization
**Objective**: Isolate instances by `organization_id`

**To do**:
- [ ] SQL Migration: Add `instances.organization_id` (nullable for backward compat)
- [ ] Modify `list_instances()` → filter by `organization_id` if org workspace
- [ ] Modify `create_deployment()` → set `organization_id` if org workspace
- [ ] Modify `get_instance()`, `terminate_instance()`, `reinstall_instance()` → check RBAC access
- [ ] UI: Badge "Personal" vs "Org: <Name>" on instances
- [ ] UI: WorkspaceBanner visible on instances page
- [ ] Tests: Personal mode → legacy instances accessible
- [ ] Tests: Org mode → only org-owned instances
- [ ] RBAC Tests: User cannot terminate org instances

**Files**:
- SQL Migration (to create)
- `inventiv-api/src/handlers/deployments.rs`
- `inventiv-frontend/src/app/(app)/instances/page.tsx`

---

#### 3. Scoping Models by Organization
**Objective**: Isolate models by `organization_id` + public/private visibility

**To do**:
- [ ] SQL Migration: Add `models.organization_id` (nullable for backward compat)
- [ ] Modify `list_models()` → filter by `organization_id` + public models
- [ ] Modify `create_model()` → set `organization_id` if org workspace
- [ ] Modify `update_model()`, `delete_model()` → check RBAC
- [ ] UI: Badge "Public" vs "Org: <Name>" on models
- [ ] UI: Workspace filter on models page
- [ ] Tests: Personal mode → public models accessible
- [ ] Tests: Org mode → private org models + public
- [ ] RBAC Tests: User cannot modify org models

**Files**:
- SQL Migration (to create)
- `inventiv-api/src/handlers/models.rs` (if exists) or `main.rs`
- `inventiv-frontend/src/app/(app)/models/page.tsx`

---

#### 4. User Invitations
**Objective**: Allow inviting users by email to an organization

**To do**:
- [ ] SQL Migration: Create `organization_invitations` table
- [ ] API: `POST /organizations/current/invitations` (invite by email)
- [ ] API: `GET /organizations/current/invitations` (list pending/accepted/revoked)
- [ ] API: `DELETE /organizations/current/invitations/:id` (revoke)
- [ ] API: `POST /organizations/invitations/:token/accept` (public, can create user if non-existent)
- [ ] UI: "Invitations" section in `OrganizationMembersDialog`
- [ ] UI: Invite form (email + role)
- [ ] UI: Invitation list with status + actions
- [ ] Tests: Invite existing user → membership created
- [ ] Tests: Invite non-existent user → account created + membership
- [ ] RBAC Tests: Owner/Admin/Manager can invite

**Files**:
- SQL Migration (to create)
- `inventiv-api/src/organizations.rs` (invitation endpoints)
- `inventiv-frontend/src/components/account/OrganizationMembersDialog.tsx`

---

### 🟢 Medium Priority

#### 5. Scoping API Keys by Organization
**Objective**: Isolate API keys by `organization_id`

**To do**:
- [ ] Modify `list_api_keys()` → filter by `organization_id`
- [ ] Modify `create_api_key()` → set `organization_id` if org workspace
- [ ] Modify `update_api_key()`, `revoke_api_key()` → check RBAC
- [ ] UI: Badge "Personal" vs "Org: <Name>" on keys
- [ ] UI: Workspace filter
- [ ] Tests: Personal mode → user-owned keys
- [ ] Tests: Org mode → org-owned keys
- [ ] RBAC Tests: User cannot modify org keys

**Files**:
- `inventiv-api/src/handlers/api_keys.rs` (if exists) or `main.rs`
- `inventiv-frontend/src/app/(app)/api-keys/page.tsx`

---

#### 6. Scoping Users by Organization
**Objective**: Filter user list according to workspace

**To do**:
- [ ] Modify `list_users()` → filter org members if org workspace
- [ ] Modify `create_user()` → create automatic membership if org workspace
- [ ] Modify `update_user()`, `delete_user()` → check RBAC + last owner invariant
- [ ] UI: WorkspaceBanner visible
- [ ] UI: Filtered org member list
- [ ] Tests: Personal mode → see all users (admin)
- [ ] Tests: Org mode → see only members
- [ ] RBAC Tests: User cannot modify members

**Files**:
- `inventiv-api/src/handlers/users.rs` (if exists) or `main.rs`
- `inventiv-frontend/src/app/(app)/users/page.tsx`

---

#### 7. Scoping FinOps by Organization
**Objective**: Filter financial dashboards according to workspace

**To do**:
- [ ] Modify `get_cost_current()` → filter by `organization_id`
- [ ] Modify `get_costs_dashboard_*()` → filter by `organization_id`
- [ ] UI: WorkspaceBanner visible
- [ ] UI: Dashboards filtered by workspace
- [ ] Tests: Personal mode → user costs
- [ ] Tests: Org mode → org costs

**Files**:
- `inventiv-api/src/handlers/finops.rs` (if exists) or `main.rs`
- `inventiv-frontend/src/app/(app)/(dashboard)/page.tsx`

---

### 🔵 Low Priority (Nice-to-Have)

#### 8. Double Activation (Tech/Eco)
**Objective**: Technical activation (Admin) + Economic activation (Manager) per resource

**To do**:
- [ ] Add columns `tech_activated_by`, `eco_activated_by` on resources (instances, models, etc.)
- [ ] Modify endpoints to check double activation
- [ ] UI: "Non-operational" state + missing flag alert
- [ ] Tests: Resource non-operational if one flag missing

---

#### 9. Model Sharing & Billing
**Objective**: Share models between orgs with token-based billing

**To do**:
- [ ] CRUD `organization_models` (publish/unpublish)
- [ ] CRUD `organization_model_shares` (grant/pause/revoke + pricing)
- [ ] Resolve `org_slug/model_code` in OpenAI proxy
- [ ] Ingest `finops.inference_usage` with chargeback
- [ ] Consumption dashboards by org/provider/consumer

---

#### 10. Frontend Module Migration
**Objective**: Hide/show modules according to workspace + role

**To do**:
- [ ] Identify modules "For All Users" vs "Admin Only" vs "Org Required"
- [ ] Modify `layout.tsx` → check `current_organization_id` for admin modules
- [ ] Modify `Sidebar.tsx` → hide links according to workspace + org role
- [ ] Badge "Org required" on admin links
- [ ] Redirect to org creation if necessary

---

## 📋 Recommended Action Plan

### Immediate Phase (Sprint 1)

1. **Session Architecture** (Critical)
   - Create `user_sessions` table
   - Migrate `current_organization_id` to sessions
   - Enrich JWT with `session_id` + `current_organization_role`
   - Tests: Multi-sessions with different orgs

2. **PK/FK Migration** (Foundation)
   - Apply migration `20260106000000_add_multi_tenant_primary_keys_and_foreign_keys.sql`
   - Verify constraints on test DB
   - Deploy to staging

### Short Term (Sprint 2-3)

3. **Scoping Instances** (Core Feature)
   - SQL Migration + API + UI + Tests

4. **Scoping Models** (Core Feature)
   - SQL Migration + API + UI + Tests

5. **Invitations** (Onboarding)
   - SQL Migration + API + UI + Tests

### Medium Term (Sprint 4-6)

6. **Scoping API Keys**
7. **Scoping Users**
8. **Scoping FinOps**
9. **Frontend Module Migration**

### Long Term (Sprint 7+)

10. **Double Activation**
11. **Model Sharing & Billing**

---

## 🔍 Points of Attention

### 1. Backward Compatibility
- ✅ Personal mode (`current_organization_id = NULL`) must always work
- ✅ Legacy resources (`organization_id = NULL`) remain accessible
- ✅ New features are opt-in (org workspace = optional)

### 2. Performance
- ⚠️ Add `current_organization_role` in JWT to avoid additional DB queries
- ⚠️ Index on `(organization_id, user_id)` for `organization_memberships`
- ⚠️ Index on `organization_id` for all scoped tables

### 3. Security
- ⚠️ RBAC checked at each business endpoint
- ⚠️ Immutable audit logs for role/member changes
- ⚠️ "Last owner" invariant non-revocable

### 4. Tests
- ⚠️ API unit tests (Rust)
- ⚠️ Frontend manual tests (Personal mode + Org mode)
- ⚠️ RBAC tests (Owner/Admin/Manager/User roles)
- ⚠️ Backward compatibility tests (Personal mode)

---

## 📚 Existing Documentation

- `docs/syntheses/MULTI_TENANT_MIGRATION_PLAN.md` - Detailed plan by phase
- `docs/syntheses/MULTI_TENANT_MIGRATION_TRACKER.md` - Visual phase tracker
- `docs/syntheses/RBAC_ANALYSIS.md` - Detailed RBAC analysis
- `docs/syntheses/archives/SESSION_ARCHITECTURE_PROPOSAL.md` - Session architecture proposal
- `docs/syntheses/archives/SESSION_AUTH_ANALYSIS.md` - Current session/auth analysis
- `docs/syntheses/MULTI_TENANT_ROADMAP.md` - Target roadmap (users first-class + org workspaces)
- `docs/syntheses/MULTI_TENANT_MODEL_SHARING_BILLING.md` - Model sharing + billing design

---

## 🎯 Goals for Next Steps

1. **Complete isolation**: Instances, Models, API Keys, Users, FinOps scoped by organization
2. **Complete RBAC**: Permissions according to org role (Owner/Admin/Manager/User)
3. **Conditional visibility**: Modules/functions displayed according to workspace + role
4. **Multi-sessions**: Multiple simultaneous sessions with different orgs
5. **Smooth onboarding**: Email invitations with automatic account creation

---

**Next step**: Validate this document and start with session architecture (Immediate Phase).
