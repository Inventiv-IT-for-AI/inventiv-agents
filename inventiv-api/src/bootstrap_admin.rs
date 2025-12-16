use sqlx::{Pool, Postgres};

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(default)
}

fn env_string(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn read_secret_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn default_admin_password() -> Option<String> {
    let file = env_string("DEFAULT_ADMIN_PASSWORD_FILE", "/run/secrets/default_admin_password");
    read_secret_file(&file)
}

pub async fn ensure_default_admin(db: &Pool<Postgres>) {
    // Allow disabling in special cases, but default is enabled.
    if !env_bool("BOOTSTRAP_DEFAULT_ADMIN", true) {
        return;
    }

    let username = env_string("DEFAULT_ADMIN_USERNAME", "admin").to_ascii_lowercase();
    let email = env_string("DEFAULT_ADMIN_EMAIL", "admin@inventiv.local").to_ascii_lowercase();
    let first_name = env_string("DEFAULT_ADMIN_FIRST_NAME", "Admin");
    let last_name = env_string("DEFAULT_ADMIN_LAST_NAME", "User");
    // Locale for the bootstrap admin account (BCP47).
    // Default: fr-FR (as requested).
    let locale_code = env_string("DEFAULT_ADMIN_LOCALE", "fr-FR");
    let update_password = env_bool("BOOTSTRAP_UPDATE_ADMIN_PASSWORD", false);

    let Some(password) = default_admin_password() else {
        eprintln!(
            "[warn] BOOTSTRAP_DEFAULT_ADMIN enabled but no DEFAULT_ADMIN_PASSWORD_FILE (or unreadable file); skipping admin creation"
        );
        return;
    };

    // Check if admin user already exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
        .bind(&username)
        .fetch_one(db)
        .await
        .unwrap_or(false);

    if exists {
        // If update_password is enabled, update the password hash
        if update_password {
            let rows_updated = sqlx::query(
                r#"
                UPDATE users
                SET password_hash = crypt($1, gen_salt('bf')),
                    email = COALESCE(NULLIF($2, ''), email),
                    first_name = COALESCE(NULLIF($3, ''), first_name),
                    last_name = COALESCE(NULLIF($4, ''), last_name),
                    locale_code = COALESCE(NULLIF($5, ''), locale_code),
                    updated_at = NOW()
                WHERE username = $6
                "#,
            )
            .bind(&password)
            .bind(&email)
            .bind(&first_name)
            .bind(&last_name)
            .bind(&locale_code)
            .bind(&username)
            .execute(db)
            .await
            .map(|r| r.rows_affected())
            .unwrap_or(0);

            if rows_updated > 0 {
                eprintln!("[info] Updated admin user password and profile");
            }
        }
        return;
    }

    // Create admin user. If another replica races, ON CONFLICT DO NOTHING makes it safe.
    let _ = sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, role, first_name, last_name, locale_code, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, crypt($3, gen_salt('bf')), 'admin', $4, $5, $6, NOW(), NOW())
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(&username)
    .bind(&email)
    .bind(&password)
    .bind(&first_name)
    .bind(&last_name)
    .bind(&locale_code)
    .execute(db)
    .await;

    // Optional: if there is an existing admin row without username (legacy), assign it.
    let _ = sqlx::query(
        r#"
        UPDATE users
        SET username = $1, updated_at = NOW()
        WHERE username IS NULL
          AND role = 'admin'
          AND NOT EXISTS (SELECT 1 FROM users WHERE username = $1)
        "#,
    )
    .bind(&username)
    .execute(db)
    .await;
}


