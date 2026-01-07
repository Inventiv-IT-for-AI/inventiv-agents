// Public routes (no authentication required)
use axum::routing::{get, post};
use axum::Router;
use utoipa_swagger_ui::SwaggerUi;
use utoipa::OpenApi;
use crate::app::AppState;
use std::sync::Arc;

use crate::api_docs;
use crate::auth_endpoints;
use crate::password_reset;

/// Create public routes router
pub fn create_public_routes() -> Router<Arc<AppState>> {
    Router::new()
        .merge(SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", api_docs::ApiDoc::openapi()))
        .route("/", get(root))
        .route("/auth/login", post(auth_endpoints::login))
        .route("/auth/logout", post(auth_endpoints::logout))
        .route(
            "/auth/password-reset/request",
            post(password_reset::request_password_reset),
        )
        .route(
            "/auth/password-reset/reset",
            post(password_reset::reset_password),
        )
}

async fn root() -> &'static str {
    "Inventiv Backend API (Product Plane) - CQRS Enabled"
}

