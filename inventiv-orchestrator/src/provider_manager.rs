use crate::provider::CloudProvider;
use crate::providers::scaleway::ScalewayProvider;
use crate::providers::mock::MockProvider;
// use std::collections::HashMap;
use std::env;
use sqlx::{Pool, Postgres};

pub struct ProviderManager;

impl ProviderManager {
    pub fn current_provider_name() -> String {
        env::var("PROVIDER").unwrap_or_else(|_| "scaleway".to_string())
    }

    pub fn get_provider(provider_name: &str, db: Pool<Postgres>) -> Option<Box<dyn CloudProvider>> {
        match provider_name.to_lowercase().as_str() {
            "scaleway" => {
                let project_id = env::var("SCALEWAY_PROJECT_ID").ok()?;
                let secret_key = env::var("SCALEWAY_SECRET_KEY").ok()?;
                if project_id.is_empty() || secret_key.is_empty() {
                    return None;
                }
                let _ = db; // not needed for real provider
                Some(Box::new(ScalewayProvider::new(project_id, secret_key)))
            }
            "mock" => Some(Box::new(MockProvider::new(db))),
            // Add other providers here:
            // "ovh" => ...
            _ => None,
        }
    }
}
