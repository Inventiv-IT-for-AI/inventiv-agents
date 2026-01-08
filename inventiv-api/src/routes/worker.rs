// Worker internal routes (worker auth handled in handler + orchestrator)
use crate::app::AppState;
use axum::routing::post;
use axum::Router;
use std::sync::Arc;

use crate::handlers::worker::proxy_worker_heartbeat;
use crate::handlers::worker::proxy_worker_register;

/// Create worker routes router
pub fn create_worker_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/internal/worker/register", post(proxy_worker_register))
        .route("/internal/worker/heartbeat", post(proxy_worker_heartbeat))
}
