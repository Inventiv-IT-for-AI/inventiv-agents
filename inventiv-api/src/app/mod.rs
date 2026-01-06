// Application state and configuration
pub mod state;

pub use state::AppState;

use tower_http::cors::{Any, CorsLayer};

/// Create CORS layer with permissive settings
pub fn create_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}

