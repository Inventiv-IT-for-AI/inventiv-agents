use crate::provider::CloudProvider;
use crate::providers::scaleway::ScalewayProvider;
// use std::collections::HashMap;
use std::env;

pub struct ProviderManager;

impl ProviderManager {
    pub fn get_provider(provider_name: &str) -> Option<Box<dyn CloudProvider>> {
        match provider_name.to_lowercase().as_str() {
            "scaleway" => {
                let project_id = env::var("SCALEWAY_PROJECT_ID").ok()?;
                let secret_key = env::var("SCALEWAY_SECRET_KEY").ok()?;
                if project_id.is_empty() || secret_key.is_empty() {
                    return None;
                }
                Some(Box::new(ScalewayProvider::new(project_id, secret_key)))
            }
            // Add other providers here:
            // "ovh" => ...
            _ => None,
        }
    }
}
