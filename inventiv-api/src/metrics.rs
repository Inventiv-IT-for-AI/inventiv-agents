use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth;
use crate::AppState;

/// Update instance request metrics (counters and tokens)
pub async fn update_instance_request_metrics(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    success: bool,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    total_tokens: Option<i32>,
) {
    let _ = sqlx::query(
        r#"
        SELECT update_instance_request_metrics($1, $2, $3, $4, $5)
        "#,
    )
    .bind(instance_id)
    .bind(success)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(total_tokens)
    .execute(db)
    .await;
}

/// Extract token usage from OpenAI API response JSON
pub fn extract_token_usage(
    response_json: &serde_json::Value,
) -> (Option<i32>, Option<i32>, Option<i32>) {
    let usage = response_json.get("usage");
    if let Some(usage_obj) = usage.and_then(|u| u.as_object()) {
        let prompt_tokens = usage_obj
            .get("prompt_tokens")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
        let completion_tokens = usage_obj
            .get("completion_tokens")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
        let total_tokens = usage_obj
            .get("total_tokens")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
        (prompt_tokens, completion_tokens, total_tokens)
    } else {
        (None, None, None)
    }
}

/// Resolve model UUID from model_id string
pub async fn resolve_model_uuid(db: &Pool<Postgres>, model_id: &str) -> Option<Uuid> {
    sqlx::query_scalar::<Postgres, Uuid>(
        "SELECT id FROM models WHERE model_id = $1 AND is_active = true LIMIT 1",
    )
    .bind(model_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// Store inference usage record in finops.inference_usage
pub async fn store_inference_usage(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    model_id: Uuid,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    total_tokens: Option<i32>,
    api_key_id: Option<Uuid>,
    user: Option<&auth::AuthUser>,
) {
    // Only store if we have at least one token value
    if input_tokens.is_none() && output_tokens.is_none() && total_tokens.is_none() {
        return;
    }

    let consumer_org_id = user.and_then(|u| u.current_organization_id);

    let _ = sqlx::query(
        r#"
        INSERT INTO finops.inference_usage (
            occurred_at,
            instance_id,
            model_id,
            input_tokens,
            output_tokens,
            total_tokens,
            api_key_id,
            consumer_organization_id
        )
        VALUES (NOW(), $1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(instance_id)
    .bind(model_id)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(total_tokens)
    .bind(api_key_id)
    .bind(consumer_org_id)
    .execute(db)
    .await;
}

/// Parse SSE stream to extract token usage from [DONE] chunk
pub fn parse_tokens_from_sse_stream(stream_text: &str) -> (Option<i32>, Option<i32>, Option<i32>) {
    let mut input_tokens = None;
    let mut output_tokens = None;
    let mut total_tokens = None;

    eprintln!(
        "[METRICS] parse_tokens_from_sse_stream: input_length={}",
        stream_text.len()
    );

    // Parse SSE stream backwards to find the last chunk with usage
    // OpenAI sends usage in a separate data chunk before [DONE]
    let lines: Vec<&str> = stream_text.lines().collect();
    eprintln!(
        "[METRICS] parse_tokens_from_sse_stream: total_lines={}",
        lines.len()
    );

    for (idx, line) in lines.iter().rev().enumerate() {
        if line.starts_with("data: ") {
            let payload = line.strip_prefix("data: ").unwrap_or("").trim();
            if payload == "[DONE]" {
                eprintln!(
                    "[METRICS] parse_tokens_from_sse_stream: found [DONE] at line {}",
                    idx
                );
                continue;
            }
            if let Ok(chunk_json) = serde_json::from_str::<serde_json::Value>(payload) {
                if let Some(usage_obj) = chunk_json.get("usage").and_then(|u| u.as_object()) {
                    input_tokens = usage_obj
                        .get("prompt_tokens")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                    output_tokens = usage_obj
                        .get("completion_tokens")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                    total_tokens = usage_obj
                        .get("total_tokens")
                        .and_then(|v| v.as_i64())
                        .map(|v| v as i32);
                    eprintln!("[METRICS] parse_tokens_from_sse_stream: found usage at line {}: input={:?}, output={:?}, total={:?}", 
                        idx, input_tokens, output_tokens, total_tokens);
                    break; // Found usage, stop parsing
                } else {
                    eprintln!(
                        "[METRICS] parse_tokens_from_sse_stream: line {} has no usage field",
                        idx
                    );
                }
            } else {
                eprintln!(
                    "[METRICS] parse_tokens_from_sse_stream: failed to parse JSON at line {}: {}",
                    idx,
                    payload.chars().take(100).collect::<String>()
                );
            }
        }
    }

    if input_tokens.is_none() && output_tokens.is_none() && total_tokens.is_none() {
        eprintln!("[METRICS] parse_tokens_from_sse_stream: no usage found, showing last 10 lines:");
        for line in lines.iter().rev().take(10) {
            eprintln!(
                "[METRICS] parse_tokens_from_sse_stream: {}",
                line.chars().take(200).collect::<String>()
            );
        }
    }

    (input_tokens, output_tokens, total_tokens)
}

#[derive(Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct InstanceRequestMetrics {
    pub instance_id: Uuid,
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_tokens: i64,
    pub first_request_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_request_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[utoipa::path(
    get,
    path = "/instances/{instance_id}/metrics",
    params(
        ("instance_id" = Uuid, Path, description = "Instance UUID")
    ),
    responses(
        (status = 200, description = "Instance request metrics", body = InstanceRequestMetrics),
        (status = 404, description = "Instance not found")
    )
)]
pub async fn get_instance_metrics(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(instance_id): axum::extract::Path<Uuid>,
) -> Response {
    let row = sqlx::query_as::<Postgres, InstanceRequestMetrics>(
        r#"
        SELECT 
            instance_id,
            total_requests,
            successful_requests,
            failed_requests,
            total_input_tokens,
            total_output_tokens,
            total_tokens,
            first_request_at,
            last_request_at
        FROM instance_request_metrics
        WHERE instance_id = $1
        "#,
    )
    .bind(instance_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(metrics)) => Json(metrics).into_response(),
        Ok(None) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not_found", "message": "No metrics found for this instance"})),
        ).into_response(),
        Err(e) => {
            eprintln!("Error fetching instance metrics: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal_error"})),
            ).into_response()
        }
    }
}
