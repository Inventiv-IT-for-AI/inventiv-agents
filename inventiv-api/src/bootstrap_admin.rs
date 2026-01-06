use sqlx::{Pool, Postgres};

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
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
    let file = env_string(
        "DEFAULT_ADMIN_PASSWORD_FILE",
        "/run/secrets/default_admin_password",
    );
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
                    updated_at = NOW()
                WHERE username = $5
                "#,
            )
            .bind(&password)
            .bind(&email)
            .bind(&first_name)
            .bind(&last_name)
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

pub async fn ensure_default_organization(db: &Pool<Postgres>) {
    // Allow disabling in special cases, but default is enabled.
    if !env_bool("BOOTSTRAP_DEFAULT_ORGANIZATION", true) {
        return;
    }

    let org_name = env_string("DEFAULT_ORGANIZATION_NAME", "Inventiv IT");
    let org_slug = env_string("DEFAULT_ORGANIZATION_SLUG", "inventiv-it");
    let admin_username = env_string("DEFAULT_ADMIN_USERNAME", "admin").to_ascii_lowercase();

    // Get admin user ID
    let admin_user_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE username = $1 LIMIT 1"
    )
    .bind(&admin_username)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let Some(admin_id) = admin_user_id else {
        eprintln!(
            "[warn] BOOTSTRAP_DEFAULT_ORGANIZATION: admin user '{}' not found; skipping organization creation",
            admin_username
        );
        return;
    };

    // Get or create organization "Inventiv IT" (idempotent)
    let org_id: Option<uuid::Uuid> = sqlx::query_scalar(
        r#"
        INSERT INTO organizations (id, name, slug, created_by_user_id, created_at, updated_at)
        VALUES (gen_random_uuid(), $1, $2, $3, NOW(), NOW())
        ON CONFLICT (slug) DO UPDATE SET updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(&org_name)
    .bind(&org_slug)
    .bind(admin_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let org_uuid = if let Some(id) = org_id {
        id
    } else {
        // Organization already exists, fetch its ID
        match sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM organizations WHERE slug = $1 LIMIT 1"
        )
        .bind(&org_slug)
        .fetch_optional(db)
        .await
        .ok()
        .flatten() {
            Some(id) => id,
            None => {
                eprintln!(
                    "[error] BOOTSTRAP_DEFAULT_ORGANIZATION: failed to get or create organization '{}'",
                    org_slug
                );
                return;
            }
        }
    };

    // Ensure admin is owner of this organization (idempotent)
    // Check if membership already exists
    let membership_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM organization_memberships WHERE organization_id = $1 AND user_id = $2)"
    )
    .bind(org_uuid)
    .bind(admin_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);

    if membership_exists {
        // Update role to owner if needed
        let rows_updated = sqlx::query(
            r#"
            UPDATE organization_memberships
            SET role = 'owner'
            WHERE organization_id = $1 AND user_id = $2 AND role != 'owner'
            "#,
        )
        .bind(org_uuid)
        .bind(admin_id)
        .execute(db)
        .await
        .ok()
        .map(|r| r.rows_affected())
        .unwrap_or(0);

        if rows_updated > 0 {
            eprintln!(
                "[info] Updated admin role to owner for default organization '{}' (slug: {})",
                org_name, org_slug
            );
        } else {
            eprintln!(
                "[info] Default organization '{}' (slug: {}) already exists with admin as owner",
                org_name, org_slug
            );
        }
    } else {
        // Create membership
        let rows_inserted = sqlx::query(
            r#"
            INSERT INTO organization_memberships (organization_id, user_id, role, created_at)
            VALUES ($1, $2, 'owner', NOW())
            "#,
        )
        .bind(org_uuid)
        .bind(admin_id)
        .execute(db)
        .await
        .ok()
        .map(|r| r.rows_affected())
        .unwrap_or(0);

        if rows_inserted > 0 {
            eprintln!(
                "[info] Created default organization '{}' (slug: {}) with admin as owner",
                org_name, org_slug
            );
        }
    }
}
