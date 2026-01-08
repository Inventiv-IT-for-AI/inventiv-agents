// Worker internal route handlers
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::app::AppState;

#[derive(Deserialize)]
struct WorkerInstanceIdPayload {
    instance_id: uuid::Uuid,
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return None;
    };
    let Ok(auth) = auth.to_str() else {
        return None;
    };
    auth.strip_prefix("Bearer ").map(|s| s.to_string())
}

async fn verify_worker_token_db(
    db: &sqlx::Pool<sqlx::Postgres>,
    instance_id: uuid::Uuid,
    token: &str,
) -> bool {
    // Compare hash in DB using pgcrypto digest; avoids adding crypto deps in Rust.
    let ok: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM worker_auth_tokens
          WHERE instance_id = $1
            AND revoked_at IS NULL
            AND token_hash = encode(digest($2::text, 'sha256'), 'hex')
        )
        "#,
    )
    .bind(instance_id)
    .bind(token)
    .fetch_one(db)
    .await
    .unwrap_or(false);

    if ok {
        let _ = sqlx::query(
            "UPDATE worker_auth_tokens SET last_seen_at = NOW() WHERE instance_id = $1",
        )
        .bind(instance_id)
        .execute(db)
        .await;
    }

    ok
}

async fn verify_worker_auth_api(
    db: &sqlx::Pool<sqlx::Postgres>,
    headers: &HeaderMap,
    instance_id: uuid::Uuid,
) -> bool {
    // Backward-compat: allow a global token (useful for early bringup).
    let expected = std::env::var("WORKER_AUTH_TOKEN").unwrap_or_default();
    if !expected.trim().is_empty() {
        if let Some(tok) = extract_bearer(headers) {
            if tok.trim() == expected.trim() {
                return true;
            }
        }
    }

    let Some(tok) = extract_bearer(headers) else {
        return false;
    };
    verify_worker_token_db(db, instance_id, &tok).await
}

fn orchestrator_internal_url() -> String {
    std::env::var("ORCHESTRATOR_INTERNAL_URL")
        .unwrap_or_else(|_| "http://orchestrator:8002".to_string())
}

async fn proxy_post_to_orchestrator(path: &str, headers: HeaderMap, body: Bytes) -> Response {
    let base = orchestrator_internal_url();
    let url = format!("{}/{}", base, path.trim_start_matches('/'));

    let mut req = reqwest::Client::new().post(url).body(body.to_vec());
    // Forward Authorization header (worker auth token)
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            req = req.header(axum::http::header::AUTHORIZATION, s);
        }
    }
    // Preserve content-type if present
    if let Some(ct) = headers.get(axum::http::header::CONTENT_TYPE) {
        if let Ok(s) = ct.to_str() {
            req = req.header(axum::http::header::CONTENT_TYPE, s);
        }
    }

    // Forward client IP chain so Orchestrator can apply bootstrap IP checks.
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(s) = xff.to_str() {
            req = req.header("x-forwarded-for", s);
        }
    }
    if let Some(xri) = headers.get("x-real-ip") {
        if let Ok(s) = xri.to_str() {
            req = req.header("x-real-ip", s);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(axum::http::StatusCode::BAD_GATEWAY);
            let bytes = resp.bytes().await.unwrap_or_default();
            Response::builder()
                .status(status)
                .body(Body::from(bytes))
                .unwrap()
        }
        Err(e) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(json!({"error":"orchestrator_unreachable","message": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn proxy_worker_register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Bootstrap flow: allow missing token on register (orchestrator will check IP + token existence).
    // If a token IS present, we verify it here too (defense-in-depth).
    if extract_bearer(&headers).is_some() {
        let parsed: WorkerInstanceIdPayload = match serde_json::from_slice(&body) {
            Ok(p) => p,
            Err(_) => {
                return (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error":"invalid_body","message":"missing_or_invalid_instance_id"}),
                    ),
                )
                    .into_response();
            }
        };
        if !verify_worker_auth_api(&state.db, &headers, parsed.instance_id).await {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized"})),
            )
                .into_response();
        }
    }

    proxy_post_to_orchestrator("/internal/worker/register", headers, body).await
}

pub async fn proxy_worker_heartbeat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Heartbeat always requires a valid worker token.
    let parsed: WorkerInstanceIdPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_body","message":"missing_or_invalid_instance_id"})),
            )
                .into_response();
        }
    };
    if !verify_worker_auth_api(&state.db, &headers, parsed.instance_id).await {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unauthorized"})),
        )
            .into_response();
    }

    proxy_post_to_orchestrator("/internal/worker/heartbeat", headers, body).await
}
