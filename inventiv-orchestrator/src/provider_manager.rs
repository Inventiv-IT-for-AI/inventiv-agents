use crate::provider::CloudProvider;
use crate::providers::scaleway::ScalewayProvider;
use crate::providers::mock::MockProvider;
// use std::collections::HashMap;
use std::env;
use std::fs;
use sqlx::{Pool, Postgres};

pub struct ProviderManager;

impl ProviderManager {
    pub fn current_provider_name() -> String {
        env::var("PROVIDER").unwrap_or_else(|_| "scaleway".to_string())
    }

    pub fn get_provider(provider_name: &str, db: Pool<Postgres>) -> Option<Box<dyn CloudProvider>> {
        match provider_name.to_lowercase().as_str() {
            "scaleway" => {
                let project_id = env::var("SCALEWAY_PROJECT_ID")
                    .ok()
                    .map(|s| s.trim().to_string())?;
                // Prefer *_FILE for secrets (Docker/K8s friendly), fallback to env var.
                let secret_key_file = env::var("SCALEWAY_SECRET_KEY_FILE")
                    .unwrap_or_else(|_| "/run/secrets/scaleway_secret_key".to_string());
                let secret_key = fs::read_to_string(&secret_key_file)
                    .ok()
                    .or_else(|| env::var("SCALEWAY_SECRET_KEY").ok())
                    .map(|s| s.trim().to_string())?;
                // Prefer a project-local key file (mounted in container via /app).
                // Fallback to an env string if desired.
                let ssh_public_key_file =
                    env::var("SCALEWAY_SSH_PUBLIC_KEY_FILE").unwrap_or_else(|_| "/app/.ssh/llm-studio-key.pub".to_string());
                let ssh_public_key = fs::read_to_string(&ssh_public_key_file)
                    .ok()
                    .or_else(|| env::var("SCALEWAY_SSH_PUBLIC_KEY").ok())
                    .map(|s| s.trim().to_string());
                if project_id.is_empty() || secret_key.is_empty() {
                    return None;
                }
                let _ = db; // not needed for real provider
                Some(Box::new(ScalewayProvider::new(project_id, secret_key, ssh_public_key)))
            }
            "mock" => Some(Box::new(MockProvider::new(db))),
            // Add other providers here:
            // "ovh" => ...
            _ => None,
        }
    }
}
