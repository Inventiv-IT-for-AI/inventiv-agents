// Public routes (no authentication required)
use crate::app::AppState;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api_docs;
use crate::auth_endpoints;
use crate::password_reset;
use crate::version;

/// Create public routes router
pub fn create_public_routes() -> Router<Arc<AppState>> {
    Router::new()
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", api_docs::ApiDoc::openapi()),
        )
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
        .route("/version", get(get_version))
        .route("/api/version", get(get_version))
}

/// Get version information (public endpoint)
async fn get_version() -> axum::Json<version::VersionInfo> {
    axum::Json(version::get_version_info())
}

async fn root() -> &'static str {
    "Inventiv Backend API (Product Plane) - CQRS Enabled"
}
