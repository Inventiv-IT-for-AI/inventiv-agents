use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::{auth, AppState};

#[derive(Debug, Serialize)]
pub struct ChatModel {
    /// Value to send as OpenAI `model` in /v1/chat/completions
    pub model: String,
    /// Human label for UI
    pub label: String,
    /// Where it comes from: "public" | "org"
    pub scope: String,
    /// Underlying HF model id routed to workers (for debugging/UI)
    pub underlying_model_id: String,
}

async fn worker_stale_seconds_db(db: &Pool<Postgres>) -> i64 {
    // Keep consistent with openai_worker_stale_seconds_db (main.rs).
    let from_db: Option<i64> = sqlx::query_scalar(
        "SELECT value_int FROM global_settings WHERE key = 'OPENAI_WORKER_STALE_SECONDS'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    if let Some(v) = from_db {
        return v.clamp(10, 24 * 60 * 60);
    }

    // Env override (shared with API)
    let env_v = std::env::var("OPENAI_WORKER_STALE_SECONDS")
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0);
    if env_v > 0 {
        return env_v.clamp(10, 24 * 60 * 60);
    }

    let def_v: Option<i64> = sqlx::query_scalar(
        "SELECT default_int FROM settings_definitions WHERE key = 'OPENAI_WORKER_STALE_SECONDS' AND scope = 'global'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    def_v.unwrap_or(300).clamp(10, 24 * 60 * 60)
}

pub async fn list_chat_models(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    // 1) Compute "live" model ids from workers (same logic as /v1/models).
    let stale = worker_stale_seconds_db(&state.db).await;
    let live: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT worker_model_id
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
        ORDER BY worker_model_id
        "#,
    )
    .bind(stale)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if live.is_empty() {
        return (StatusCode::OK, Json(Vec::<ChatModel>::new())).into_response();
    }

    // 2) Public models (catalog) available in live set.
    #[derive(sqlx::FromRow)]
    struct PublicRow {
        name: String,
        model_id: String,
    }
    let public_rows: Vec<PublicRow> = sqlx::query_as(
        r#"
        SELECT name, model_id
        FROM models
        WHERE is_active = true
          AND model_id = ANY($1)
        ORDER BY name ASC
        "#,
    )
    .bind(&live)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut out: Vec<ChatModel> = public_rows
        .into_iter()
        .map(|r| ChatModel {
            model: r.model_id.clone(),
            label: r.name,
            scope: "public".to_string(),
            underlying_model_id: r.model_id,
        })
        .collect();

    // 3) Org offerings (private to current org for now): only if org selected.
    if let Some(org_id) = user.current_organization_id {
        #[derive(sqlx::FromRow)]
        struct OrgRow {
            org_slug: String,
            code: String,
            name: String,
            underlying_model_id: String,
        }
        let org_rows: Vec<OrgRow> = sqlx::query_as(
            r#"
            SELECT
              o.slug as org_slug,
              om.code as code,
              om.name as name,
              m.model_id as underlying_model_id
            FROM organization_models om
            JOIN organizations o ON o.id = om.organization_id
            JOIN models m ON m.id = om.model_id
            WHERE om.organization_id = $1
              AND om.is_active = true
              AND m.is_active = true
              AND m.model_id = ANY($2)
            ORDER BY om.name ASC
            "#,
        )
        .bind(org_id)
        .bind(&live)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        for r in org_rows {
            out.push(ChatModel {
                model: format!("{}/{}", r.org_slug, r.code),
                label: r.name,
                scope: "org".to_string(),
                underlying_model_id: r.underlying_model_id,
            });
        }
    }

    (StatusCode::OK, Json(out)).into_response()
}


