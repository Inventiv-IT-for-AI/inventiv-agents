// Worker internal routes (worker auth handled in handler + orchestrator)
use axum::routing::post;
use axum::Router;
use crate::app::AppState;
use std::sync::Arc;

use crate::handlers::worker::proxy_worker_register;
use crate::handlers::worker::proxy_worker_heartbeat;

/// Create worker routes router
pub fn create_worker_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/internal/worker/register", post(proxy_worker_register))
        .route("/internal/worker/heartbeat", post(proxy_worker_heartbeat))
}

