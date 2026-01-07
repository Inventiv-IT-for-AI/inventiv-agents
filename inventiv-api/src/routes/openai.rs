// OpenAI-compatible proxy routes (auth = cookie/JWT OR API key)
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use crate::app::AppState;
use crate::auth;
use std::sync::Arc;

use crate::handlers::openai::openai_list_models;
use crate::handlers::openai::openai_proxy_chat_completions;
use crate::handlers::openai::openai_proxy_completions;
use crate::handlers::openai::openai_proxy_embeddings;

/// Create OpenAI proxy routes router
pub fn create_openai_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/models", get(openai_list_models))
        .route("/v1/chat/completions", post(openai_proxy_chat_completions))
        .route("/v1/completions", post(openai_proxy_completions))
        .route("/v1/embeddings", post(openai_proxy_embeddings))
        .route_layer(middleware::from_fn_with_state(
            state.db.clone(),
            auth::require_user_or_api_key,
        ))
}

