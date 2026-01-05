use axum::http::HeaderMap;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::auth;

#[derive(sqlx::FromRow, Clone)]
struct ReadyWorkerRow {
    id: Uuid,
    ip_address: String,
    worker_vllm_port: Option<i32>,
    worker_queue_depth: Option<i32>,
    worker_last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
}

/// Extract header value by name
pub fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Update runtime model counters
pub async fn bump_runtime_model_counters(db: &Pool<Postgres>, model_id: &str, ok: bool) {
    let mid = model_id.trim();
    if mid.is_empty() {
        return;
    }
    let _ = sqlx::query(
        r#"
        INSERT INTO runtime_models (model_id, first_seen_at, last_seen_at, total_requests, failed_requests)
        VALUES ($1, NOW(), NOW(), 1, CASE WHEN $2 THEN 0 ELSE 1 END)
        ON CONFLICT (model_id) DO UPDATE
          SET last_seen_at = GREATEST(runtime_models.last_seen_at, NOW()),
              total_requests = runtime_models.total_requests + 1,
              failed_requests = runtime_models.failed_requests + (CASE WHEN $2 THEN 0 ELSE 1 END)
        "#,
    )
    .bind(mid)
    .bind(ok)
    .execute(db)
    .await;
}

/// Select a ready worker for a given model
pub async fn select_ready_worker_for_model(
    db: &Pool<Postgres>,
    model: &str,
    sticky_key: Option<&str>,
) -> Option<(Uuid, String)> {
    // `model` here is the vLLM/OpenAI model id (HF repo id).
    // We route based on `instances.worker_model_id` (set by worker heartbeat/register).
    let model = model.trim();
    let stale = openai_worker_stale_seconds_db(db).await;
    let rows = sqlx::query_as::<Postgres, ReadyWorkerRow>(
        r#"
        SELECT
          id,
          ip_address::text as ip_address,
          worker_vllm_port,
          worker_queue_depth,
          worker_last_heartbeat
        FROM instances
        WHERE status::text = 'ready'
          AND ip_address IS NOT NULL
          AND (worker_status = 'ready' OR worker_status IS NULL)
          AND ($1::text = '' OR worker_model_id = $1)
          -- Use the same freshness signal as /v1/models + /runtime/models:
          -- allow either worker heartbeat OR orchestrator health timestamps to keep the instance routable.
          AND GREATEST(
              COALESCE(worker_last_heartbeat, 'epoch'::timestamptz),
              COALESCE(last_health_check, 'epoch'::timestamptz),
              COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
            ) > NOW() - ($2::bigint * INTERVAL '1 second')
        ORDER BY worker_queue_depth NULLS LAST,
                 GREATEST(
                   COALESCE(worker_last_heartbeat, 'epoch'::timestamptz),
                   COALESCE(last_health_check, 'epoch'::timestamptz),
                   COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
                 ) DESC,
                 created_at DESC
        LIMIT 50
        "#,
    )
    .bind(model)
    .bind(stale)
    .fetch_all(db)
    .await
    .ok()?;

    if rows.is_empty() {
        return None;
    }

    let chosen = if let Some(key) = sticky_key.filter(|k| !k.trim().is_empty()) {
        // Stable-ish affinity to an instance across requests (best effort).
        let mut sorted = rows;
        sorted.sort_by_key(|r| r.id);
        let idx = (stable_hash_u64(key) as usize) % sorted.len();
        sorted[idx].clone()
    } else {
        rows[0].clone()
    };

    let ip = chosen
        .ip_address
        .split('/')
        .next()
        .unwrap_or(&chosen.ip_address)
        .to_string();
    let port = chosen.worker_vllm_port.unwrap_or(8000).max(1) as i32;
    Some((chosen.id, format!("http://{}:{}", ip, port)))
}

/// Resolve OpenAI model ID from request
pub async fn resolve_openai_model_id(
    db: &Pool<Postgres>,
    requested: Option<&str>,
    user: Option<&auth::AuthUser>,
) -> Result<String, (axum::http::StatusCode, axum::Json<serde_json::Value>)> {
    use axum::{http::StatusCode, Json};
    use serde_json::json;
    
    // Accept either:
    // - HF repo id (models.model_id)
    // - UUID (models.id) as string
    // - Organization offering id: org_slug/model_code (private to current org for now)
    // If missing, fallback to WORKER_MODEL_ID env (dev convenience).
    let Some(raw) = requested
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            std::env::var("WORKER_MODEL_ID")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
    else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"missing_model"})),
        ));
    };

    // Step 1: If it looks like a UUID, try resolve -> HF repo id.
    if let Ok(uid) = uuid::Uuid::parse_str(&raw) {
        let hf: Option<String> =
            sqlx::query_scalar("SELECT model_id FROM models WHERE id = $1 AND is_active = true")
                .bind(uid)
                .fetch_optional(db)
                .await
                .ok()
                .flatten();
        return Ok(hf.unwrap_or(raw));
    }

    // Step 2: Check if it matches an active catalog entry by HF repo id FIRST
    // This handles models like "Qwen/Qwen2.5-0.5B-Instruct" which contain "/"
    // We check this BEFORE checking for offering ids to avoid false positives
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM models WHERE model_id = $1 AND is_active = true)",
    )
    .bind(&raw)
    .fetch_one(db)
    .await
    .unwrap_or(false);

    if exists {
        return Ok(raw);
    }

    // Step 3: Only if it's NOT a public model, check if it's an offering id: org_slug/model_code
    // This allows org offerings to override public models if needed, but prevents
    // public HF models from being mistaken for offering ids
    if let Some((org_slug, code)) = raw.split_once('/') {
        let org_slug = org_slug.trim();
        let code = code.trim();
        if org_slug.is_empty() || code.is_empty() {
            // Invalid format, but not a public model either
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error":"model_not_found", "model": raw})),
            ));
        }

        let Some(u) = user else {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"error":"forbidden","message":"offering_requires_user_session"})),
            ));
        };
        let Some(current_org) = u.current_organization_id else {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"error":"forbidden","message":"organization_required"})),
            ));
        };

        // For now: org offerings are private (only usable when the offering belongs to the current org).
        let row: Option<(uuid::Uuid, String)> = sqlx::query_as(
            r#"
            SELECT om.organization_id, m.model_id
            FROM organization_models om
            JOIN organizations o ON o.id = om.organization_id
            JOIN models m ON m.id = om.model_id
            WHERE o.slug = $1
              AND om.code = $2
              AND om.is_active = true
              AND m.is_active = true
            LIMIT 1
            "#,
        )
        .bind(org_slug)
        .bind(code)
        .fetch_optional(db)
        .await
        .ok()
        .flatten();

        let Some((offering_org_id, hf_model_id)) = row else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error":"not_found","message":"offering_not_found"})),
            ));
        };

        if offering_org_id != current_org {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({"error":"forbidden","message":"offering_not_accessible_in_current_org"})),
            ));
        }

        return Ok(hf_model_id);
    }

    // Step 4: Not a UUID, not a public model, not an offering id -> not found
    Err((
        StatusCode::NOT_FOUND,
        Json(json!({"error":"model_not_found", "model": raw})),
    ))
}


fn openai_worker_stale_seconds_env() -> i64 {
    std::env::var("OPENAI_WORKER_STALE_SECONDS")
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|v| *v >= 10 && *v <= 24 * 60 * 60)
        .unwrap_or(300) // 5 minutes
}

async fn openai_worker_stale_seconds_db(db: &Pool<Postgres>) -> i64 {
    // Global settings override (DB) -> env -> settings_definitions default -> hard default.
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

    let env_v = openai_worker_stale_seconds_env();
    if env_v > 0 {
        return env_v;
    }

    300 // Hard default: 5 minutes
}

fn stable_hash_u64(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

