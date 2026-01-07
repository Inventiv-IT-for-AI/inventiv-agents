// Library entry point for tests and external usage
// Re-exports all modules needed for testing

pub mod action_logs_search;
pub mod api_docs;
pub mod api_keys;
pub mod app;
pub mod auth;
pub mod auth_endpoints;
pub mod bootstrap_admin;
pub mod chat;
pub mod config;
pub mod email;
pub mod finops;
pub mod handlers;
pub mod instance_type_zones;
pub mod metrics;
pub mod openai_proxy;
pub mod organizations;
pub mod password_reset;
pub mod progress;
pub mod provider_settings;
pub mod rbac;
pub mod routes;
pub mod settings;
pub mod setup;
pub mod simple_logger;
pub mod users_endpoint;
pub mod version;
pub mod workbench;
pub mod worker_routing;

// Re-export commonly used types
pub use app::AppState;
