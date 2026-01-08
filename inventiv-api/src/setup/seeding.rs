use sqlx::Pool;
use sqlx::Postgres;
use std::fs;

/// Optional dev convenience: auto-seed catalog when DB is empty.
/// Guarded by env var to avoid accidental seeding in staging/prod.
pub async fn maybe_seed_catalog(pool: &Pool<Postgres>) {
    let enabled = std::env::var("AUTO_SEED_CATALOG")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    if !enabled {
        return;
    }

    // Important: do NOT skip seeding based on one table (e.g. providers).
    // We want seeding to be re-runnable and idempotent (the seed file should use ON CONFLICT),
    // otherwise partial resets (like TRUNCATE action_types) would leave the UI broken.

    let seed_path = std::env::var("SEED_CATALOG_PATH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "seeds/catalog_seeds.sql".to_string());

    let sql = match fs::read_to_string(&seed_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("⚠️  AUTO_SEED_CATALOG failed to read {}: {}", seed_path, e);
            return;
        }
    };

    // Very simple splitter: seed file is expected to be plain SQL statements separated by ';'
    // and may contain '--' line comments.
    let mut cleaned = String::new();
    for line in sql.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") {
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }

    let statements: Vec<String> = cleaned
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("{};", s))
        .collect();

    if statements.is_empty() {
        eprintln!(
            "⚠️  AUTO_SEED_CATALOG: no statements found in {}",
            seed_path
        );
        return;
    }

    println!(
        "🌱 AUTO_SEED_CATALOG: seeding {} statements from {}",
        statements.len(),
        seed_path
    );
    for (idx, stmt) in statements.iter().enumerate() {
        if let Err(e) = sqlx::query(stmt).execute(pool).await {
            eprintln!(
                "❌ AUTO_SEED_CATALOG failed at statement {}: {}",
                idx + 1,
                e
            );
            return;
        }
    }
    println!("✅ AUTO_SEED_CATALOG done");
}

/// Optional: seed provider credentials from /run/secrets into DB (provider_settings).
/// Guarded by env var; keeps secrets out of git while centralizing config in DB.
pub async fn maybe_seed_provider_credentials(pool: &Pool<Postgres>) {
    let enabled = std::env::var("AUTO_SEED_PROVIDER_CREDENTIALS")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    if !enabled {
        return;
    }

    // We seed only what we can read from runtime secrets/env.
    // Expected runtime:
    // - SCALEWAY_PROJECT_ID in env (non-secret)
    // - /run/secrets/scaleway_secret_key present (secret)
    // - /run/secrets/provider_settings_key present (secret passphrase for pgcrypto)

    let project_id = std::env::var("SCALEWAY_PROJECT_ID")
        .ok()
        .or_else(|| std::env::var("SCW_PROJECT_ID").ok())
        .unwrap_or_default()
        .trim()
        .to_string();
    if project_id.is_empty() {
        eprintln!("⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: missing SCALEWAY_PROJECT_ID; skipping");
        return;
    }

    let secret_key_path = std::env::var("SCALEWAY_SECRET_KEY_FILE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "/run/secrets/scaleway_secret_key".to_string());
    let secret_key = match fs::read_to_string(&secret_key_path) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            eprintln!(
                "⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: failed to read {}: {}",
                secret_key_path, e
            );
            return;
        }
    };
    if secret_key.is_empty() {
        eprintln!("⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: empty scaleway secret key; skipping");
        return;
    }

    let passphrase_path = std::env::var("PROVIDER_SETTINGS_ENCRYPTION_KEY_FILE")
        .ok()
        .or_else(|| std::env::var("PROVIDER_SETTINGS_PASSPHRASE_FILE").ok())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "/run/secrets/provider_settings_key".to_string());
    let passphrase = match fs::read_to_string(&passphrase_path) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            eprintln!(
                "⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: missing encryption key file {}: {}",
                passphrase_path, e
            );
            return;
        }
    };
    if passphrase.is_empty() {
        eprintln!("⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: empty encryption passphrase; skipping");
        return;
    }

    // Resolve provider_id
    let provider_id: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM providers WHERE code = 'scaleway' LIMIT 1")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let Some(provider_id) = provider_id else {
        eprintln!("⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: provider 'scaleway' not found in DB; seed catalog first");
        return;
    };

    // Resolve default organization (from env or default to 'inventiv-it')
    let org_slug = std::env::var("DEFAULT_ORGANIZATION_SLUG")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "inventiv-it".to_string());
    let organization_id: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT id FROM organizations WHERE slug = $1 LIMIT 1")
            .bind(&org_slug)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let Some(organization_id) = organization_id else {
        eprintln!("⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: organization '{}' not found in DB; bootstrap default organization first", org_slug);
        return;
    };

    // Encrypt using pgcrypto (installed by baseline migration).
    let enc_b64: Option<String> =
        sqlx::query_scalar("SELECT encode(pgp_sym_encrypt($1::text, $2::text), 'base64')")
            .bind(&secret_key)
            .bind(&passphrase)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let Some(enc_b64) = enc_b64 else {
        eprintln!("⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: encryption failed; skipping");
        return;
    };

    // Read SCALEWAY_ACCESS_KEY and SCALEWAY_ORGANIZATION_ID from secrets/env
    let access_key_path = std::env::var("SCALEWAY_ACCESS_KEY_FILE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "/run/secrets/scaleway_access_key".to_string());
    let access_key = match fs::read_to_string(&access_key_path) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            eprintln!(
                "⚠️  AUTO_SEED_PROVIDER_CREDENTIALS: failed to read {}: {}",
                access_key_path, e
            );
            String::new()
        }
    };
    let access_key = if !access_key.is_empty() {
        Some(access_key)
    } else {
        std::env::var("SCALEWAY_ACCESS_KEY")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let organization_id_scw = std::env::var("SCALEWAY_ORGANIZATION_ID")
        .ok()
        .or_else(|| std::env::var("SCW_DEFAULT_ORGANIZATION_ID").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    println!(
        "🌱 AUTO_SEED_PROVIDER_CREDENTIALS: upserting scaleway credentials into provider_settings for organization {}",
        organization_id
    );

    // Upsert project id (non-secret) with organization_id.
    let _ = sqlx::query(
        r#"
        INSERT INTO provider_settings (provider_id, organization_id, key, value_text, value_int, value_bool, value_json)
        VALUES ($1, $2, 'SCALEWAY_PROJECT_ID', $3, NULL, NULL, NULL)
        ON CONFLICT (provider_id, key, organization_id) DO UPDATE SET
          value_text = EXCLUDED.value_text,
          value_int = NULL,
          value_bool = NULL,
          value_json = NULL
        "#,
    )
    .bind(provider_id)
    .bind(organization_id)
    .bind(&project_id)
    .execute(pool)
    .await;

    // Upsert encrypted secret key with organization_id.
    let _ = sqlx::query(
        r#"
        INSERT INTO provider_settings (provider_id, organization_id, key, value_text, value_int, value_bool, value_json)
        VALUES ($1, $2, 'SCALEWAY_SECRET_KEY_ENC', $3, NULL, NULL, NULL)
        ON CONFLICT (provider_id, key, organization_id) DO UPDATE SET
          value_text = EXCLUDED.value_text,
          value_int = NULL,
          value_bool = NULL,
          value_json = NULL
        "#,
    )
    .bind(provider_id)
    .bind(organization_id)
    .bind(&enc_b64)
    .execute(pool)
    .await;

    // Upsert SCALEWAY_ACCESS_KEY if available (for CLI operations like volume resize)
    if let Some(ak) = access_key {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, organization_id, key, value_text, value_int, value_bool, value_json)
            VALUES ($1, $2, 'SCALEWAY_ACCESS_KEY', $3, NULL, NULL, NULL)
            ON CONFLICT (provider_id, key, organization_id) DO UPDATE SET
              value_text = EXCLUDED.value_text,
              value_int = NULL,
              value_bool = NULL,
              value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(organization_id)
        .bind(&ak)
        .execute(pool)
        .await;
    }

    // Upsert SCALEWAY_ORGANIZATION_ID if available (for CLI operations like volume resize)
    if let Some(org_id_scw) = organization_id_scw {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, organization_id, key, value_text, value_int, value_bool, value_json)
            VALUES ($1, $2, 'SCALEWAY_ORGANIZATION_ID', $3, NULL, NULL, NULL)
            ON CONFLICT (provider_id, key, organization_id) DO UPDATE SET
              value_text = EXCLUDED.value_text,
              value_int = NULL,
              value_bool = NULL,
              value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(organization_id)
        .bind(&org_id_scw)
        .execute(pool)
        .await;
    }

    println!("✅ AUTO_SEED_PROVIDER_CREDENTIALS done");
}
