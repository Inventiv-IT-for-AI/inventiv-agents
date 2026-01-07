// Workbench routes (auth = cookie/JWT OR API key)
use crate::app::AppState;
use crate::auth;
use axum::middleware;
use axum::routing::{delete, get, post, put};
use axum::Router;
use std::sync::Arc;

use crate::workbench;

/// Create workbench routes router
pub fn create_workbench_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/workbench/runs",
            get(workbench::list_workbench_runs).post(workbench::create_workbench_run),
        )
        .route("/workbench/runs/{id}", get(workbench::get_workbench_run))
        .route(
            "/workbench/runs/{id}",
            put(workbench::update_workbench_run).delete(workbench::delete_workbench_run),
        )
        .route(
            "/workbench/runs/{id}/messages",
            post(workbench::append_workbench_message),
        )
        .route(
            "/workbench/runs/{id}/complete",
            post(workbench::complete_workbench_run),
        )
        .route(
            "/workbench/projects",
            get(workbench::list_workbench_projects).post(workbench::create_workbench_project),
        )
        .route(
            "/workbench/projects/{id}",
            put(workbench::update_workbench_project).delete(workbench::delete_workbench_project),
        )
        .route_layer(middleware::from_fn_with_state(
            state.db.clone(),
            auth::require_user_or_api_key,
        ))
}
