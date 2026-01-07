// Routes module - Centralizes all route definitions
pub mod openai;
pub mod protected;
pub mod public;
pub mod workbench;
pub mod worker;

use crate::app::AppState;
use axum::Router;
use std::sync::Arc;

/// Build the main application router
pub fn create_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .merge(public::create_public_routes())
        .merge(worker::create_worker_routes())
        .merge(openai::create_openai_routes(state.clone()))
        .merge(workbench::create_workbench_routes(state.clone()))
        .merge(protected::create_protected_routes(state.clone()))
}
