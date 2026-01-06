use axum::{
    extract::Query,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Postgres, QueryBuilder};
use std::sync::Arc;
use uuid::Uuid;

use crate::{auth, AppState};

#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String, // User's global role (admin, user, etc.)
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    // Organization context (nullable - user may not be in any org)
    pub organization_id: Option<Uuid>,
    pub organization_name: Option<String>,
    pub organization_slug: Option<String>,
    pub organization_role: Option<String>, // Role in the organization (owner, admin, manager, user)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateUserRequest {
    pub username: Option<String>,
    pub email: String,
    pub password: String,
    pub role: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct UsersSearchQuery {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    /// Search in username/email (ILIKE).
    pub q: Option<String>,
    /// Filter by organization ID (UUID)
    pub organization_id: Option<Uuid>,
    /// Filter by organization role (owner|admin|manager|user)
    pub organization_role: Option<String>,
    /// Sort field allowlist: username|email|role|created_at|updated_at|organization_name|organization_role
    pub sort_by: Option<String>,
    /// "asc" | "desc"
    pub sort_dir: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct UsersSearchResponse {
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub filtered_count: i64,
    pub rows: Vec<UserResponse>,
}

fn dir_sql(dir: Option<&str>) -> &'static str {
    match dir.unwrap_or("asc").to_ascii_lowercase().as_str() {
        "desc" => "DESC",
        _ => "ASC",
    }
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    if let Err(e) = auth::require_admin(&user) {
        return e.into_response();
    }

    let rows: Vec<UserResponse> = sqlx::query_as(
        r#"
        SELECT id, username, email, role, first_name, last_name, created_at, updated_at
        FROM users
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows).into_response()
}

#[utoipa::path(
    get,
    path = "/users/search",
    tag = "Users",
    params(UsersSearchQuery),
    responses((status = 200, description = "Search users (admin)", body = UsersSearchResponse))
)]
pub async fn search_users(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Query(params): Query<UsersSearchQuery>,
) -> impl IntoResponse {
    if let Err(e) = auth::require_admin(&user) {
        return e.into_response();
    }

    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);
    let q_like: Option<String> = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s));
    
    let org_role_filter: Option<String> = params
        .organization_role
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());

    // Count total users (distinct)
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT id) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    // Count filtered user-organization pairs
    let mut count_qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT COUNT(*)
        FROM users u
        LEFT JOIN organization_memberships om ON om.user_id = u.id
        LEFT JOIN organizations o ON o.id = om.organization_id
        WHERE 1=1
        "#,
    );
    if q_like.is_some() {
        count_qb.push(" AND (u.username ILIKE ");
        count_qb.push_bind(q_like.as_deref());
        count_qb.push(" OR u.email ILIKE ");
        count_qb.push_bind(q_like.as_deref());
        count_qb.push(")");
    }
    if let Some(org_id) = params.organization_id {
        count_qb.push(" AND o.id = ");
        count_qb.push_bind(org_id);
    }
    if org_role_filter.is_some() {
        count_qb.push(" AND om.role = ");
        count_qb.push_bind(org_role_filter.as_deref());
    }
    
    let filtered_count: i64 = count_qb
        .build_query_scalar()
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let order_by = match params.sort_by.as_deref() {
        Some("email") => "u.email",
        Some("role") => "u.role",
        Some("created_at") => "u.created_at",
        Some("updated_at") => "u.updated_at",
        Some("organization_name") => "o.name",
        Some("organization_role") => "om.role",
        _ => "u.username",
    };
    let dir = dir_sql(params.sort_dir.as_deref());

    // Build main query: one row per user-organization pair
    // Users without organizations appear once with NULL organization fields
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT 
            u.id,
            u.username,
            u.email,
            u.role,
            u.first_name,
            u.last_name,
            u.created_at,
            u.updated_at,
            o.id as organization_id,
            o.name as organization_name,
            o.slug as organization_slug,
            om.role as organization_role
        FROM users u
        LEFT JOIN organization_memberships om ON om.user_id = u.id
        LEFT JOIN organizations o ON o.id = om.organization_id
        WHERE 1=1
        "#,
    );
    if q_like.is_some() {
        qb.push(" AND (u.username ILIKE ");
        qb.push_bind(q_like.as_deref());
        qb.push(" OR u.email ILIKE ");
        qb.push_bind(q_like.as_deref());
        qb.push(")");
    }
    if let Some(org_id) = params.organization_id {
        qb.push(" AND o.id = ");
        qb.push_bind(org_id);
    }
    if org_role_filter.is_some() {
        qb.push(" AND om.role = ");
        qb.push_bind(org_role_filter.as_deref());
    }
    qb.push(" ORDER BY ");
    qb.push(order_by);
    qb.push(" ");
    qb.push(dir);
    qb.push(", u.id ");
    qb.push(dir);
    qb.push(", o.id ");
    qb.push(dir);
    qb.push(" LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    let rows: Vec<UserResponse> = qb
        .build_query_as()
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(UsersSearchResponse {
        offset,
        limit,
        total_count,
        filtered_count,
        rows,
    })
    .into_response()
}

pub async fn get_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(e) = auth::require_admin(&user) {
        return e.into_response();
    }

    let row: Option<UserResponse> = sqlx::query_as(
        r#"
        SELECT id, username, email, role, first_name, last_name, created_at, updated_at
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(u) => Json(u).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response(),
    }
}

pub async fn create_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    if let Err(e) = auth::require_admin(&user) {
        return e.into_response();
    }

    let email = req.email.trim().to_ascii_lowercase();
    let username = req
        .username
        .map(|u| u.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| email.clone());
    let password = req.password;
    if email.is_empty() || password.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request"})),
        )
            .into_response();
    }
    let role = req
        .role
        .unwrap_or_else(|| "admin".to_string())
        .trim()
        .to_string();

    let id = Uuid::new_v4();
    let row: Option<UserResponse> = sqlx::query_as(
        r#"
        INSERT INTO users (id, username, email, password_hash, role, first_name, last_name, created_at, updated_at)
        VALUES ($1, $2, $3, crypt($4, gen_salt('bf')), $5, $6, $7, NOW(), NOW())
        ON CONFLICT DO NOTHING
        RETURNING id, username, email, role, first_name, last_name, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(username)
    .bind(email)
    .bind(password)
    .bind(role)
    .bind(req.first_name)
    .bind(req.last_name)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(u) => (StatusCode::CREATED, Json(u)).into_response(),
        None => (
            StatusCode::CONFLICT,
            Json(json!({"error":"email_already_exists"})),
        )
            .into_response(),
    }
}

pub async fn update_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> impl IntoResponse {
    if let Err(e) = auth::require_admin(&user) {
        return e.into_response();
    }

    let username = req
        .username
        .map(|u| u.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let email = req
        .email
        .map(|e| e.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let role = req
        .role
        .map(|r| r.trim().to_string())
        .filter(|s| !s.is_empty());

    // Update base fields
    let res = sqlx::query(
        r#"
        UPDATE users
        SET username = COALESCE($2, username),
            email = COALESCE($3, email),
            role = COALESCE($4, role),
            first_name = COALESCE($5, first_name),
            last_name = COALESCE($6, last_name),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(username)
    .bind(email)
    .bind(role)
    .bind(req.first_name)
    .bind(req.last_name)
    .execute(&state.db)
    .await;

    match res {
        Ok(r) if r.rows_affected() == 0 => {
            return (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response()
        }
        _ => {}
    }

    // Update password if provided
    if let Some(pw) = req.password {
        if pw.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_password"})),
            )
                .into_response();
        }
        let res = sqlx::query(
            r#"
            UPDATE users
            SET password_hash = crypt($2, gen_salt('bf')),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(pw)
        .execute(&state.db)
        .await;

        if let Err(e) = res {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response();
        }
    }

    let row: Option<UserResponse> = sqlx::query_as(
        r#"
        SELECT id, username, email, role, first_name, last_name, created_at, updated_at
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(u) => Json(u).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response(),
    }
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if let Err(e) = auth::require_admin(&user) {
        return e.into_response();
    }

    // Prevent deleting yourself (simple safety)
    if id == user.user_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"cannot_delete_self"})),
        )
            .into_response();
    }

    let res = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await;

    match res {
        Ok(r) if r.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}
