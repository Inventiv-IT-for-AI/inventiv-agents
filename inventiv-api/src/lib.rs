// Library entry point for tests and external usage
// Re-exports all modules needed for testing

pub mod app;
pub mod config;
pub mod setup;
pub mod routes;
pub mod handlers;
pub mod auth;
pub mod auth_endpoints;
pub mod bootstrap_admin;
pub mod organizations;
pub mod users_endpoint;
pub mod api_keys;
pub mod workbench;
pub mod settings;
pub mod provider_settings;
pub mod finops;
pub mod chat;
pub mod password_reset;
pub mod email;
pub mod simple_logger;
pub mod progress;
pub mod metrics;
pub mod instance_type_zones;
pub mod action_logs_search;
pub mod api_docs;
pub mod rbac;
pub mod openai_proxy;
pub mod worker_routing;

// Re-export commonly used types
pub use app::AppState;

