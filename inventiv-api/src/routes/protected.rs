// Protected routes (require user session)
use crate::app::AppState;
use crate::auth;
use axum::middleware;
use axum::routing::{get, post, put};
use axum::Router;
use std::sync::Arc;

use crate::action_logs_search;
use crate::api_keys;
use crate::auth_endpoints;
use crate::chat;
use crate::finops;
use crate::instance_type_zones;
use crate::metrics;
use crate::organizations;
use crate::provider_settings;
use crate::settings;
use crate::users_endpoint;

use crate::handlers::commands::list_action_logs;
use crate::handlers::commands::list_action_types;
use crate::handlers::commands::manual_catalog_sync_trigger;
use crate::handlers::commands::manual_reconcile_trigger;
use crate::handlers::deployments::create_deployment;
use crate::handlers::events::events_stream;
use crate::handlers::instances::archive_instance;
use crate::handlers::instances::get_instance;
use crate::handlers::instances::list_instances;
use crate::handlers::instances::reinstall_instance;
use crate::handlers::instances::search_instances;
use crate::handlers::instances::terminate_instance;
use crate::handlers::models::create_model;
use crate::handlers::models::delete_model;
use crate::handlers::models::get_model;
use crate::handlers::models::get_recommended_data_volume;
use crate::handlers::models::list_compatible_models;
use crate::handlers::models::list_models;
use crate::handlers::models::update_model;
use crate::handlers::monitoring::list_gpu_activity;
use crate::handlers::monitoring::list_runtime_models;
use crate::handlers::monitoring::list_system_activity;

/// Create protected routes router
pub fn create_protected_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/auth/me",
            get(auth_endpoints::me).put(auth_endpoints::update_me),
        )
        .route("/auth/me/password", put(auth_endpoints::change_password))
        .route("/auth/sessions", get(auth_endpoints::list_sessions))
        .route(
            "/auth/sessions/{session_id}/revoke",
            post(auth_endpoints::revoke_session_endpoint),
        )
        // Chat (UI): list allowed models for current workspace
        .route("/chat/models", get(chat::list_chat_models))
        // Organizations (multi-tenant MVP)
        .route(
            "/organizations",
            get(organizations::list_organizations).post(organizations::create_organization),
        )
        .route(
            "/organizations/current",
            put(organizations::set_current_organization),
        )
        // Organization management (current org)
        .route(
            "/organizations/current/members",
            get(organizations::list_current_organization_members),
        )
        .route(
            "/organizations/current/members/{user_id}",
            put(organizations::set_current_organization_member_role)
                .delete(organizations::remove_current_organization_member),
        )
        .route(
            "/organizations/current/leave",
            post(organizations::leave_current_organization),
        )
        // Organization invitations
        .route(
            "/organizations/current/invitations",
            get(organizations::list_current_organization_invitations)
                .post(organizations::create_current_organization_invitation),
        )
        .route(
            "/organizations/invitations/{token}/accept",
            post(organizations::accept_invitation),
        )
        // API Keys (dashboard-managed)
        .route(
            "/api_keys",
            get(api_keys::list_api_keys).post(api_keys::create_api_key),
        )
        .route("/api_keys/search", get(api_keys::search_api_keys))
        .route(
            "/api_keys/{id}",
            put(api_keys::update_api_key).delete(api_keys::revoke_api_key),
        )
        // Runtime models (models in service + historical + counters)
        .route("/runtime/models", get(list_runtime_models))
        // GPU activity (nvtop-like)
        .route("/gpu/activity", get(list_gpu_activity))
        // System activity (CPU/Mem/Disk/Network)
        .route("/system/activity", get(list_system_activity))
        .route("/deployments", post(create_deployment))
        // Realtime (SSE)
        .route("/events/stream", get(events_stream))
        // Models (catalog)
        .route("/models", get(list_models).post(create_model))
        .route(
            "/instance_types/{instance_type_id}/models",
            get(list_compatible_models),
        )
        .route(
            "/models/{id}",
            get(get_model).put(update_model).delete(delete_model),
        )
        .route(
            "/models/{id}/recommended-data-volume",
            get(get_recommended_data_volume),
        )
        // Instances
        .route("/instances", get(list_instances))
        .route("/instances/search", get(search_instances))
        .route(
            "/instances/{instance_id}/metrics",
            get(metrics::get_instance_metrics),
        )
        .route("/instances/{id}/archive", put(archive_instance))
        .route(
            "/instances/{id}",
            get(get_instance).delete(terminate_instance),
        )
        .route("/instances/{id}/reinstall", post(reinstall_instance))
        // Action logs
        .route("/action_logs", get(list_action_logs))
        .route(
            "/action_logs/search",
            get(action_logs_search::search_action_logs),
        )
        .route("/action_types", get(list_action_types))
        // Commands
        .route("/reconcile", post(manual_reconcile_trigger))
        .route("/catalog/sync", post(manual_catalog_sync_trigger))
        // Settings
        .route(
            "/providers",
            get(settings::list_providers).post(settings::create_provider),
        )
        .route("/providers/search", get(settings::search_providers))
        .route("/providers/{id}", put(settings::update_provider))
        .route(
            "/settings/definitions",
            get(provider_settings::list_settings_definitions),
        )
        .route(
            "/settings/global",
            get(provider_settings::list_global_settings)
                .put(provider_settings::upsert_global_setting),
        )
        // Provider-scoped params
        .route(
            "/providers/params",
            get(provider_settings::list_provider_params),
        )
        .route(
            "/providers/{id}/params",
            put(provider_settings::update_provider_params),
        )
        .route(
            "/providers/config-status",
            get(provider_settings::list_provider_config_status),
        )
        .route(
            "/regions",
            get(settings::list_regions).post(settings::create_region),
        )
        .route("/regions/search", get(settings::search_regions))
        .route("/regions/{id}", put(settings::update_region))
        .route(
            "/zones",
            get(settings::list_zones).post(settings::create_zone),
        )
        .route("/zones/search", get(settings::search_zones))
        .route("/zones/{id}", put(settings::update_zone))
        .route(
            "/instance_types",
            get(settings::list_instance_types).post(settings::create_instance_type),
        )
        .route(
            "/instance_types/search",
            get(settings::search_instance_types),
        )
        .route("/instance_types/{id}", put(settings::update_instance_type))
        // Instance Type <-> Zones
        .route(
            "/instance_types/{id}/zones",
            get(instance_type_zones::list_instance_type_zones),
        )
        .route(
            "/instance_types/{id}/zones",
            put(instance_type_zones::associate_zones_to_instance_type),
        )
        .route(
            "/zones/{zone_id}/instance_types",
            get(instance_type_zones::list_instance_types_for_zone),
        )
        // Finops
        .route("/finops/cost/current", get(finops::get_cost_current))
        .route(
            "/finops/dashboard/costs/current",
            get(finops::get_costs_dashboard_current),
        )
        .route(
            "/finops/dashboard/costs/summary",
            get(finops::get_costs_dashboard_summary),
        )
        .route(
            "/finops/dashboard/costs/window",
            get(finops::get_costs_dashboard_window),
        )
        .route(
            "/finops/dashboard/costs/series",
            get(finops::get_costs_dashboard_series),
        )
        .route(
            "/finops/cost/forecast/minute",
            get(finops::get_cost_forecast_series),
        )
        .route(
            "/finops/cost/actual/minute",
            get(finops::get_cost_actual_series),
        )
        .route(
            "/finops/cost/cumulative/minute",
            get(finops::get_cost_cumulative_series),
        )
        // Users management
        .route(
            "/users",
            get(users_endpoint::list_users).post(users_endpoint::create_user),
        )
        .route("/users/search", get(users_endpoint::search_users))
        .route(
            "/users/{id}",
            get(users_endpoint::get_user)
                .put(users_endpoint::update_user)
                .delete(users_endpoint::delete_user),
        )
        .route_layer(middleware::from_fn_with_state(
            state.db.clone(),
            auth::require_user,
        ))
}
