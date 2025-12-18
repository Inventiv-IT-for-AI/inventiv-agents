use axum::{
    extract::State,
    http::{header, StatusCode},
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
}

#[derive(sqlx::FromRow)]
struct MeRow {
    username: String,
    email: String,
    role: String,
    first_name: Option<String>,
    last_name: Option<String>,
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
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"invalid_credentials"})),
        )
            .into_response();
    };

    let auth_user = auth::AuthUser {
        user_id: u.id,
        email: u.email.clone(),
        role: u.role.clone(),
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

pub async fn logout() -> impl IntoResponse {
    let cookie = auth::clear_session_cookie_value();
    let mut resp = Json(json!({"status":"ok"})).into_response();
    resp.headers_mut().insert(header::SET_COOKIE, cookie);
    resp
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let row: Option<MeRow> = sqlx::query_as(
        r#"
        SELECT username, email, role, first_name, last_name
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(user.user_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some(r) = row else {
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

    Json(MeResponse {
        user_id: user.user_id,
        username: r.username,
        email: r.email,
        role: r.role,
        first_name: r.first_name,
        last_name: r.last_name,
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
    let row: Option<MeRow> = sqlx::query_as(
        r#"
        SELECT username, email, role, first_name, last_name
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(user.user_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(r) => Json(MeResponse {
            user_id: user.user_id,
            username: r.username,
            email: r.email,
            role: r.role,
            first_name: r.first_name,
            last_name: r.last_name,
        })
        .into_response(),
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
