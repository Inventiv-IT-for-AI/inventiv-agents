use crate::provider::CloudProvider;
use crate::providers::mock::MockProvider;
use crate::providers::scaleway::ScalewayProvider;
// use std::collections::HashMap;
use sqlx::{Pool, Postgres};
use std::env;
use std::fs;
use std::path::Path;

pub struct ProviderManager;

impl ProviderManager {
    pub fn current_provider_name() -> String {
        env::var("PROVIDER").unwrap_or_else(|_| "scaleway".to_string())
    }

    fn read_secret_file(path: &str) -> Result<String, String> {
        let p = path.trim();
        if p.is_empty() {
            return Err("secret file path is empty".to_string());
        }
        fs::read_to_string(p)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("failed to read secret file '{}': {}", p, e))
    }

    fn scaleway_init_from_env() -> Result<(String, String, Option<String>), String> {
        // Project id can come from either SCALEWAY_PROJECT_ID or SCW_PROJECT_ID (common alias).
        let project_id = env::var("SCALEWAY_PROJECT_ID")
            .ok()
            .or_else(|| env::var("SCW_PROJECT_ID").ok())
            .unwrap_or_default()
            .trim()
            .to_string();

        // Prefer *_FILE for secrets (Docker/K8s friendly), fallback to env vars.
        let secret_key_file = env::var("SCALEWAY_SECRET_KEY_FILE")
            .ok()
            .or_else(|| env::var("SCW_SECRET_KEY_FILE").ok())
            .unwrap_or_else(|| "/run/secrets/scaleway_secret_key".to_string());

        let secret_key = match Self::read_secret_file(&secret_key_file) {
            Ok(v) if !v.is_empty() => v,
            Ok(_) => {
                // Empty file: continue to env var fallbacks.
                String::new()
            }
            Err(_) => String::new(),
        };

        let secret_key = if !secret_key.is_empty() {
            secret_key
        } else {
            env::var("SCALEWAY_SECRET_KEY")
                .ok()
                .or_else(|| env::var("SCALEWAY_API_TOKEN").ok())
                .or_else(|| env::var("SCW_SECRET_KEY").ok())
                .unwrap_or_default()
                .trim()
                .to_string()
        };

        // SSH key is optional: used for provisioning/debug.
        let ssh_public_key_file = env::var("SCALEWAY_SSH_PUBLIC_KEY_FILE")
            .unwrap_or_else(|_| "/app/.ssh/llm-studio-key.pub".to_string());
        let ssh_public_key = fs::read_to_string(&ssh_public_key_file)
            .ok()
            .or_else(|| env::var("SCALEWAY_SSH_PUBLIC_KEY").ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if project_id.is_empty() || secret_key.is_empty() {
            // Return a high-signal diagnostic that can be bubbled up into action logs.
            let mut missing: Vec<String> = vec![];
            if project_id.is_empty() {
                missing.push("project_id (env SCALEWAY_PROJECT_ID or SCW_PROJECT_ID)".to_string());
            }
            if secret_key.is_empty() {
                let file_exists = Path::new(secret_key_file.trim()).exists();
                missing.push(format!(
                    "secret_key (file {}{}; or env SCALEWAY_SECRET_KEY / SCALEWAY_API_TOKEN / SCW_SECRET_KEY)",
                    secret_key_file,
                    if file_exists { "" } else { " [missing]" }
                ));
            }
            return Err(format!(
                "Scaleway provider credentials missing: {}",
                missing.join(", ")
            ));
        }

        Ok((project_id, secret_key, ssh_public_key))
    }

    pub fn get_provider(
        provider_name: &str,
        db: Pool<Postgres>,
    ) -> Result<Box<dyn CloudProvider>, String> {
        match provider_name.to_lowercase().as_str() {
            "scaleway" => {
                let (project_id, secret_key, ssh_public_key) = Self::scaleway_init_from_env()?;
                let _ = db; // not needed for real provider
                Ok(Box::new(ScalewayProvider::new(
                    project_id,
                    secret_key,
                    ssh_public_key,
                )))
            }
            "mock" => Ok(Box::new(MockProvider::new(db))),
            // Add other providers here:
            // "ovh" => ...
            _ => Err(format!("Unknown provider '{}'", provider_name)),
        }
    }
}
