use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

use crate::{email, AppState};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RequestPasswordResetRequest {
    pub email: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RequestPasswordResetResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ResetPasswordResponse {
    pub message: String,
}

/// Generate a secure random token
fn generate_token() -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    STANDARD.encode(bytes)
}

/// Hash a token using SHA256
fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Request password reset - sends email with reset link
#[utoipa::path(
    post,
    path = "/auth/password-reset/request",
    tag = "Auth",
    request_body = RequestPasswordResetRequest,
    responses(
        (status = 200, description = "Password reset email sent (or will appear to be sent)", body = RequestPasswordResetResponse),
        (status = 400, description = "Invalid request")
    )
)]
pub async fn request_password_reset(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RequestPasswordResetRequest>,
) -> impl IntoResponse {
    let email_addr = req.email.trim().to_ascii_lowercase();
    if email_addr.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request","message":"email_required"})),
        )
            .into_response();
    }

    // Find user by email
    let user_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1 LIMIT 1")
        .bind(&email_addr)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

    // Always return success to prevent email enumeration
    // But only actually send email if user exists
    if let Some(uid) = user_id {
        // Generate token
        let token = generate_token();
        let token_hash = hash_token(&token);
        let expires_at = Utc::now() + Duration::hours(1);

        tracing::debug!("Token expires at: {}", expires_at);

        tracing::debug!(
            "Generated password reset token: length={}, hash={}",
            token.len(),
            &token_hash[..16.min(token_hash.len())]
        );

        // Store token in database
        let insert_result = sqlx::query(
            r#"
            INSERT INTO password_reset_tokens (user_id, token_hash, expires_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(uid)
        .bind(&token_hash)
        .bind(expires_at)
        .execute(&state.db)
        .await;

        if let Err(e) = insert_result {
            tracing::error!("Failed to insert password reset token: {}", e);
            // Continue anyway - email will be sent but token won't be stored
        } else if let Ok(result) = insert_result {
            tracing::debug!(
                "Password reset token inserted: rows_affected={}",
                result.rows_affected()
            );
        }

        // Send email if email service is configured
        if let Some(email_service) = email::EmailService::from_env() {
            let base_url = std::env::var("FRONTEND_URL")
                .or_else(|_| std::env::var("FRONTEND_DOMAIN"))
                .unwrap_or_else(|_| "http://localhost:3000".to_string());

            if let Err(e) = email_service
                .send_password_reset(&email_addr, &token, Some(&base_url))
                .await
            {
                tracing::error!("Failed to send password reset email: {}", e);
                // Continue anyway - token is stored, user can request again
            }
        } else {
            tracing::warn!(
                "SMTP not configured - password reset email not sent. Token: {}",
                token
            );
        }
    }

    // Always return success to prevent email enumeration
    (
        StatusCode::OK,
        Json(RequestPasswordResetResponse {
            message: "Si cette adresse email existe, un email de réinitialisation a été envoyé."
                .to_string(),
        }),
    )
        .into_response()
}

/// Reset password using token
#[utoipa::path(
    post,
    path = "/auth/password-reset/reset",
    tag = "Auth",
    request_body = ResetPasswordRequest,
    responses(
        (status = 200, description = "Password reset successful", body = ResetPasswordResponse),
        (status = 400, description = "Invalid token or password"),
        (status = 404, description = "Token not found or expired")
    )
)]
pub async fn reset_password(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ResetPasswordRequest>,
) -> impl IntoResponse {
    let token = req.token.trim();
    let new_password = req.new_password.trim();

    if token.is_empty() || new_password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_request","message":"token_and_password_required"})),
        )
            .into_response();
    }

    if new_password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_password","message":"password_too_short"})),
        )
            .into_response();
    }

    // Decode URL-encoded token if needed (base64 tokens may contain +, /, =)
    let decoded_token = urlencoding::decode(token)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| token.to_string());
    let token_hash = hash_token(&decoded_token);

    tracing::debug!(
        "Token validation: received token length={}, decoded length={}, hash={}",
        token.len(),
        decoded_token.len(),
        &token_hash[..16.min(token_hash.len())]
    );

    // Find valid token
    let now = Utc::now();
    tracing::debug!(
        "Current time: {}, looking for token hash: {}",
        now,
        &token_hash[..16.min(token_hash.len())]
    );

    let token_row: Option<(Uuid, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)> =
        sqlx::query_as(
            r#"
        SELECT user_id, expires_at, used_at
        FROM password_reset_tokens
        WHERE token_hash = $1
        LIMIT 1
        "#,
        )
        .bind(&token_hash)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

    let Some((user_id, expires_at, used_at)) = token_row else {
        tracing::debug!("Token not found in database");
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"invalid_token","message":"token_not_found_or_expired"})),
        )
            .into_response();
    };

    if used_at.is_some() {
        tracing::debug!("Token already used at: {}", used_at.unwrap());
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"invalid_token","message":"token_not_found_or_expired"})),
        )
            .into_response();
    }

    if expires_at <= now {
        tracing::debug!("Token expired: expires_at={}, now={}", expires_at, now);
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"invalid_token","message":"token_not_found_or_expired"})),
        )
            .into_response();
    }

    // Update password
    let update_result = sqlx::query(
        r#"
        UPDATE users
        SET password_hash = crypt($1, gen_salt('bf')),
            updated_at = NOW()
        WHERE id = $2
        "#,
    )
    .bind(new_password)
    .bind(user_id)
    .execute(&state.db)
    .await;

    if update_result.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message":"failed_to_update_password"})),
        )
            .into_response();
    }

    // Mark token as used
    let _ = sqlx::query(
        r#"
        UPDATE password_reset_tokens
        SET used_at = NOW()
        WHERE token_hash = $1
        "#,
    )
    .bind(&token_hash)
    .execute(&state.db)
    .await;

    (
        StatusCode::OK,
        Json(ResetPasswordResponse {
            message: "Mot de passe réinitialisé avec succès.".to_string(),
        }),
    )
        .into_response()
}
