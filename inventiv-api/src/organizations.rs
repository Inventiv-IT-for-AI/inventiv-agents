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
use crate::rbac;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrganizationRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub role: Option<String>,
    pub member_count: i64,
}

#[derive(Debug, Serialize)]
pub struct OrganizationResponse {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub role: Option<String>,
    pub member_count: i64,
}

impl From<OrganizationRow> for OrganizationResponse {
    fn from(r: OrganizationRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            slug: r.slug,
            created_at: r.created_at,
            role: r.role,
            member_count: r.member_count,
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct OrganizationMemberRow {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct OrganizationMemberResponse {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<OrganizationMemberRow> for OrganizationMemberResponse {
    fn from(r: OrganizationMemberRow) -> Self {
        Self {
            user_id: r.user_id,
            username: r.username,
            email: r.email,
            first_name: r.first_name,
            last_name: r.last_name,
            role: r.role,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SetMemberRoleRequest {
    pub role: String,
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

async fn get_membership_role(
    db: &Pool<Postgres>,
    org_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Option<rbac::OrgRole> {
    let role: Option<String> = sqlx::query_scalar(
        r#"
        SELECT role
        FROM organization_memberships
        WHERE organization_id = $1 AND user_id = $2
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    role.and_then(|r| rbac::OrgRole::parse(&r))
}

async fn count_owners(db: &Pool<Postgres>, org_id: uuid::Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM organization_memberships
        WHERE organization_id = $1 AND role = 'owner'
        "#,
    )
    .bind(org_id)
    .fetch_one(db)
    .await
    .unwrap_or(0)
}

async fn insert_audit_action(
    db: &Pool<Postgres>,
    action_type: &str,
    actor_user_id: uuid::Uuid,
    request_payload: serde_json::Value,
    response_payload: Option<serde_json::Value>,
) {
    // Best-effort: never fail the business operation because of audit logging.
    let _ = sqlx::query(
        r#"
        INSERT INTO action_logs (id, action_type, component, status, user_id, request_payload, response_payload, created_at, completed_at, duration_ms)
        VALUES (gen_random_uuid(), $1, 'api', 'success', $2, $3, $4, NOW(), NOW(), 0)
        "#,
    )
    .bind(action_type)
    .bind(actor_user_id.to_string())
    .bind(request_payload)
    .bind(response_payload)
    .execute(db)
    .await;
}

pub async fn list_organizations(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let rows: Vec<OrganizationRow> = sqlx::query_as(
        r#"
        SELECT 
            o.id, 
            o.name, 
            o.slug, 
            o.created_at, 
            om.role,
            (SELECT COUNT(*) FROM organization_memberships om2 WHERE om2.organization_id = o.id)::bigint as member_count
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

    // Get member count for the new organization
    let member_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint
        FROM organization_memberships
        WHERE organization_id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(1); // At least 1 (the creator)

    // If we set it as current, re-issue JWT cookie so the org context is available immediately.
    let mut resp = Json(OrganizationResponse {
        id,
        name,
        slug,
        created_at,
        role: Some("owner".to_string()),
        member_count,
    })
    .into_response();

    if set_as_current {
        // Get user's role in the new organization
        let org_role = get_membership_role(&state.db, id, user.user_id)
            .await
            .map(|r| r.as_str().to_string());
        
        // Update session in DB
        let session_id = uuid::Uuid::parse_str(&user.session_id)
            .unwrap_or_else(|_| uuid::Uuid::new_v4());
        if let Err(e) = auth::update_session_org(&state.db, session_id, Some(id), org_role.clone()).await {
            tracing::error!("Failed to update session org: {}", e);
        } else {
            // Regenerate JWT with new org context
            let auth_user = auth::AuthUser {
                user_id: user.user_id,
                email: user.email.clone(),
                role: user.role.clone(),
                session_id: user.session_id.clone(),
                current_organization_id: Some(id),
                current_organization_role: org_role,
            };
            if let Ok(tok) = auth::sign_session_jwt(&auth_user) {
                let token_hash = auth::hash_session_token(&tok);
                if let Err(e) = auth::update_session_token_hash(&state.db, session_id, &token_hash).await {
                    tracing::error!("Failed to update session token hash: {}", e);
                }
                resp.headers_mut()
                    .insert(header::SET_COOKIE, auth::session_cookie_value(&tok));
            }
        }
    }

    resp
}

pub async fn set_current_organization(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Json(req): Json<SetCurrentOrganizationRequest>,
) -> impl IntoResponse {
    let session_id = match uuid::Uuid::parse_str(&user.session_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_session"})),
            )
                .into_response();
        }
    };

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

        // Resolve organization role
        let org_role = get_membership_role(&state.db, org_id, user.user_id)
            .await
            .map(|r| r.as_str().to_string());

        // Update session in DB
        if let Err(e) = auth::update_session_org(&state.db, session_id, Some(org_id), org_role.clone()).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response();
        }

        // Regenerate JWT with new org context
        let auth_user = auth::AuthUser {
            user_id: user.user_id,
            email: user.email.clone(),
            role: user.role.clone(),
            session_id: user.session_id.clone(),
            current_organization_id: Some(org_id),
            current_organization_role: org_role,
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

        // Update token hash in DB
        let token_hash = auth::hash_session_token(&token);
        if let Err(e) = auth::update_session_token_hash(&state.db, session_id, &token_hash).await {
            tracing::error!("Failed to update session token hash: {}", e);
        }

        let cookie = auth::session_cookie_value(&token);
        let mut resp = Json(json!({"status":"ok","current_organization_id": org_id}))
            .into_response();
        resp.headers_mut().insert(header::SET_COOKIE, cookie);
        return resp;
    }

    // 2) Clear org selection -> personal mode
    // Update session in DB
    if let Err(e) = auth::update_session_org(&state.db, session_id, None, None).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response();
    }

    // Regenerate JWT without org context
    let auth_user = auth::AuthUser {
        user_id: user.user_id,
        email: user.email.clone(),
        role: user.role.clone(),
        session_id: user.session_id.clone(),
        current_organization_id: None,
        current_organization_role: None,
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

    // Update token hash in DB
    let token_hash = auth::hash_session_token(&token);
    if let Err(e) = auth::update_session_token_hash(&state.db, session_id, &token_hash).await {
        tracing::error!("Failed to update session token hash: {}", e);
    }

    let cookie = auth::session_cookie_value(&token);
    let mut resp = Json(json!({"status":"ok","current_organization_id": null}))
        .into_response();
    resp.headers_mut().insert(header::SET_COOKIE, cookie);
    resp
}

pub async fn list_current_organization_members(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let Some(org_id) = user.current_organization_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request","message":"no_current_organization"})),
        )
            .into_response();
    };

    let ok = is_member(&state.db, org_id, user.user_id).await;
    if !ok {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"forbidden","message":"not_a_member"})),
        )
            .into_response();
    }

    let rows: Vec<OrganizationMemberRow> = sqlx::query_as(
        r#"
        SELECT u.id AS user_id,
               u.username,
               u.email,
               u.first_name,
               u.last_name,
               om.role,
               om.created_at
        FROM organization_memberships om
        JOIN users u ON u.id = om.user_id
        WHERE om.organization_id = $1
        ORDER BY
          CASE om.role
            WHEN 'owner' THEN 0
            WHEN 'admin' THEN 1
            WHEN 'manager' THEN 2
            ELSE 3
          END,
          om.created_at ASC
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows.into_iter().map(OrganizationMemberResponse::from).collect::<Vec<_>>()).into_response()
}

pub async fn set_current_organization_member_role(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    axum::extract::Path(member_user_id): axum::extract::Path<uuid::Uuid>,
    Json(req): Json<SetMemberRoleRequest>,
) -> impl IntoResponse {
    let Some(org_id) = user.current_organization_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request","message":"no_current_organization"})),
        )
            .into_response();
    };

    let Some(actor_role) = get_membership_role(&state.db, org_id, user.user_id).await else {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"forbidden","message":"not_a_member"})),
        )
            .into_response();
    };

    let Some(target_role) = get_membership_role(&state.db, org_id, member_user_id).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"not_found","message":"member_not_found"})),
        )
            .into_response();
    };

    let Some(new_role) = rbac::OrgRole::parse(&req.role) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request","message":"invalid_role"})),
        )
            .into_response();
    };

    if !rbac::can_assign_role(actor_role, target_role, new_role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"forbidden","message":"role_change_not_allowed"})),
        )
            .into_response();
    }

    // Invariant: last owner cannot be changed/downgraded.
    if target_role == rbac::OrgRole::Owner && new_role != rbac::OrgRole::Owner {
        let owners = count_owners(&state.db, org_id).await;
        if owners <= 1 {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error":"conflict","message":"last_owner_cannot_be_changed"})),
            )
                .into_response();
        }
    }

    let res = sqlx::query(
        r#"
        UPDATE organization_memberships
        SET role = $3
        WHERE organization_id = $1 AND user_id = $2
        "#,
    )
    .bind(org_id)
    .bind(member_user_id)
    .bind(new_role.as_str())
    .execute(&state.db)
    .await;

    if let Err(e) = res {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response();
    }

    insert_audit_action(
        &state.db,
        "ORG_MEMBER_ROLE_UPDATED",
        user.user_id,
        json!({
            "organization_id": org_id,
            "member_user_id": member_user_id,
            "from_role": target_role.as_str(),
            "to_role": new_role.as_str(),
        }),
        None,
    )
    .await;

    Json(json!({"status":"ok","member_user_id":member_user_id,"role":new_role.as_str()})).into_response()
}

pub async fn remove_current_organization_member(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    axum::extract::Path(member_user_id): axum::extract::Path<uuid::Uuid>,
) -> impl IntoResponse {
    let Some(org_id) = user.current_organization_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request","message":"no_current_organization"})),
        )
            .into_response();
    };

    let Some(actor_role) = get_membership_role(&state.db, org_id, user.user_id).await else {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"forbidden","message":"not_a_member"})),
        )
            .into_response();
    };
    let Some(target_role) = get_membership_role(&state.db, org_id, member_user_id).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"not_found","message":"member_not_found"})),
        )
            .into_response();
    };

    // Allow self-leave for any role (subject to last-owner invariant).
    let is_self = member_user_id == user.user_id;

    // Permission to remove:
    // - Owner: anyone (except last-owner invariant)
    // - Admin: admin/user
    // - Manager: manager/user
    // - User: only self (leave)
    let allowed = if is_self {
        true
    } else {
        match (actor_role, target_role) {
            (rbac::OrgRole::Owner, _) => true,
            (rbac::OrgRole::Admin, rbac::OrgRole::Admin | rbac::OrgRole::User) => true,
            (rbac::OrgRole::Manager, rbac::OrgRole::Manager | rbac::OrgRole::User) => true,
            _ => false,
        }
    };

    if !allowed {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"forbidden","message":"member_remove_not_allowed"})),
        )
            .into_response();
    }

    // Invariant: last owner cannot be removed (including self leave).
    if target_role == rbac::OrgRole::Owner {
        let owners = count_owners(&state.db, org_id).await;
        if owners <= 1 {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error":"conflict","message":"last_owner_cannot_be_removed"})),
            )
                .into_response();
        }
    }

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

    let res = sqlx::query(
        r#"
        DELETE FROM organization_memberships
        WHERE organization_id = $1 AND user_id = $2
        "#,
    )
    .bind(org_id)
    .bind(member_user_id)
    .execute(&mut *tx)
    .await;

    if let Err(e) = res {
        let _ = tx.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response();
    }

    // If user removed is currently in this org workspace, clear their selection to avoid dangling scope.
    let _ = sqlx::query(
        r#"
        UPDATE users
        SET current_organization_id = NULL,
            updated_at = NOW()
        WHERE id = $1 AND current_organization_id = $2
        "#,
    )
    .bind(member_user_id)
    .bind(org_id)
    .execute(&mut *tx)
    .await;

    if let Err(e) = tx.commit().await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response();
    }

    insert_audit_action(
        &state.db,
        "ORG_MEMBER_REMOVED",
        user.user_id,
        json!({
            "organization_id": org_id,
            "member_user_id": member_user_id,
            "member_role": target_role.as_str(),
            "is_self": is_self,
        }),
        None,
    )
    .await;

    Json(json!({"status":"ok"})).into_response()
}

pub async fn leave_current_organization(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    // Just call the remove endpoint logic for self.
    let member_user_id = user.user_id;
    remove_current_organization_member(
        State(state),
        axum::extract::Extension(user),
        axum::extract::Path(member_user_id),
    )
    .await
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
            session_id: uuid::Uuid::new_v4().to_string(),
            current_organization_id: None,
            current_organization_role: None,
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

    #[tokio::test]
    async fn last_owner_cannot_be_downgraded_or_removed() {
        let Some(pool) = setup_pool().await else {
            return;
        };

        let owner_id = uuid::Uuid::new_v4();
        let other_id = uuid::Uuid::new_v4();
        let org_id = uuid::Uuid::new_v4();

        // Create users
        let _ = sqlx::query(
            r#"
            INSERT INTO users (id, username, email, password_hash, role)
            VALUES ($1, 'owner1', 'owner1@inventiv.local', 'x', 'user')
            "#,
        )
        .bind(owner_id)
        .execute(&pool)
        .await;

        let _ = sqlx::query(
            r#"
            INSERT INTO users (id, username, email, password_hash, role)
            VALUES ($1, 'user2', 'user2@inventiv.local', 'x', 'user')
            "#,
        )
        .bind(other_id)
        .execute(&pool)
        .await;

        // Create org
        let _ = sqlx::query(
            r#"
            INSERT INTO organizations (id, name, slug, created_by_user_id)
            VALUES ($1, 'Org T', 'org-t', $2)
            ON CONFLICT (slug) DO UPDATE SET updated_at = NOW()
            "#,
        )
        .bind(org_id)
        .bind(owner_id)
        .execute(&pool)
        .await;

        // Memberships: owner + user
        let _ = sqlx::query(
            r#"
            INSERT INTO organization_memberships (organization_id, user_id, role)
            VALUES ($1, $2, 'owner')
            ON CONFLICT (organization_id, user_id) DO UPDATE SET role = 'owner'
            "#,
        )
        .bind(org_id)
        .bind(owner_id)
        .execute(&pool)
        .await;

        let _ = sqlx::query(
            r#"
            INSERT INTO organization_memberships (organization_id, user_id, role)
            VALUES ($1, $2, 'user')
            ON CONFLICT (organization_id, user_id) DO UPDATE SET role = 'user'
            "#,
        )
        .bind(org_id)
        .bind(other_id)
        .execute(&pool)
        .await;

        let state = Arc::new(crate::AppState {
            redis_client: redis::Client::open("redis://127.0.0.1/").unwrap(),
            db: pool.clone(),
        });
        let auth_user = auth::AuthUser {
            user_id: owner_id,
            email: "owner1@inventiv.local".to_string(),
            role: "user".to_string(),
            session_id: uuid::Uuid::new_v4().to_string(),
            current_organization_id: Some(org_id),
            current_organization_role: Some("owner".to_string()),
        };

        // Try to downgrade last owner -> conflict
        let resp = set_current_organization_member_role(
            AxumState(state.clone()),
            Extension(auth_user.clone()),
            axum::extract::Path(owner_id),
            Json(SetMemberRoleRequest {
                role: "admin".to_string(),
            }),
        )
        .await
        .into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        // Try to remove last owner -> conflict
        let resp = remove_current_organization_member(
            AxumState(state.clone()),
            Extension(auth_user.clone()),
            axum::extract::Path(owner_id),
        )
        .await
        .into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        // Add second owner then downgrade/remove should succeed
        let _ = sqlx::query(
            r#"
            INSERT INTO organization_memberships (organization_id, user_id, role)
            VALUES ($1, $2, 'owner')
            ON CONFLICT (organization_id, user_id) DO UPDATE SET role = 'owner'
            "#,
        )
        .bind(org_id)
        .bind(other_id)
        .execute(&pool)
        .await;

        let resp = set_current_organization_member_role(
            AxumState(state.clone()),
            Extension(auth_user.clone()),
            axum::extract::Path(owner_id),
            Json(SetMemberRoleRequest {
                role: "admin".to_string(),
            }),
        )
        .await
        .into_response();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = remove_current_organization_member(
            AxumState(state),
            Extension(auth_user),
            axum::extract::Path(owner_id),
        )
        .await
        .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}


