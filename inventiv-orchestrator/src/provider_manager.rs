use inventiv_providers::CloudProvider;
#[cfg(feature = "provider-scaleway")]
use inventiv_providers::scaleway::ScalewayProvider;
// use std::collections::HashMap;
use sqlx::{Pool, Postgres};
use std::env;
use std::fs;
use std::path::Path;

#[cfg(feature = "provider-mock")]
use inventiv_providers::mock::MockProvider;

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

    fn provider_settings_passphrase() -> Option<String> {
        // This passphrase MUST come from a secret (never committed).
        // We support both *_FILE (preferred) and direct env value.
        let passphrase_file = env::var("PROVIDER_SETTINGS_ENCRYPTION_KEY_FILE")
            .ok()
            .or_else(|| env::var("PROVIDER_SETTINGS_PASSPHRASE_FILE").ok())
            .unwrap_or_else(|| "/run/secrets/provider_settings_key".to_string());
        let from_file = Self::read_secret_file(&passphrase_file).ok();
        let from_env = env::var("PROVIDER_SETTINGS_ENCRYPTION_KEY")
            .ok()
            .or_else(|| env::var("PROVIDER_SETTINGS_PASSPHRASE").ok());
        from_file.or(from_env).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    async fn scaleway_init_from_db(
        db: &Pool<Postgres>,
    ) -> Result<Option<(String, String, Option<String>)>, String> {
        // Resolve provider_id by code.
        let provider_id: Option<uuid::Uuid> =
            sqlx::query_scalar("SELECT id FROM providers WHERE code = 'scaleway' LIMIT 1")
                .fetch_optional(db)
                .await
                .map_err(|e| format!("DB error resolving provider id: {}", e))?
                .flatten();

        let Some(provider_id) = provider_id else {
            return Ok(None);
        };

        let project_id: Option<String> = sqlx::query_scalar(
            "SELECT NULLIF(btrim(value_text), '') FROM provider_settings WHERE provider_id=$1 AND key='SCALEWAY_PROJECT_ID'",
        )
        .bind(provider_id)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("DB error reading SCALEWAY_PROJECT_ID: {}", e))?
        .flatten();

        // Prefer encrypted key if present.
        let enc_b64: Option<String> = sqlx::query_scalar(
            "SELECT NULLIF(btrim(value_text), '') FROM provider_settings WHERE provider_id=$1 AND key='SCALEWAY_SECRET_KEY_ENC'",
        )
        .bind(provider_id)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("DB error reading SCALEWAY_SECRET_KEY_ENC: {}", e))?
        .flatten();

        let plain: Option<String> = sqlx::query_scalar(
            "SELECT NULLIF(btrim(value_text), '') FROM provider_settings WHERE provider_id=$1 AND key='SCALEWAY_SECRET_KEY'",
        )
        .bind(provider_id)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("DB error reading SCALEWAY_SECRET_KEY: {}", e))?
        .flatten();

        let secret_key = if let Some(enc) = enc_b64 {
            let Some(passphrase) = Self::provider_settings_passphrase() else {
                return Err("SCALEWAY_SECRET_KEY_ENC exists in DB but PROVIDER_SETTINGS_ENCRYPTION_KEY[_FILE] is not set".to_string());
            };
            let decrypted: Option<String> = sqlx::query_scalar(
                "SELECT NULLIF(convert_from(pgp_sym_decrypt(decode($1,'base64'), $2::text), 'utf8'), '')",
            )
            .bind(enc)
            .bind(passphrase)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("DB error decrypting SCALEWAY_SECRET_KEY_ENC: {}", e))?
            .flatten();
            decrypted
        } else {
            plain
        };

        let Some(project_id) = project_id else {
            // No credentials in DB (yet)
            return Ok(None);
        };
        let Some(secret_key) = secret_key else {
            return Ok(None);
        };

        // SSH pubkey remains file/env based (not stored in DB).
        let ssh_public_key_file = env::var("SCALEWAY_SSH_PUBLIC_KEY_FILE")
            .unwrap_or_else(|_| "/app/.ssh/llm-studio-key.pub".to_string());
        let ssh_public_key = fs::read_to_string(&ssh_public_key_file)
            .ok()
            .or_else(|| env::var("SCALEWAY_SSH_PUBLIC_KEY").ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Ok(Some((project_id, secret_key, ssh_public_key)))
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

    pub async fn get_provider(
        provider_name: &str,
        db: Pool<Postgres>,
    ) -> Result<Box<dyn CloudProvider>, String> {
        match provider_name.to_lowercase().as_str() {
            #[cfg(feature = "provider-scaleway")]
            "scaleway" => {
                if let Some((project_id, secret_key, ssh_public_key)) =
                    Self::scaleway_init_from_db(&db).await?
                {
                    return Ok(Box::new(ScalewayProvider::new(
                        project_id,
                        secret_key,
                        ssh_public_key,
                    )));
                }

                // Fallback: env/secrets mode (backwards compat)
                let (project_id, secret_key, ssh_public_key) = Self::scaleway_init_from_env()?;
                Ok(Box::new(ScalewayProvider::new(
                    project_id,
                    secret_key,
                    ssh_public_key,
                )))
            }
            #[cfg(not(feature = "provider-scaleway"))]
            "scaleway" => Err(
                "Scaleway provider is disabled (build without --features provider-scaleway)"
                    .to_string(),
            ),
            #[cfg(feature = "provider-mock")]
            "mock" => Ok(Box::new(MockProvider::new(db))),
            #[cfg(not(feature = "provider-mock"))]
            "mock" => Err("Mock provider is disabled (build without --features provider-mock)".to_string()),
            // Add other providers here:
            // "ovh" => ...
            _ => Err(format!("Unknown provider '{}'", provider_name)),
        }
    }
}
