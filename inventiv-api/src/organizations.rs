use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::{auth, AppState};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrganizationRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrganizationResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub role: Option<String>,
}

impl From<OrganizationRow> for OrganizationResponse {
    fn from(r: OrganizationRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            slug: r.slug,
            created_at: r.created_at,
            role: r.role,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateOrganizationRequest {
    pub name: String,
    pub slug: Option<String>,
    pub set_as_current: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SetCurrentOrganizationRequest {
    /// When null, switches back to "personal" mode (no org selected).
    pub organization_id: Option<uuid::Uuid>,
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.trim().to_ascii_lowercase().chars() {
        let is_alnum = ch.is_ascii_alphanumeric();
        if is_alnum {
            out.push(ch);
            last_dash = false;
            continue;
        }
        let is_sep = ch.is_ascii_whitespace() || ch == '_' || ch == '-' || ch == '.';
        if is_sep && !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn normalize_slug(req_slug: Option<String>, name: &str, fallback_uuid: uuid::Uuid) -> String {
    let raw = req_slug.unwrap_or_else(|| name.to_string());
    let mut s = slugify(&raw);
    if s.is_empty() {
        s = format!("org-{}", &fallback_uuid.to_string()[..8]);
    }
    // Keep slugs reasonably short for URLs.
    if s.len() > 64 {
        s.truncate(64);
        while s.ends_with('-') {
            s.pop();
        }
        if s.is_empty() {
            s = format!("org-{}", &fallback_uuid.to_string()[..8]);
        }
    }
    s
}

async fn is_member(db: &Pool<Postgres>, org_id: uuid::Uuid, user_id: uuid::Uuid) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM organization_memberships
          WHERE organization_id = $1 AND user_id = $2
        )
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(false)
}

pub async fn list_organizations(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let rows: Vec<OrganizationRow> = sqlx::query_as(
        r#"
        SELECT o.id, o.name, o.slug, o.created_at, om.role
        FROM organizations o
        JOIN organization_memberships om ON om.organization_id = o.id
        WHERE om.user_id = $1
        ORDER BY o.created_at DESC
        "#,
    )
    .bind(user.user_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows.into_iter().map(OrganizationResponse::from).collect::<Vec<_>>()).into_response()
}

pub async fn create_organization(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Json(req): Json<CreateOrganizationRequest>,
) -> impl IntoResponse {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request","message":"name_required"})),
        )
            .into_response();
    }

    let org_id = uuid::Uuid::new_v4();
    let slug = normalize_slug(req.slug, &name, org_id);
    let set_as_current = req.set_as_current.unwrap_or(true);

    let mut tx = match state.db.begin().await {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response();
        }
    };

    let inserted: Result<(uuid::Uuid, String, String, chrono::DateTime<chrono::Utc>), sqlx::Error> =
        sqlx::query_as(
            r#"
            INSERT INTO organizations (id, name, slug, created_by_user_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            RETURNING id, name, slug, created_at
            "#,
        )
        .bind(org_id)
        .bind(&name)
        .bind(&slug)
        .bind(user.user_id)
        .fetch_one(&mut *tx)
        .await;

    let (id, name, slug, created_at) = match inserted {
        Ok(v) => v,
        Err(e) => {
            let code = match &e {
                sqlx::Error::Database(db) => db.code().map(|c| c.to_string()),
                _ => None,
            };
            let _ = tx.rollback().await;
            if code.as_deref() == Some("23505") {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({"error":"conflict","message":"organization_slug_already_exists"})),
                )
                    .into_response();
            }
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response();
        }
    };

    // Create membership as owner
    let _ = sqlx::query(
        r#"
        INSERT INTO organization_memberships (organization_id, user_id, role, created_at)
        VALUES ($1, $2, 'owner', NOW())
        ON CONFLICT (organization_id, user_id) DO NOTHING
        "#,
    )
    .bind(id)
    .bind(user.user_id)
    .execute(&mut *tx)
    .await;

    if set_as_current {
        let _ = sqlx::query(
            r#"
            UPDATE users
            SET current_organization_id = $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(user.user_id)
        .bind(id)
        .execute(&mut *tx)
        .await;
    }

    if let Err(e) = tx.commit().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response();
    }

    // If we set it as current, re-issue JWT cookie so the org context is available immediately.
    let mut resp = Json(OrganizationResponse {
        id,
        name,
        slug,
        created_at,
        role: Some("owner".to_string()),
    })
    .into_response();

    if set_as_current {
        let auth_user = auth::AuthUser {
            user_id: user.user_id,
            email: user.email.clone(),
            role: user.role.clone(),
            current_organization_id: Some(id),
        };
        if let Ok(tok) = auth::sign_session_jwt(&auth_user) {
            resp.headers_mut()
                .insert(header::SET_COOKIE, auth::session_cookie_value(&tok));
        }
    }

    resp
}

pub async fn set_current_organization(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Json(req): Json<SetCurrentOrganizationRequest>,
) -> impl IntoResponse {
    // 1) Set a concrete org (requires membership)
    if let Some(org_id) = req.organization_id {
        let ok = is_member(&state.db, org_id, user.user_id).await;
        if !ok {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error":"forbidden","message":"not_a_member"})),
            )
                .into_response();
        }

        let res = sqlx::query(
            r#"
            UPDATE users
            SET current_organization_id = $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(user.user_id)
        .bind(org_id)
        .execute(&state.db)
        .await;

        if let Err(e) = res {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response();
        }

        let auth_user = auth::AuthUser {
            user_id: user.user_id,
            email: user.email.clone(),
            role: user.role.clone(),
            current_organization_id: Some(org_id),
        };
        let token = match auth::sign_session_jwt(&auth_user) {
            Ok(t) => t,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error":"token_sign_failed","message": e.to_string()})),
                )
                    .into_response();
            }
        };

        let cookie = auth::session_cookie_value(&token);
        let mut resp = Json(json!({"status":"ok","current_organization_id": org_id}))
            .into_response();
        resp.headers_mut().insert(header::SET_COOKIE, cookie);
        return resp;
    }

    // 2) Clear org selection -> personal mode
    let res = sqlx::query(
        r#"
        UPDATE users
        SET current_organization_id = NULL,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(user.user_id)
    .execute(&state.db)
    .await;

    if let Err(e) = res {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response();
    }

    let auth_user = auth::AuthUser {
        user_id: user.user_id,
        email: user.email.clone(),
        role: user.role.clone(),
        current_organization_id: None,
    };
    let token = match auth::sign_session_jwt(&auth_user) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"token_sign_failed","message": e.to_string()})),
            )
                .into_response();
        }
    };

    let cookie = auth::session_cookie_value(&token);
    let mut resp = Json(json!({"status":"ok","current_organization_id": null}))
        .into_response();
    resp.headers_mut().insert(header::SET_COOKIE, cookie);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::extract::{State as AxumState};
    use axum::http::StatusCode;
    use axum::Extension;
    use sqlx::postgres::PgPoolOptions;

    fn test_database_url() -> Option<String> {
        std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
    }

    async fn setup_pool() -> Option<Pool<Postgres>> {
        let Some(url) = test_database_url() else {
            eprintln!("skipping integration test: DATABASE_URL not set");
            return None;
        };
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .ok()?;

        // Ensure schema/migrations exist (safe to re-run in dev DB; in CI prefer a dedicated DB).
        let _ = sqlx::migrate!("../sqlx-migrations").run(&pool).await;
        Some(pool)
    }

    #[tokio::test]
    async fn admin_is_owner_of_all_orgs_migration_is_idempotent() {
        let Some(pool) = setup_pool().await else {
            return;
        };

        // Create an admin user (if not exists)
        let admin_id: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO users (id, username, email, password_hash, role)
            VALUES (gen_random_uuid(), 'admin', 'admin@inventiv.local', 'x', 'admin')
            ON CONFLICT (username) DO UPDATE SET email = EXCLUDED.email
            RETURNING id
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("admin upsert");

        // Create two orgs
        let org_a: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO organizations (id, name, slug, created_by_user_id)
            VALUES (gen_random_uuid(), 'Org A', 'org-a', $1)
            ON CONFLICT (slug) DO UPDATE SET updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(admin_id)
        .fetch_one(&pool)
        .await
        .expect("org a");

        let org_b: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO organizations (id, name, slug, created_by_user_id)
            VALUES (gen_random_uuid(), 'Org B', 'org-b', $1)
            ON CONFLICT (slug) DO UPDATE SET updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(admin_id)
        .fetch_one(&pool)
        .await
        .expect("org b");

        // Ensure admin membership is not owner initially (simulate)
        let _ = sqlx::query(
            r#"
            INSERT INTO organization_memberships (organization_id, user_id, role)
            VALUES ($1, $2, 'user')
            ON CONFLICT (organization_id, user_id) DO UPDATE SET role = 'user'
            "#,
        )
        .bind(org_a)
        .bind(admin_id)
        .execute(&pool)
        .await;

        // Execute the migration SQL manually (idempotent)
        let mig_sql =
            include_str!("../../sqlx-migrations/20251218032000_admin_owner_all_organizations.sql");
        sqlx::query(mig_sql)
            .execute(&pool)
            .await
            .expect("migration execution");
        // Run again to prove idempotence
        sqlx::query(mig_sql)
            .execute(&pool)
            .await
            .expect("migration execution 2");

        // Verify membership role is owner for both orgs
        let roles: Vec<(uuid::Uuid, String)> = sqlx::query_as(
            r#"
            SELECT organization_id, role
            FROM organization_memberships
            WHERE user_id = $1
              AND organization_id IN ($2, $3)
            ORDER BY organization_id
            "#,
        )
        .bind(admin_id)
        .bind(org_a)
        .bind(org_b)
        .fetch_all(&pool)
        .await
        .expect("roles fetch");

        assert_eq!(roles.len(), 2);
        assert!(roles.iter().all(|(_, r)| r == "owner"));

        // Also verify the handler would list both orgs for admin (because membership exists).
        let state = Arc::new(crate::AppState {
            redis_client: redis::Client::open("redis://127.0.0.1/").unwrap(),
            db: pool.clone(),
        });
        let auth_user = auth::AuthUser {
            user_id: admin_id,
            email: "admin@inventiv.local".to_string(),
            role: "admin".to_string(),
            current_organization_id: None,
        };

        let resp = list_organizations(AxumState(state), Extension(auth_user))
            .await
            .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let arr = v.as_array().cloned().unwrap_or_default();
        // at least those two slugs
        let slugs: Vec<String> = arr
            .iter()
            .filter_map(|o| o.get("slug").and_then(|s| s.as_str()).map(|s| s.to_string()))
            .collect();
        assert!(slugs.contains(&"org-a".to_string()));
        assert!(slugs.contains(&"org-b".to_string()));
    }
}


