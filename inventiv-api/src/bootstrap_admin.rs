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

    // If already exists, nothing to do.
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
        .bind(&username)
        .fetch_one(db)
        .await
        .unwrap_or(false);
    if exists {
        return;
    }

    let Some(password) = default_admin_password() else {
        eprintln!(
            "[warn] BOOTSTRAP_DEFAULT_ADMIN enabled but no DEFAULT_ADMIN_PASSWORD_FILE (or unreadable file); skipping admin creation"
        );
        return;
    };

    // Create admin user. If another replica races, ON CONFLICT DO NOTHING makes it safe.
    let _ = sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, role, first_name, last_name, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, crypt($3, gen_salt('bf')), 'admin', $4, $5, NOW(), NOW())
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(&username)
    .bind(&email)
    .bind(&password)
    .bind(&first_name)
    .bind(&last_name)
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


