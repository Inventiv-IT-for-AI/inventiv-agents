// OpenAI proxy handlers
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use std::sync::Arc;

use crate::app::AppState;
use crate::auth;
use crate::openai_proxy;

pub async fn openai_list_models(
    State(state): State<Arc<AppState>>,
) -> impl axum::response::IntoResponse {
    // Return *live* models based on worker heartbeats:
    // - if at least 1 READY worker serves model_id and heartbeat is recent -> exposed in /v1/models
    // - if no workers for a model for a while -> disappears (staleness window)
    #[derive(serde::Serialize, sqlx::FromRow)]
    struct Row {
        model_id: String,
        last_seen: chrono::DateTime<chrono::Utc>,
    }
    #[derive(serde::Serialize)]
    struct ModelObj {
        id: String,
        object: &'static str,
        created: i64,
        owned_by: &'static str,
    }
    #[derive(serde::Serialize)]
    struct Resp {
        object: &'static str,
        data: Vec<ModelObj>,
    }

    let stale = openai_worker_stale_seconds_db(&state.db).await;
    let rows = sqlx::query_as::<sqlx::Postgres, Row>(
        r#"
        SELECT
          worker_model_id as model_id,
          MAX(
            GREATEST(
              COALESCE(worker_last_heartbeat, 'epoch'::timestamptz),
              COALESCE(last_health_check, 'epoch'::timestamptz),
              COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
            )
          ) as last_seen
        FROM instances
        WHERE status::text = 'ready'
          AND ip_address IS NOT NULL
          AND (worker_status = 'ready' OR worker_status IS NULL)
          AND worker_model_id IS NOT NULL
          AND GREATEST(
              COALESCE(worker_last_heartbeat, 'epoch'::timestamptz),
              COALESCE(last_health_check, 'epoch'::timestamptz),
              COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
            ) > NOW() - ($1::bigint * INTERVAL '1 second')
        GROUP BY worker_model_id
        ORDER BY worker_model_id
        "#,
    )
    .bind(stale)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let data = rows
        .into_iter()
        .map(|r| ModelObj {
            id: r.model_id,
            object: "model",
            created: r.last_seen.timestamp(),
            owned_by: "inventiv",
        })
        .collect();

    axum::Json(Resp {
        object: "list",
        data,
    })
}

pub async fn openai_proxy_chat_completions(
    State(state): State<Arc<AppState>>,
    user: Option<axum::extract::Extension<auth::AuthUser>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    openai_proxy::proxy_to_worker(
        &state,
        "/v1/chat/completions",
        headers,
        body,
        user.map(|u| u.0),
    )
    .await
}

pub async fn openai_proxy_completions(
    State(state): State<Arc<AppState>>,
    user: Option<axum::extract::Extension<auth::AuthUser>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    openai_proxy::proxy_to_worker(&state, "/v1/completions", headers, body, user.map(|u| u.0)).await
}

pub async fn openai_proxy_embeddings(
    State(state): State<Arc<AppState>>,
    user: Option<axum::extract::Extension<auth::AuthUser>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    openai_proxy::proxy_to_worker(&state, "/v1/embeddings", headers, body, user.map(|u| u.0)).await
}

// Helper function for OpenAI worker stale seconds
pub(crate) async fn openai_worker_stale_seconds_db(db: &sqlx::Pool<sqlx::Postgres>) -> i64 {
    openai_worker_stale_seconds_env().max(
        sqlx::query_scalar::<_, i64>(
            "SELECT value_int FROM settings WHERE key = 'OPENAI_WORKER_STALE_SECONDS' LIMIT 1",
        )
        .fetch_optional(db)
        .await
        .unwrap_or(None)
        .unwrap_or(300),
    )
}

fn openai_worker_stale_seconds_env() -> i64 {
    std::env::var("OPENAI_WORKER_STALE_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300)
}
