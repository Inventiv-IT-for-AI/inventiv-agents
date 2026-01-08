# Documentation Index

This directory contains all project documentation. Documents are organized by category and purpose.

## 📚 Core Documentation (Essential Entry Points)

These documents are **must-read** and must be kept up-to-date:

- **[README.md](../README.md)** - Project overview and quickstart
- **[TODO.md](../TODO.md)** - Roadmap and current status
- **[architecture.md](architecture.md)** - Technical architecture overview
- **[domain_design_and_data_model.md](domain_design_and_data_model.md)** - Domain model and data structures (DDD)
- **[project_requirements.md](project_requirements.md)** - Project requirements and general specification
- **[ui_design_system.md](ui_design_system.md)** - UI design system and component conventions
- **[ia_widgets.md](ia_widgets.md)** - AI UI widgets documentation
- **[IADATA_TABLE_GUIDE.md](IADATA_TABLE_GUIDE.md)** - IADataTable component user guide
- **[engineering_guidelines.md](engineering_guidelines.md)** - Code quality and best practices
- **[testing.md](testing.md)** - Testing strategies and test plans
- **[CONTRIBUTING.md](CONTRIBUTING.md)** - Contribution guidelines
- **[DEVELOPMENT_SETUP.md](DEVELOPMENT_SETUP.md)** - Development environment setup

## 🎯 Feature Documentation

### Core Features
- **[AGENT_VERSION_MANAGEMENT.md](AGENT_VERSION_MANAGEMENT.md)** - Agent versioning and integrity
- **[STORAGE_MANAGEMENT.md](STORAGE_MANAGEMENT.md)** - Volume discovery and cleanup
- **[STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md)** - Instance lifecycle and progress tracking
- **[SCALEWAY_PROVISIONING.md](SCALEWAY_PROVISIONING.md)** - Scaleway provider integration
- **[CI_CD.md](CI_CD.md)** - Continuous Integration and Deployment
- **[API_URL_CONFIGURATION.md](API_URL_CONFIGURATION.md)** - API endpoint configuration
- **[INVENTIV_DATA_TABLE.md](INVENTIV_DATA_TABLE.md)** - Database schema reference
- **[PLATFORM_COMPATIBILITY.md](PLATFORM_COMPATIBILITY.md)** - Platform compatibility matrix

### Implementation Guides
- **[ACTION_TYPES_FORMAT.md](ACTION_TYPES_FORMAT.md)** - Action types format reference
- **[CHAT_SESSIONS_AND_INFERENCE.md](CHAT_SESSIONS_AND_INFERENCE.md)** - Chat sessions and inference
- **[DATA_VOLUME_RECOMMENDATION_SYNC.md](DATA_VOLUME_RECOMMENDATION_SYNC.md)** - Data volume recommendations
- **[DEPLOYMENT_STAGING.md](DEPLOYMENT_STAGING.md)** - Staging deployment guide
- **[ENDPOINTS_INVENTORY.md](ENDPOINTS_INVENTORY.md)** - API endpoints inventory
- **[worker_and_router_phase_0_2.md](worker_and_router_phase_0_2.md)** - Worker and router architecture
- **[INSTANCE_TYPE_ZONES_COMPLETE.md](INSTANCE_TYPE_ZONES_COMPLETE.md)** - Instance types and zones reference
- **[INSTANCE_TYPE_ZONES_IMPLEMENTATION.md](INSTANCE_TYPE_ZONES_IMPLEMENTATION.md)** - Instance types implementation
- **[MOCK_REAL_VLLM_IMPLEMENTATION.md](MOCK_REAL_VLLM_IMPLEMENTATION.md)** - Mock vLLM implementation
- **[MOCK_REAL_VLLM_USAGE.md](MOCK_REAL_VLLM_USAGE.md)** - Mock vLLM usage guide
- **[VLLM_IMAGE_SELECTION_ARCHITECTURE.md](VLLM_IMAGE_SELECTION_ARCHITECTURE.md)** - vLLM image selection

### Testing
- **[testing.md](testing.md)** - Testing guide and strategies (consolidated)
- **[TEST_PLAN_CHAT_SESSIONS.md](TEST_PLAN_CHAT_SESSIONS.md)** - Chat sessions test plan
- **[TEST_PLAN_STORAGE_MANAGEMENT.md](TEST_PLAN_STORAGE_MANAGEMENT.md)** - Storage management test plan

## 🏢 Multi-Tenant Documentation

Active plans and implementation guides (see `syntheses/` directory):

- **[syntheses/MULTI_TENANT_ROADMAP.md](syntheses/MULTI_TENANT_ROADMAP.md)** - Multi-tenant roadmap
- **[syntheses/MULTI_TENANT_MODEL_SHARING_BILLING.md](syntheses/MULTI_TENANT_MODEL_SHARING_BILLING.md)** - Model sharing and billing
- **[syntheses/MULTI_TENANT_MIGRATION_PLAN.md](syntheses/MULTI_TENANT_MIGRATION_PLAN.md)** - Migration plan
- **[syntheses/MULTI_TENANT_MIGRATION_TRACKER.md](syntheses/MULTI_TENANT_MIGRATION_TRACKER.md)** - Migration progress tracker
- **[syntheses/RBAC_ANALYSIS.md](syntheses/RBAC_ANALYSIS.md)** - Role-Based Access Control analysis
- **[syntheses/MULTI_TENANT_NEXT_STEPS.md](syntheses/MULTI_TENANT_NEXT_STEPS.md)** - Next steps (active plan)
- **[syntheses/MULTI_TENANT_STATUS_2025.md](syntheses/MULTI_TENANT_STATUS_2025.md)** - Current status
- **[syntheses/VISIBILITY_AND_DATA_MODEL_ANALYSIS.md](syntheses/VISIBILITY_AND_DATA_MODEL_ANALYSIS.md)** - Visibility and data model analysis

## 📊 Reference Documentation

- **[FLUX_ARCHITECTURE_MAP.md](FLUX_ARCHITECTURE_MAP.md)** - Architecture flow map
- **[MONITORING_IMPROVEMENTS.md](MONITORING_IMPROVEMENTS.md)** - Monitoring improvements reference

## 📁 Documentation Organization

### `docs/syntheses/`
Contains synthesis documents and active action plans:
- Multi-tenant implementation plans and roadmaps
- Active analysis documents
- Current status and next steps documents

### `docs/syntheses/archives/`
Contains historical documents:
- Completed implementation summaries
- Obsolete analysis documents
- Historical session summaries

### `docs/tmp/`
Temporary documents (dated, can be deleted after session):
- Working documents
- Temporary analysis files

## 🔄 Documentation Maintenance

### Rules
1. **All documentation must be in English** (for open-source contribution)
2. **Core documents** (README, specification, architecture) must be kept up-to-date
3. **Historical documents** should be moved to `syntheses/archives/`
4. **Temporary documents** should be in `tmp/` and dated

### Adding New Documentation
- Feature documentation → `docs/`
- Implementation plans → `docs/syntheses/`
- Historical summaries → `docs/syntheses/archives/`
- Temporary work → `docs/tmp/` (with date)

## 📝 Translation Status

- ✅ English: All core documents and main entry points
- ✅ English: Technical documentation (ACTION_TYPES_FORMAT, API_URL_CONFIGURATION, etc.)
- ⚠️ French: Some historical documents in archives (can remain)
- 🔄 In progress: Remaining technical documentation translation (incremental)

