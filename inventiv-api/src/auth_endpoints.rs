use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::{auth, AppState};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    // Backward-compat JSON field name (frontend sends "email").
    // This value is treated as "login": can be username OR email.
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub role: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(sqlx::FromRow)]
struct UserAuthRow {
    id: uuid::Uuid,
    email: String,
    role: String,
    first_name: Option<String>,
    last_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub current_organization_id: Option<uuid::Uuid>,
    pub current_organization_name: Option<String>,
    pub current_organization_slug: Option<String>,
    pub current_organization_role: Option<String>,
}

#[derive(sqlx::FromRow)]
struct MeRow {
    username: String,
    email: String,
    role: String,
    first_name: Option<String>,
    last_name: Option<String>,
    current_organization_id: Option<uuid::Uuid>,
    current_organization_name: Option<String>,
    current_organization_slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMeRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let login = req.email.trim().to_ascii_lowercase();
    let password = req.password;
    if login.is_empty() || password.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request"})),
        )
            .into_response();
    }

    // Verify using pgcrypto bcrypt:
    // password_hash = crypt($password, password_hash)
    let row: Option<UserAuthRow> = sqlx::query_as(
        r#"
        SELECT id, email, role, first_name, last_name
        FROM users
        WHERE (username = $1 OR email = $1)
          AND password_hash = crypt($2, password_hash)
        LIMIT 1
        "#,
    )
    .bind(&login)
    .bind(&password)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some(u) = row else {
        tracing::debug!("Login failed: no user found for login={}", login);
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"invalid_credentials"})),
        )
            .into_response();
    };
    
    tracing::debug!("Login successful for user_id={}, email={}", u.id, u.email);

    // 1. Get user's last used organization (or None for Personal mode)
    let default_org_id = auth::get_user_last_org(&state.db, u.id)
        .await
        .ok()
        .flatten();

    // 2. Resolve organization role if org_id exists
    let org_role = if let Some(org_id) = default_org_id {
        // Use a helper function from organizations module
        // We'll need to make get_membership_role public or create a wrapper
        let role_str: Option<String> = sqlx::query_scalar(
            r#"
            SELECT role
            FROM organization_memberships
            WHERE organization_id = $1 AND user_id = $2
            "#,
        )
        .bind(org_id)
        .bind(u.id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        role_str
    } else {
        None
    };

    // 3. Create session in DB
    let session_id = uuid::Uuid::new_v4();
    let ip_address = auth::extract_ip_address(&headers);
    let user_agent = auth::extract_user_agent(&headers);

    // 4. Generate JWT first (we need it to hash it)
    let auth_user = auth::AuthUser {
        user_id: u.id,
        email: u.email.clone(),
        role: u.role.clone(),
        session_id: session_id.to_string(),
        current_organization_id: default_org_id,
        current_organization_role: org_role.clone(),
    };
    let token = match auth::sign_session_jwt(&auth_user) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"token_sign_failed","message": e.to_string()})),
            )
                .into_response()
        }
    };

    // 5. Store session in DB with token hash
    let token_hash = auth::hash_session_token(&token);
    match auth::create_session(
        &state.db,
        session_id,
        u.id,
        default_org_id,
        org_role.clone(),
        ip_address.clone(),
        user_agent.clone(),
        token_hash,
    )
    .await
    {
        Ok(_) => {
            tracing::debug!("Session created successfully: session_id={}, user_id={}", session_id, u.id);
        }
        Err(e) => {
            tracing::error!("Failed to create session: {}", e);
            tracing::error!("Session creation failed for user_id={}, session_id={}, org_id={:?}", 
                u.id, session_id, default_org_id);
            // Continue login even if session creation fails
            // The session will be created on next request if needed, or user can retry login
            tracing::warn!("Continuing login despite session creation failure - user can still authenticate");
        }
    }

    // 6. Return JWT in cookie
    let cookie = auth::session_cookie_value(&token);
    let mut resp = Json(LoginResponse {
        user_id: u.id,
        email: u.email,
        role: u.role,
        first_name: u.first_name,
        last_name: u.last_name,
    })
    .into_response();
    resp.headers_mut().insert(header::SET_COOKIE, cookie);
    resp
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    // Revoke session in DB
    if let Ok(session_id) = uuid::Uuid::parse_str(&user.session_id) {
        auth::revoke_session(&state.db, session_id).await.ok();
    }
    
    let cookie = auth::clear_session_cookie_value();
    let mut resp = Json(json!({"status":"ok"})).into_response();
    resp.headers_mut().insert(header::SET_COOKIE, cookie);
    resp
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    // Get user data from database
    let row: Option<(String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT
          u.username,
          u.email,
          u.role,
          u.first_name,
          u.last_name
        FROM users u
        WHERE u.id = $1
        LIMIT 1
        "#,
    )
    .bind(user.user_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some((username, email, role, first_name, last_name)) = row else {
        // Session token refers to a user that no longer exists (e.g. DB reset).
        // Treat as unauthorized and clear cookie so the UI can re-login cleanly.
        let mut resp = (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unauthorized","message":"session_invalid"})),
        )
            .into_response();
        resp.headers_mut()
            .insert(header::SET_COOKIE, auth::clear_session_cookie_value());
        return resp;
    };

    // Get organization info if current_organization_id is set (from JWT/session)
    let (org_name, org_slug) = if let Some(org_id) = user.current_organization_id {
        let org_row: Option<(String, String)> = sqlx::query_as(
            r#"
            SELECT name, slug
            FROM organizations
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(org_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        
        org_row.map(|(name, slug)| (Some(name), Some(slug)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    Json(MeResponse {
        user_id: user.user_id,
        username,
        email,
        role,
        first_name,
        last_name,
        current_organization_id: user.current_organization_id,
        current_organization_name: org_name,
        current_organization_slug: org_slug,
        current_organization_role: user.current_organization_role,
    })
    .into_response()
}

pub async fn update_me(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Json(req): Json<UpdateMeRequest>,
) -> impl IntoResponse {
    let username = req
        .username
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let email = req
        .email
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let first_name = req
        .first_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let last_name = req
        .last_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let res = sqlx::query(
        r#"
        UPDATE users
        SET username = COALESCE($2, username),
            email = COALESCE($3, email),
            first_name = COALESCE($4, first_name),
            last_name = COALESCE($5, last_name),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(user.user_id)
    .bind(username)
    .bind(email)
    .bind(first_name)
    .bind(last_name)
    .execute(&state.db)
    .await;

    match res {
        Ok(r) if r.rows_affected() == 0 => {
            let mut resp = (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized","message":"session_invalid"})),
            )
                .into_response();
            resp.headers_mut()
                .insert(header::SET_COOKIE, auth::clear_session_cookie_value());
            return resp;
        }
        Ok(_) => {}
        Err(e) => {
            // Unique constraint violations for username/email.
            let code = match &e {
                sqlx::Error::Database(db) => db.code().map(|c| c.to_string()),
                _ => None,
            };
            if code.as_deref() == Some("23505") {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({"error":"conflict","message":"username_or_email_already_exists"})),
                )
                    .into_response();
            }
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response();
        }
    }

    // Return refreshed profile
    let row: Option<(String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT
          u.username,
          u.email,
          u.role,
          u.first_name,
          u.last_name
        FROM users u
        WHERE u.id = $1
        LIMIT 1
        "#,
    )
    .bind(user.user_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some((username, email, role, first_name, last_name)) => {
            // Get organization info if current_organization_id is set (from JWT/session)
            let (org_name, org_slug) = if let Some(org_id) = user.current_organization_id {
                let org_row: Option<(String, String)> = sqlx::query_as(
                    r#"
                    SELECT name, slug
                    FROM organizations
                    WHERE id = $1
                    LIMIT 1
                    "#,
                )
                .bind(org_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();
                
                org_row.map(|(name, slug)| (Some(name), Some(slug)))
                    .unwrap_or((None, None))
            } else {
                (None, None)
            };

            Json(MeResponse {
                user_id: user.user_id,
                username,
                email,
                role,
                first_name,
                last_name,
                current_organization_id: user.current_organization_id,
                current_organization_name: org_name,
                current_organization_slug: org_slug,
                current_organization_role: user.current_organization_role,
            })
            .into_response()
        }
        None => {
            let mut resp = (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized","message":"session_invalid"})),
            )
                .into_response();
            resp.headers_mut()
                .insert(header::SET_COOKIE, auth::clear_session_cookie_value());
            resp
        }
    }
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let current = req.current_password;
    let new_pw = req.new_password;

    if current.trim().is_empty() || new_pw.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request"})),
        )
            .into_response();
    }

    // If the user no longer exists (DB reset), invalidate session.
    let user_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user.user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);
    if !user_exists {
        let mut resp = (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unauthorized","message":"session_invalid"})),
        )
            .into_response();
        resp.headers_mut()
            .insert(header::SET_COOKIE, auth::clear_session_cookie_value());
        return resp;
    }

    let ok: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM users
          WHERE id = $1
            AND password_hash = crypt($2, password_hash)
        )
        "#,
    )
    .bind(user.user_id)
    .bind(&current)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    if !ok {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_current_password","message":"current_password_invalid"})),
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
    .bind(user.user_id)
    .bind(&new_pw)
    .execute(&state.db)
    .await;

    match res {
        Ok(_) => Json(json!({"status":"ok"})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}

// ============================================================================
// Session Management Endpoints
// ============================================================================

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: uuid::Uuid,
    pub current_organization_id: Option<uuid::Uuid>,
    pub current_organization_name: Option<String>,
    pub organization_role: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub is_current: bool,
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: uuid::Uuid,
    current_organization_id: Option<uuid::Uuid>,
    current_organization_name: Option<String>,
    organization_role: Option<String>,
    ip_address: Option<String>,
    user_agent: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
}

/// List all active sessions for the current user
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let current_session_id = match uuid::Uuid::parse_str(&user.session_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_session_id"})),
            )
                .into_response();
        }
    };

    let rows: Vec<SessionRow> = match sqlx::query_as(
        r#"
        SELECT 
            us.id,
            us.current_organization_id,
            o.name as current_organization_name,
            us.organization_role,
            us.ip_address::text as ip_address,
            us.user_agent,
            us.created_at,
            us.last_used_at,
            us.expires_at
        FROM user_sessions us
        LEFT JOIN organizations o ON o.id = us.current_organization_id
        WHERE us.user_id = $1
          AND us.revoked_at IS NULL
          AND us.expires_at > NOW()
        ORDER BY us.last_used_at DESC
        "#,
    )
    .bind(user.user_id)
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to fetch sessions: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response();
        }
    };

    let sessions: Vec<SessionResponse> = rows
        .into_iter()
        .map(|row| SessionResponse {
            session_id: row.id,
            current_organization_id: row.current_organization_id,
            current_organization_name: row.current_organization_name,
            organization_role: row.organization_role,
            ip_address: row.ip_address,
            user_agent: row.user_agent,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
            expires_at: row.expires_at,
            is_current: row.id == current_session_id,
        })
        .collect();

    Json(sessions).into_response()
}

/// Revoke a specific session
pub async fn revoke_session_endpoint(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Path(session_id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Verify that session_id belongs to user_id (security check)
    let session_user_id: Option<uuid::Uuid> = match sqlx::query_scalar(
        r#"
        SELECT user_id 
        FROM user_sessions 
        WHERE id = $1 
          AND revoked_at IS NULL
        "#,
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(id)) => Some(id),
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"session_not_found"})),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to verify session ownership: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response();
        }
    };

    if session_user_id != Some(user.user_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"forbidden","message":"session_does_not_belong_to_user"})),
        )
            .into_response();
    }

    // Prevent revoking the current session (user should use logout instead)
    let current_session_id = match uuid::Uuid::parse_str(&user.session_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_session_id"})),
            )
                .into_response();
        }
    };

    if session_id == current_session_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"cannot_revoke_current_session","message":"use_logout_endpoint_instead"})),
        )
            .into_response();
    }

    // Revoke the session
    match auth::revoke_session(&state.db, session_id).await {
        Ok(_) => Json(json!({"status":"ok","message":"session_revoked"})).into_response(),
        Err(e) => {
            tracing::error!("Failed to revoke session: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error":"db_error","message": e.to_string()})),
            )
                .into_response()
        }
    }
}
