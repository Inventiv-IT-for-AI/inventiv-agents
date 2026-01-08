# Documentation Consolidation Plan - January 15, 2025

## Objective
Unify, reorganize, and clean up documentation to reduce volume, eliminate duplicates, inconsistencies, and obsolete content, keeping only what is useful, coherent, and up-to-date.

## Rules

### Main Entry Points (Must Always Be Up-to-Date)
- `README.md` - Project overview and quickstart
- `TODO.md` - Roadmap and backlog
- `docs/project_requirements.md` - General specifications (with subsections)
- `docs/architecture.md` - Technical architecture
- `docs/domain_design_and_data_model.md` - Domain design and data structures
- `docs/ui_design_system.md` - UI/UX guidelines and frontend info
- `docs/testing.md` - Testing strategies and guidelines

### Synthesis Documents
- **Active plans** (to implement or in progress) → `docs/syntheses/`
- **Completed/obsolete plans** → `docs/syntheses/archives/`

### Temporary Documents
- **Work-in-progress** → `docs/tmp/` (dated, to be deleted after session)

### Language
- **All documentation must be in English** (translate French documents)

---

## Current State Analysis

### Main Entry Points Status

| File | Status | Action Needed |
|------|--------|---------------|
| `README.md` | ✅ Up-to-date | Minor updates for multi-tenant |
| `TODO.md` | ✅ Up-to-date | Update multi-tenant section |
| `docs/project_requirements.md` | ⚠️ Needs update | Consolidate with architecture.md |
| `docs/architecture.md` | ✅ Up-to-date | Minor updates |
| `docs/domain_design_and_data_model.md` | ✅ Up-to-date | Minor updates |
| `docs/ui_design_system.md` | ✅ Up-to-date | No changes |
| `docs/testing.md` | ✅ Up-to-date | No changes |

### Synthesis Documents Analysis

#### Active Plans (Keep in `docs/syntheses/`)
- `MULTI_TENANT_MIGRATION_PLAN.md` - ✅ Active plan
- `MULTI_TENANT_MIGRATION_TRACKER.md` - ✅ Active tracker
- `MULTI_TENANT_NEXT_STEPS.md` - ✅ Active next steps
- `MULTI_TENANT_ROADMAP.md` - ✅ Active roadmap
- `MULTI_TENANT_STATUS_2025.md` - ✅ Active status (in French, needs translation)
- `RBAC_ANALYSIS.md` - ✅ Active analysis
- `VISIBILITY_AND_DATA_MODEL_ANALYSIS.md` - ✅ Active analysis
- `PHASE2_SCOPING_INSTANCES_PLAN.md` - ✅ Active plan
- `MULTI_TENANT_MODEL_SHARING_BILLING.md` - ✅ Active design doc

#### To Archive (Move to `docs/syntheses/archives/`)
- `DOCUMENTATION_CONSOLIDATION_20250115.md` - ⚠️ This document (after consolidation)

#### Already Archived (Keep in `docs/syntheses/archives/`)
- All files in `archives/` are correctly placed ✅

### Technical Documentation Analysis

#### Keep (Core Documentation)
- `docs/API_URL_CONFIGURATION.md` - ✅ Keep
- `docs/CHAT_SESSIONS_AND_INFERENCE.md` - ✅ Keep
- `docs/CI_CD.md` - ✅ Keep
- `docs/CONTRIBUTING.md` - ✅ Keep
- `docs/DEVELOPMENT_SETUP.md` - ✅ Keep
- `docs/engineering_guidelines.md` - ✅ Keep
- `docs/ia_widgets.md` - ✅ Keep
- `docs/STORAGE_MANAGEMENT.md` - ✅ Keep
- `docs/SCALEWAY_PROVISIONING.md` - ✅ Keep
- `docs/AGENT_VERSION_MANAGEMENT.md` - ✅ Keep
- `docs/STATE_MACHINE_AND_PROGRESS.md` - ✅ Keep
- `docs/worker_and_router_phase_0_2.md` - ✅ Keep

#### Consolidate/Merge
- `docs/ENDPOINTS_INVENTORY.md` - ⚠️ Check if still relevant (may be outdated)
- `docs/FLUX_ARCHITECTURE_MAP.md` - ⚠️ Check if still relevant
- `docs/INVENTIV_DATA_TABLE.md` - ⚠️ May be redundant with domain_design_and_data_model.md

#### Review/Update
- `docs/MOCK_REAL_VLLM_IMPLEMENTATION.md` - ⚠️ Review relevance
- `docs/MOCK_REAL_VLLM_USAGE.md` - ⚠️ Review relevance
- `docs/MONITORING_IMPROVEMENTS.md` - ⚠️ Review relevance
- `docs/PLATFORM_COMPATIBILITY.md` - ⚠️ Review relevance
- `docs/VLLM_IMAGE_SELECTION_ARCHITECTURE.md` - ⚠️ Review relevance
- `docs/DATA_VOLUME_RECOMMENDATION_SYNC.md` - ⚠️ Review relevance
- `docs/INSTANCE_TYPE_ZONES_COMPLETE.md` - ⚠️ Review relevance
- `docs/INSTANCE_TYPE_ZONES_IMPLEMENTATION.md` - ⚠️ Review relevance
- `docs/ACTION_TYPES_FORMAT.md` - ⚠️ Review relevance

#### Test Plans (Keep)
- `docs/TEST_PLAN_CHAT_SESSIONS.md` - ✅ Keep
- `docs/TEST_PLAN_STORAGE_MANAGEMENT.md` - ✅ Keep

#### Temporary Documents (Clean Up)
- `docs/tmp/DOCUMENTATION_CATEGORIZATION_20250108.md` - ⚠️ Old, can delete
- `docs/tmp/DOCUMENTATION_CLEANUP_FINAL_SUMMARY_20250108.md` - ⚠️ Old, can delete
- `docs/tmp/DOCUMENTATION_CLEANUP_PLAN_20250108.md` - ⚠️ Old, can delete
- `docs/tmp/DOCUMENTATION_CLEANUP_SUMMARY_20250108.md` - ⚠️ Old, can delete

---

## Consolidation Actions

### Phase 1: Translate French Documents

1. **Translate to English**:
   - `docs/syntheses/MULTI_TENANT_STATUS_2025.md` → English
   - Any other French documents found

### Phase 2: Consolidate Redundant Documents

1. **Merge/Consolidate**:
   - Review `docs/INVENTIV_DATA_TABLE.md` vs `docs/domain_design_and_data_model.md`
   - If redundant, merge into `domain_design_and_data_model.md` and archive `INVENTIV_DATA_TABLE.md`

2. **Review and Update**:
   - Check `docs/ENDPOINTS_INVENTORY.md` - if outdated, archive or update
   - Check `docs/FLUX_ARCHITECTURE_MAP.md` - if outdated, archive or update
   - Review technical docs for relevance

### Phase 3: Organize Synthesis Documents

1. **Keep Active** (in `docs/syntheses/`):
   - `MULTI_TENANT_MIGRATION_PLAN.md`
   - `MULTI_TENANT_MIGRATION_TRACKER.md`
   - `MULTI_TENANT_NEXT_STEPS.md`
   - `MULTI_TENANT_ROADMAP.md`
   - `MULTI_TENANT_STATUS_2025.md` (after translation)
   - `RBAC_ANALYSIS.md`
   - `VISIBILITY_AND_DATA_MODEL_ANALYSIS.md`
   - `PHASE2_SCOPING_INSTANCES_PLAN.md`
   - `MULTI_TENANT_MODEL_SHARING_BILLING.md`

2. **Archive** (move to `docs/syntheses/archives/`):
   - `DOCUMENTATION_CONSOLIDATION_20250115.md` (this file, after consolidation)

### Phase 4: Clean Up Temporary Documents

1. **Delete Old Temporary Files**:
   - `docs/tmp/DOCUMENTATION_CATEGORIZATION_20250108.md`
   - `docs/tmp/DOCUMENTATION_CLEANUP_FINAL_SUMMARY_20250108.md`
   - `docs/tmp/DOCUMENTATION_CLEANUP_PLAN_20250108.md`
   - `docs/tmp/DOCUMENTATION_CLEANUP_SUMMARY_20250108.md`

2. **Keep Current Temporary**:
   - `docs/tmp/DOCUMENTATION_CONSOLIDATION_PLAN_20250115.md` (this file)

### Phase 5: Update Main Entry Points

1. **Update README.md**:
   - Add multi-tenant features to Key Features section
   - Update architecture diagram if needed
   - Update documentation links

2. **Update TODO.md**:
   - Update multi-tenant section with current status
   - Add next steps for multi-tenant work

3. **Update docs/project_requirements.md**:
   - Consolidate with architecture.md if needed
   - Add multi-tenant requirements section

4. **Update docs/domain_design_and_data_model.md**:
   - Ensure user_sessions table is documented
   - Update with latest multi-tenant schema

---

## Implementation Order

1. ✅ Create consolidation plan (this document)
2. ⏳ Translate French documents
3. ⏳ Review and consolidate redundant documents
4. ⏳ Organize synthesis documents
5. ⏳ Clean up temporary documents
6. ⏳ Update main entry points
7. ⏳ Final review and verification

---

## Verification Checklist

After consolidation, verify:
- [ ] All main entry points are up-to-date
- [ ] All documentation is in English
- [ ] No duplicate content
- [ ] Synthesis documents are properly organized
- [ ] Temporary documents are cleaned up
- [ ] All links in README.md work
- [ ] Documentation structure is clear and navigable

---

## Notes

- This consolidation should be done incrementally to avoid breaking existing references
- Keep backups of archived documents
- Update any external references to moved documents
- Consider creating a `docs/README.md` index if needed

