# Inventaire Complet des Endpoints - inventiv-api

**Date**: 2024  
**Objectif**: Référence exhaustive de tous les endpoints pour la modularisation.

---

## 📋 Routes Publiques (No Auth)

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/` | `root()` | main.rs | ❌ À garder |
| GET | `/swagger-ui/*` | SwaggerUi | api_docs.rs | ✅ OK |
| GET | `/api-docs/openapi.json` | ApiDoc::openapi() | api_docs.rs | ✅ OK |
| POST | `/auth/login` | `auth_endpoints::login` | auth_endpoints.rs | ✅ OK |
| POST | `/auth/logout` | `auth_endpoints::logout` | auth_endpoints.rs | ✅ OK |

---

## 🔧 Routes Worker (Worker Auth)

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| POST | `/internal/worker/register` | `proxy_worker_register()` | main.rs | ❌ À extraire |
| POST | `/internal/worker/heartbeat` | `proxy_worker_heartbeat()` | main.rs | ❌ À extraire |

---

## 🤖 Routes OpenAI Proxy (User/API Key Auth)

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/v1/models` | `openai_list_models()` | main.rs | ⚠️ Handler dans main.rs |
| POST | `/v1/chat/completions` | `openai_proxy_chat_completions()` | main.rs | ⚠️ Handler dans main.rs |
| POST | `/v1/completions` | `openai_proxy_completions()` | main.rs | ⚠️ Handler dans main.rs |
| POST | `/v1/embeddings` | `openai_proxy_embeddings()` | main.rs | ⚠️ Handler dans main.rs |

**Note**: Routes définies dans main.rs mais logique dans `openai_proxy.rs` ? À vérifier.

---

## 💼 Routes Workbench (User/API Key Auth)

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/workbench/runs` | `workbench::list_workbench_runs` | workbench.rs | ✅ OK |
| POST | `/workbench/runs` | `workbench::create_workbench_run` | workbench.rs | ✅ OK |
| GET | `/workbench/runs/:id` | `workbench::get_workbench_run` | workbench.rs | ✅ OK |
| PUT | `/workbench/runs/:id` | `workbench::update_workbench_run` | workbench.rs | ✅ OK |
| DELETE | `/workbench/runs/:id` | `workbench::delete_workbench_run` | workbench.rs | ✅ OK |
| POST | `/workbench/runs/:id/messages` | `workbench::append_workbench_message` | workbench.rs | ✅ OK |
| POST | `/workbench/runs/:id/complete` | `workbench::complete_workbench_run` | workbench.rs | ✅ OK |
| GET | `/workbench/projects` | `workbench::list_workbench_projects` | workbench.rs | ✅ OK |
| POST | `/workbench/projects` | `workbench::create_workbench_project` | workbench.rs | ✅ OK |
| PUT | `/workbench/projects/:id` | `workbench::update_workbench_project` | workbench.rs | ✅ OK |
| DELETE | `/workbench/projects/:id` | `workbench::delete_workbench_project` | workbench.rs | ✅ OK |

---

## 🔐 Routes Protégées (User Auth Required)

### Authentication & Profile

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/auth/me` | `auth_endpoints::me` | auth_endpoints.rs | ✅ OK |
| PUT | `/auth/me` | `auth_endpoints::update_me` | auth_endpoints.rs | ✅ OK |
| PUT | `/auth/me/password` | `auth_endpoints::change_password` | auth_endpoints.rs | ✅ OK |

### Chat

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/chat/models` | `chat::list_chat_models` | chat.rs | ✅ OK |

### Organizations (Multi-tenant)

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/organizations` | `organizations::list_organizations` | organizations.rs | ✅ OK |
| POST | `/organizations` | `organizations::create_organization` | organizations.rs | ✅ OK |
| PUT | `/organizations/current` | `organizations::set_current_organization` | organizations.rs | ✅ OK |
| GET | `/organizations/current/members` | `organizations::list_current_organization_members` | organizations.rs | ✅ OK |
| PUT | `/organizations/current/members/:user_id` | `organizations::set_current_organization_member_role` | organizations.rs | ✅ OK |
| DELETE | `/organizations/current/members/:user_id` | `organizations::remove_current_organization_member` | organizations.rs | ✅ OK |
| POST | `/organizations/current/leave` | `organizations::leave_current_organization` | organizations.rs | ✅ OK |

### API Keys

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/api_keys` | `api_keys::list_api_keys` | api_keys.rs | ✅ OK |
| POST | `/api_keys` | `api_keys::create_api_key` | api_keys.rs | ✅ OK |
| GET | `/api_keys/search` | `api_keys::search_api_keys` | api_keys.rs | ✅ OK |
| PUT | `/api_keys/:id` | `api_keys::update_api_key` | api_keys.rs | ✅ OK |
| DELETE | `/api_keys/:id` | `api_keys::revoke_api_key` | api_keys.rs | ✅ OK |

### Runtime & Observability

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/runtime/models` | `list_runtime_models()` | main.rs | ❌ À extraire |
| GET | `/gpu/activity` | `list_gpu_activity()` | main.rs | ❌ À extraire |
| GET | `/system/activity` | `list_system_activity()` | main.rs | ❌ À extraire |

### Deployments

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| POST | `/deployments` | `create_deployment()` | main.rs | ❌ À extraire |

### Realtime (SSE)

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/events/stream` | `events_stream()` | main.rs | ❌ À extraire |

### Models (Catalogue)

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/models` | `list_models()` | main.rs | ❌ À extraire |
| POST | `/models` | `create_model()` | main.rs | ❌ À extraire |
| GET | `/models/:id` | `get_model()` | main.rs | ❌ À extraire |
| PUT | `/models/:id` | `update_model()` | main.rs | ❌ À extraire |
| DELETE | `/models/:id` | `delete_model()` | main.rs | ❌ À extraire |
| GET | `/instance_types/:instance_type_id/models` | `list_compatible_models()` | main.rs | ❌ À extraire |

### Instances

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/instances` | `list_instances()` | main.rs | ❌ À extraire |
| GET | `/instances/search` | `search_instances()` | main.rs | ❌ À extraire |
| GET | `/instances/:instance_id/metrics` | `metrics::get_instance_metrics` | metrics.rs | ✅ OK |
| GET | `/instances/:id` | `get_instance()` | main.rs | ❌ À extraire |
| DELETE | `/instances/:id` | `terminate_instance()` | main.rs | ❌ À extraire |
| PUT | `/instances/:id/archive` | `archive_instance()` | main.rs | ❌ À extraire |
| POST | `/instances/:id/reinstall` | `reinstall_instance()` | main.rs | ❌ À extraire |

### Action Logs

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/action_logs` | `list_action_logs()` | main.rs | ❌ À extraire |
| GET | `/action_logs/search` | `action_logs_search::search_action_logs` | action_logs_search.rs | ✅ OK |
| GET | `/action_types` | `list_action_types()` | main.rs | ❌ À extraire |

### Commands (Orchestrator)

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| POST | `/reconcile` | `manual_reconcile_trigger()` | main.rs | ❌ À extraire |
| POST | `/catalog/sync` | `manual_catalog_sync_trigger()` | main.rs | ❌ À extraire |

### Settings (Infrastructure)

#### Providers

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/providers` | `settings::list_providers` | settings.rs | ✅ OK |
| POST | `/providers` | `settings::create_provider` | settings.rs | ✅ OK |
| GET | `/providers/search` | `settings::search_providers` | settings.rs | ✅ OK |
| PUT | `/providers/:id` | `settings::update_provider` | settings.rs | ✅ OK |

#### Provider Settings

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/settings/definitions` | `provider_settings::list_settings_definitions` | provider_settings.rs | ✅ OK |
| GET | `/settings/global` | `provider_settings::list_global_settings` | provider_settings.rs | ✅ OK |
| PUT | `/settings/global` | `provider_settings::upsert_global_setting` | provider_settings.rs | ✅ OK |
| GET | `/providers/params` | `provider_settings::list_provider_params` | provider_settings.rs | ✅ OK |
| PUT | `/providers/:id/params` | `provider_settings::update_provider_params` | provider_settings.rs | ✅ OK |

#### Regions

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/regions` | `settings::list_regions` | settings.rs | ✅ OK |
| POST | `/regions` | `settings::create_region` | settings.rs | ✅ OK |
| GET | `/regions/search` | `settings::search_regions` | settings.rs | ✅ OK |
| PUT | `/regions/:id` | `settings::update_region` | settings.rs | ✅ OK |

#### Zones

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/zones` | `settings::list_zones` | settings.rs | ✅ OK |
| POST | `/zones` | `settings::create_zone` | settings.rs | ✅ OK |
| GET | `/zones/search` | `settings::search_zones` | settings.rs | ✅ OK |
| PUT | `/zones/:id` | `settings::update_zone` | settings.rs | ✅ OK |

#### Instance Types

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/instance_types` | `settings::list_instance_types` | settings.rs | ✅ OK |
| POST | `/instance_types` | `settings::create_instance_type` | settings.rs | ✅ OK |
| GET | `/instance_types/search` | `settings::search_instance_types` | settings.rs | ✅ OK |
| PUT | `/instance_types/:id` | `settings::update_instance_type` | settings.rs | ✅ OK |

#### Instance Type ↔ Zones Associations

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/instance_types/:id/zones` | `instance_type_zones::list_instance_type_zones` | instance_type_zones.rs | ✅ OK |
| PUT | `/instance_types/:id/zones` | `instance_type_zones::associate_zones_to_instance_type` | instance_type_zones.rs | ✅ OK |
| GET | `/zones/:zone_id/instance_types` | `instance_type_zones::list_instance_types_for_zone` | instance_type_zones.rs | ✅ OK |

### Finops

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/finops/cost/current` | `finops::get_cost_current` | finops.rs | ✅ OK |
| GET | `/finops/dashboard/costs/current` | `finops::get_costs_dashboard_current` | finops.rs | ✅ OK |
| GET | `/finops/dashboard/costs/summary` | `finops::get_costs_dashboard_summary` | finops.rs | ✅ OK |
| GET | `/finops/dashboard/costs/window` | `finops::get_costs_dashboard_window` | finops.rs | ✅ OK |
| GET | `/finops/dashboard/costs/series` | `finops::get_costs_dashboard_series` | finops.rs | ✅ OK |
| GET | `/finops/cost/forecast/minute` | `finops::get_cost_forecast_series` | finops.rs | ✅ OK |
| GET | `/finops/cost/actual/minute` | `finops::get_cost_actual_series` | finops.rs | ✅ OK |
| GET | `/finops/cost/cumulative/minute` | `finops::get_cost_cumulative_series` | finops.rs | ✅ OK |

### Users Management

| Méthode | Route | Handler | Module | Statut |
|---------|-------|---------|--------|--------|
| GET | `/users` | `users_endpoint::list_users` | users_endpoint.rs | ✅ OK |
| POST | `/users` | `users_endpoint::create_user` | users_endpoint.rs | ✅ OK |
| GET | `/users/search` | `users_endpoint::search_users` | users_endpoint.rs | ✅ OK |
| GET | `/users/:id` | `users_endpoint::get_user` | users_endpoint.rs | ✅ OK |
| PUT | `/users/:id` | `users_endpoint::update_user` | users_endpoint.rs | ✅ OK |
| DELETE | `/users/:id` | `users_endpoint::delete_user` | users_endpoint.rs | ✅ OK |

---

## 📊 Résumé par Statut

### ✅ Déjà Modulaires (Pas de changement)
- **Authentication**: `auth.rs`, `auth_endpoints.rs`
- **API Keys**: `api_keys.rs`
- **Organizations**: `organizations.rs`
- **Users**: `users_endpoint.rs`
- **Finops**: `finops.rs`
- **Workbench**: `workbench.rs`
- **Chat**: `chat.rs`
- **Settings**: `settings.rs`, `provider_settings.rs`, `instance_type_zones.rs`
- **Metrics**: `metrics.rs` (partiel)
- **Action Logs Search**: `action_logs_search.rs`
- **API Docs**: `api_docs.rs`

**Total**: ~12 modules bien organisés

### ❌ À Extraire de main.rs

| Domaine | Endpoints | Lignes estimées | Priorité |
|---------|-----------|-----------------|----------|
| **Models** | 6 endpoints | ~300 | Moyenne |
| **Instances** | 7 endpoints | ~1000 | Haute |
| **Deployments** | 1 endpoint | ~600 | Critique |
| **Observability** | 3 endpoints | ~600 | Moyenne |
| **Action Logs** | 2 endpoints | ~100 | Basse |
| **Commands** | 2 endpoints | ~80 | Basse |
| **Realtime** | 1 endpoint | ~180 | Moyenne |
| **Worker** | 2 endpoints | ~150 | Moyenne |
| **OpenAI Proxy** | 4 endpoints | ~100 | À vérifier |

**Total à extraire**: ~28 endpoints, ~3110 lignes

---

## 🎯 Plan d'Action Recommandé

1. **Phase 1** (Low Risk): Commands, Action Logs, Models
2. **Phase 2** (Medium Risk): Observability, Realtime, Worker
3. **Phase 3** (High Risk): Instances, Deployments
4. **Phase 4** (Cleanup): Réduire main.rs à orchestration uniquement

---

**Note**: Ce document sert de référence pendant le refactoring. Mettre à jour au fur et à mesure de l'extraction.

