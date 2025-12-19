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


