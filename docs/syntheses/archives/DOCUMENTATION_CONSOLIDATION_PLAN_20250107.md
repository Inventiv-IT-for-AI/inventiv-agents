# Documentation Consolidation Plan

**Date**: 2025-01-07  
**Goal**: Unify, consolidate, and reorganize documentation

## Current State

**Total files in docs/**: 67 files

## Analysis by Category

### 1. Core Documentation (Keep & Maintain - Must be in English)

#### Essential Entry Points
- ✅ `README.md` - Already in English
- ✅ `TODO.md` - Mixed French/English (needs translation)
- ✅ `docs/architecture.md` - Already in English
- ✅ `docs/domain_design.md` - Already in English
- ⚠️ `docs/specification_generale.md` - **FRENCH** (needs translation to `specification.md`)
- ✅ `docs/ui_design_system.md` - French (needs translation)
- ✅ `docs/ia_widgets.md` - Already in English
- ✅ `docs/engineering_guidelines.md` - Already in English
- ✅ `docs/CONTRIBUTING.md` - Already in English
- ✅ `docs/DEVELOPMENT_SETUP.md` - Already in English

### 2. Feature Documentation (Keep - Translate if needed)

- ✅ `docs/AGENT_VERSION_MANAGEMENT.md`
- ✅ `docs/STORAGE_MANAGEMENT.md`
- ✅ `docs/STATE_MACHINE_AND_PROGRESS.md`
- ✅ `docs/SCALEWAY_PROVISIONING.md`
- ✅ `docs/CI_CD.md`
- ✅ `docs/API_URL_CONFIGURATION.md`
- ✅ `docs/INVENTIV_DATA_TABLE.md`
- ✅ `docs/PLATFORM_COMPATIBILITY.md`
- ✅ `docs/ACTION_TYPES_FORMAT.md`
- ✅ `docs/CHAT_SESSIONS_AND_INFERENCE.md`
- ✅ `docs/DATA_VOLUME_RECOMMENDATION_SYNC.md`
- ✅ `docs/DEPLOIEMENT_STAGING.md` (needs translation)
- ✅ `docs/ENDPOINTS_INVENTORY.md`
- ✅ `docs/worker_and_router_phase_0_2.md`
- ✅ `docs/INSTANCE_TYPE_ZONES_COMPLETE.md`
- ✅ `docs/INSTANCE_TYPE_ZONES_IMPLEMENTATION.md`
- ✅ `docs/MOCK_REAL_VLLM_IMPLEMENTATION.md`
- ✅ `docs/MOCK_REAL_VLLM_USAGE.md`
- ✅ `docs/VLLM_IMAGE_SELECTION_ARCHITECTURE.md`
- ✅ `docs/TEST_PLAN_CHAT_SESSIONS.md`
- ✅ `docs/TEST_PLAN_STORAGE_MANAGEMENT.md`

### 3. Multi-Tenant Documentation (Keep Active Plans)

- ✅ `docs/MULTI_TENANT_ROADMAP.md`
- ✅ `docs/MULTI_TENANT_MODEL_SHARING_BILLING.md`
- ✅ `docs/MULTI_TENANT_MIGRATION_PLAN.md`
- ✅ `docs/MULTI_TENANT_MIGRATION_TRACKER.md`
- ✅ `docs/RBAC_ANALYSIS.md`
- ✅ `docs/MULTI_TENANT_NEXT_STEPS.md` (active plan)
- ✅ `docs/MULTI_TENANT_STATUS_2025.md` (status doc)

### 4. Session Documentation (Move to syntheses/archives)

**Reason**: Phase 1 completed, these are historical records

- 📦 `docs/SESSION_ARCHITECTURE_PROPOSAL.md` → `docs/syntheses/archives/`
- 📦 `docs/SESSION_AUTH_ANALYSIS.md` → `docs/syntheses/archives/`
- 📦 `docs/SESSION_IMPLEMENTATION_STATUS.md` → `docs/syntheses/archives/`
- 📦 `docs/PHASE1_SESSIONS_STATUS.md` → `docs/syntheses/archives/`
- 📦 `docs/PHASE1_SESSIONS_TESTS.md` → `docs/syntheses/archives/`
- 📦 `docs/PHASE1_TEST_COVERAGE_ANALYSIS.md` → `docs/syntheses/archives/`
- 📦 `docs/PHASE1_TESTS_SUMMARY.md` → `docs/syntheses/archives/`

### 5. Session Close Summaries (Move to syntheses/archives)

**Reason**: Historical session summaries

- 📦 `docs/SESSION_CLOSE_20260105.md` → `docs/syntheses/archives/`
- 📦 `docs/SESSION_CLOSE_20260107.md` → `docs/syntheses/archives/`
- 📦 `docs/SESSION_CLOSE_20260108.md` → `docs/syntheses/archives/`
- 📦 `docs/SESSION_INIT_SUMMARY.md` → `docs/syntheses/archives/`
- 📦 `docs/SESSION_SUMMARY_CHAT_INFERENCE.md` → `docs/syntheses/archives/`
- 📦 `docs/ETAT_DES_LIEUX_20260107.md` → `docs/syntheses/archives/`

### 6. Analysis Documents (Move to syntheses/archives)

**Reason**: Historical analysis, already implemented or obsolete

- 📦 `docs/ANALYSE_LOGS_INSTANCE_FAILED.md` → `docs/syntheses/archives/`
- 📦 `docs/ANALYSE_MODULARISATION_MAIN_RS.md` → `docs/syntheses/archives/`
- 📦 `docs/ARCHITECTURE_COMPREHENSION.md` → `docs/syntheses/archives/`
- 📦 `docs/ARCHITECTURE_COMPREHENSION_SESSION.md` → `docs/syntheses/archives/`
- 📦 `docs/OBSERVABILITY_ANALYSIS.md` → `docs/syntheses/archives/`
- 📦 `docs/OBSERVABILITY_TEST_REPORT.md` → `docs/syntheses/archives/`
- 📦 `docs/WORKER_RELIABILITY_ANALYSIS.md` → `docs/syntheses/archives/`

### 7. Consolidation Plans (Move to syntheses/archives)

**Reason**: Historical consolidation plans

- 📦 `docs/DOCUMENTATION_CONSOLIDATION_PLAN.md` → `docs/syntheses/archives/`
- 📦 `docs/DOCUMENTATION_CONSOLIDATION_SUMMARY.md` → `docs/syntheses/archives/`
- 📦 `docs/MIGRATION_CONSOLIDATION_PLAN.md` → `docs/syntheses/archives/`

### 8. Implementation Guides (Keep or Archive)

**Keep** (reference documentation):
- ✅ `docs/INSTANCE_TYPE_ZONES_COMPLETE.md`
- ✅ `docs/INSTANCE_TYPE_ZONES_IMPLEMENTATION.md`
- ✅ `docs/MOCK_REAL_VLLM_IMPLEMENTATION.md`
- ✅ `docs/MOCK_REAL_VLLM_USAGE.md`
- ✅ `docs/VLLM_IMAGE_SELECTION_ARCHITECTURE.md`

**Archive** (obsolete proposals):
- 📦 `docs/INSTANCE_TYPE_FILTERING_PROPOSAL.md` → `docs/syntheses/archives/`
- 📦 `docs/MOCK_REAL_LLM_PROPOSAL.md` → `docs/syntheses/archives/`
- 📦 `docs/STRUCTURE_MODULAIRE_PROPOSEE.md` → `docs/syntheses/archives/` (already implemented)

### 9. Other Documents (Review)

**Keep** (reference/feature docs):
- ✅ `docs/FLUX_ARCHITECTURE_MAP.md` (keep as reference)
- ✅ `docs/MONITORING_IMPROVEMENTS.md` (keep as reference)

**Archive** (fixed/obsolete):
- 📦 `docs/FRONTEND_401_REDIRECT.md` → `docs/syntheses/archives/` (fixed)
- 📦 `docs/VERIFICATION_CI_CD.md` → `docs/syntheses/archives/` (completed)
- 📦 `docs/VOLUME_HISTORY_ENHANCEMENT.md` → `docs/syntheses/archives/` (completed)

## Action Plan

### Step 1: Create Directory Structure
- ✅ Create `docs/syntheses/archives/`
- ✅ Create `docs/tmp/`

### Step 2: Move Historical Documents
Move all documents marked with 📦 to `docs/syntheses/archives/`

### Step 3: Translate French Documents
Translate to English:
1. `docs/specification_generale.md` → `docs/specification.md`
2. `docs/ui_design_system.md` → Translate in place
3. `docs/DEPLOIEMENT_STAGING.md` → `docs/DEPLOYMENT_STAGING.md`
4. `docs/TODO.md` → Translate mixed content

### Step 4: Update README.md
Remove references to archived documents, keep only active documentation links

### Step 5: Create Documentation Index
Create `docs/README.md` with clear structure and navigation

## Summary

**Files to keep**: ~35 files  
**Files to archive**: ~32 files  
**Files to translate**: ~4 files

