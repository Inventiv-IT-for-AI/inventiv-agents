use std::net::SocketAddr;

// Configuration and setup modules
mod app;
mod config;
mod setup;

// Routes and handlers modules
mod handlers;
mod routes;

// Domain modules
mod action_logs_search;
mod api_docs;
mod api_keys;
mod auth;
mod auth_endpoints;
mod bootstrap_admin;
mod chat;
mod email;
mod finops;
mod instance_type_zones;
mod metrics;
mod openai_proxy;
mod organizations;
mod password_reset;
mod progress;
mod provider_settings;
mod rbac;
mod settings;
mod simple_logger;
mod users_endpoint;
mod version;
mod workbench;
mod worker_routing;

use app::AppState;
use config::{database::create_pool, redis::create_client};
use routes::create_router;
use setup::{maybe_seed_catalog, maybe_seed_provider_credentials, run_migrations};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    // Initialize Redis client
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let client = create_client(&redis_url).expect("Failed to create Redis client");

    // Initialize database pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = create_pool(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    // Run migrations
    if let Err(e) = run_migrations(&pool).await {
        eprintln!("[error] Failed to run migrations: {}", e);
        panic!("Failed to run migrations: {}", e);
    }

    // Optional seeding (guarded by env vars)
    maybe_seed_catalog(&pool).await;
    maybe_seed_provider_credentials(&pool).await;

    // Bootstrap default admin and organization
    bootstrap_admin::ensure_default_admin(&pool).await;
    bootstrap_admin::ensure_default_organization(&pool).await;

    // Create application state
    let state = AppState::new(client, pool);

    // Create CORS layer
    let cors = app::create_cors();

    // Create router using modular route definitions
    let app = create_router(state.clone())
        .layer(cors) // Apply CORS to ALL routes
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8003));
    println!("Backend listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
