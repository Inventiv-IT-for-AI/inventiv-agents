use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::{auth, AppState};

// -----------------------------------------------------------------------------
// Actor (either a logged-in user or an API key principal)
// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Actor {
    User(auth::AuthUser),
    ApiKey(auth::ApiKeyPrincipal),
}

#[derive(Clone, Debug)]
pub struct ActorIds {
    pub user_id: Option<uuid::Uuid>,
    pub api_key_id: Option<uuid::Uuid>,
}

impl Actor {
    pub fn ids(&self) -> ActorIds {
        match self {
            Actor::User(u) => ActorIds {
                user_id: Some(u.user_id),
                api_key_id: None,
            },
            Actor::ApiKey(k) => ActorIds {
                user_id: Some(k.user_id),
                api_key_id: Some(k.api_key_id),
            },
        }
    }
}

// This extractor is used on routes protected by `require_user_or_api_key`.
// That middleware injects either `AuthUser` or `ApiKeyPrincipal` into extensions.
#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for Actor
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        if let Some(u) = parts.extensions.get::<auth::AuthUser>().cloned() {
            return Ok(Actor::User(u));
        }
        if let Some(k) = parts.extensions.get::<auth::ApiKeyPrincipal>().cloned() {
            return Ok(Actor::ApiKey(k));
        }
        Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error":"unauthorized",
                "message":"api_key_or_login_required"
            })),
        ))
    }
}

// -----------------------------------------------------------------------------
// Models
// -----------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct WorkbenchRunRow {
    pub id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by_user_id: Option<uuid::Uuid>,
    pub created_via_api_key_id: Option<uuid::Uuid>,
    pub model_id: String,
    pub mode: String,
    pub status: String,
    pub ttft_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub error_message: Option<String>,
    pub metadata: sqlx::types::Json<serde_json::Value>,
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct WorkbenchMessageRow {
    pub id: uuid::Uuid,
    pub run_id: uuid::Uuid,
    pub message_index: i32,
    pub role: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateWorkbenchRunRequest {
    pub model_id: String,
    /// Optional: associate this run to a dashboard API key row (id), without ever storing plaintext keys.
    pub api_key_id: Option<uuid::Uuid>,
    /// chat | validation | batch
    pub mode: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct CreateWorkbenchRunResponse {
    pub run: WorkbenchRunRow,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AppendWorkbenchMessageRequest {
    pub message_index: i32,
    pub role: String, // system|user|assistant
    pub content: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct AppendWorkbenchMessageResponse {
    pub message: WorkbenchMessageRow,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CompleteWorkbenchRunRequest {
    /// success | failed | cancelled
    pub status: String,
    pub ttft_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub error_message: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct WorkbenchRunWithMessages {
    pub run: WorkbenchRunRow,
    pub messages: Vec<WorkbenchMessageRow>,
}

#[derive(Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
pub struct ListWorkbenchRunsQuery {
    pub limit: Option<i64>,
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

async fn actor_can_access_run(db: &Pool<Postgres>, actor: &Actor, run_id: uuid::Uuid) -> bool {
    match actor {
        Actor::ApiKey(k) => {
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM workbench_runs WHERE id=$1 AND created_via_api_key_id=$2)",
            )
            .bind(run_id)
            .bind(k.api_key_id)
            .fetch_one(db)
            .await
            .unwrap_or(false);
            ok
        }
        Actor::User(u) => {
            // User can access runs they created OR runs created through any of their API keys.
            let ok: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                  SELECT 1
                  FROM workbench_runs r
                  WHERE r.id = $1
                    AND (
                      r.created_by_user_id = $2
                      OR r.created_via_api_key_id IN (SELECT id FROM api_keys WHERE user_id = $2)
                    )
                )
                "#,
            )
            .bind(run_id)
            .bind(u.user_id)
            .fetch_one(db)
            .await
            .unwrap_or(false);
            ok
        }
    }
}

async fn fetch_run(db: &Pool<Postgres>, run_id: uuid::Uuid) -> Option<WorkbenchRunRow> {
    sqlx::query_as::<Postgres, WorkbenchRunRow>(
        r#"
        SELECT
          id, created_at, started_at, completed_at,
          created_by_user_id, created_via_api_key_id,
          model_id, mode, status,
          ttft_ms, duration_ms, error_message,
          metadata
        FROM workbench_runs
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

// -----------------------------------------------------------------------------
// Handlers
// -----------------------------------------------------------------------------

#[utoipa::path(
    post,
    path = "/workbench/runs",
    request_body = CreateWorkbenchRunRequest,
    responses((status = 200, description = "Created workbench run", body = CreateWorkbenchRunResponse))
)]
pub async fn create_workbench_run(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Json(req): Json<CreateWorkbenchRunRequest>,
) -> impl IntoResponse {
    let model_id = req.model_id.trim().to_string();
    if model_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"model_id_required"})),
        )
            .into_response();
    }
    if model_id.len() > 512 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"model_id_too_long"})),
        )
            .into_response();
    }

    let mode = req
        .mode
        .unwrap_or_else(|| "chat".to_string())
        .trim()
        .to_ascii_lowercase();
    let mode = match mode.as_str() {
        "chat" | "validation" | "batch" => mode,
        _ => "chat".to_string(),
    };

    // If an api_key_id is provided, ensure it belongs to the actor (user or api key principal).
    let provided_api_key_id = req.api_key_id;
    if let Some(kid) = provided_api_key_id {
        let ok = match &actor {
            Actor::ApiKey(k) => kid == k.api_key_id,
            Actor::User(u) => {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM api_keys WHERE id=$1 AND user_id=$2)",
                )
                .bind(kid)
                .bind(u.user_id)
                .fetch_one(&state.db)
                .await
                .unwrap_or(false);
                exists
            }
        };
        if !ok {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error":"forbidden","message":"api_key_not_allowed"})),
            )
                .into_response();
        }
    }

    let ids = actor.ids();
    let metadata = req.metadata.unwrap_or_else(|| serde_json::json!({}));

    let run_id = uuid::Uuid::new_v4();
    let res = sqlx::query(
        r#"
        INSERT INTO workbench_runs (
          id, created_by_user_id, created_via_api_key_id,
          model_id, mode, status, ttft_ms, duration_ms, error_message, metadata
        )
        VALUES ($1, $2, $3, $4, $5, 'in_progress', NULL, NULL, NULL, $6)
        "#,
    )
    .bind(run_id)
    .bind(ids.user_id)
    // Prefer explicit api_key_id if provided; else if actor is api key, link it.
    .bind(provided_api_key_id.or(ids.api_key_id))
    .bind(&model_id)
    .bind(&mode)
    .bind(sqlx::types::Json(metadata))
    .execute(&state.db)
    .await;

    if let Err(e) = res {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response();
    }

    let Some(run) = fetch_run(&state.db, run_id).await else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"db_error","message":"run_fetch_failed"})),
        )
            .into_response();
    };

    Json(CreateWorkbenchRunResponse { run }).into_response()
}

#[utoipa::path(
    get,
    path = "/workbench/runs",
    params(ListWorkbenchRunsQuery),
    responses((status = 200, description = "List workbench runs", body = Vec<WorkbenchRunRow>))
)]
pub async fn list_workbench_runs(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Query(q): Query<ListWorkbenchRunsQuery>,
) -> Json<Vec<WorkbenchRunRow>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows = match actor {
        Actor::ApiKey(k) => {
            sqlx::query_as::<Postgres, WorkbenchRunRow>(
                r#"
                SELECT
                  id, created_at, started_at, completed_at,
                  created_by_user_id, created_via_api_key_id,
                  model_id, mode, status,
                  ttft_ms, duration_ms, error_message,
                  metadata
                FROM workbench_runs
                WHERE created_via_api_key_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(k.api_key_id)
            .bind(limit)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
        }
        Actor::User(u) => {
            sqlx::query_as::<Postgres, WorkbenchRunRow>(
                r#"
                SELECT
                  id, created_at, started_at, completed_at,
                  created_by_user_id, created_via_api_key_id,
                  model_id, mode, status,
                  ttft_ms, duration_ms, error_message,
                  metadata
                FROM workbench_runs
                WHERE created_by_user_id = $1
                   OR created_via_api_key_id IN (SELECT id FROM api_keys WHERE user_id = $1)
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(u.user_id)
            .bind(limit)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
        }
    };
    Json(rows)
}

#[utoipa::path(
    get,
    path = "/workbench/runs/{id}",
    responses((status = 200, description = "Workbench run with messages", body = WorkbenchRunWithMessages))
)]
pub async fn get_workbench_run(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    if !actor_can_access_run(&state.db, &actor, id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"not_found"})),
        )
            .into_response();
    }

    let Some(run) = fetch_run(&state.db, id).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"not_found"})),
        )
            .into_response();
    };

    let msgs = sqlx::query_as::<Postgres, WorkbenchMessageRow>(
        r#"
        SELECT id, run_id, message_index, role, content, created_at
        FROM workbench_messages
        WHERE run_id = $1
        ORDER BY message_index ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(WorkbenchRunWithMessages { run, messages: msgs }).into_response()
}

#[utoipa::path(
    post,
    path = "/workbench/runs/{id}/messages",
    request_body = AppendWorkbenchMessageRequest,
    responses((status = 200, description = "Appended message", body = AppendWorkbenchMessageResponse))
)]
pub async fn append_workbench_message(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<AppendWorkbenchMessageRequest>,
) -> impl IntoResponse {
    if !actor_can_access_run(&state.db, &actor, id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"not_found"})),
        )
            .into_response();
    }

    let role = req.role.trim().to_ascii_lowercase();
    if !matches!(role.as_str(), "system" | "user" | "assistant") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"invalid_role"})),
        )
            .into_response();
    }
    let content = req.content.trim_end().to_string();
    if content.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"content_required"})),
        )
            .into_response();
    }

    let res = sqlx::query(
        r#"
        INSERT INTO workbench_messages (run_id, message_index, role, content)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(id)
    .bind(req.message_index)
    .bind(&role)
    .bind(&content)
    .execute(&state.db)
    .await;

    if let Err(e) = res {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":"conflict","message": e.to_string()})),
        )
            .into_response();
    }

    let msg = sqlx::query_as::<Postgres, WorkbenchMessageRow>(
        r#"
        SELECT id, run_id, message_index, role, content, created_at
        FROM workbench_messages
        WHERE run_id = $1 AND message_index = $2
        "#,
    )
    .bind(id)
    .bind(req.message_index)
    .fetch_one(&state.db)
    .await;

    match msg {
        Ok(message) => Json(AppendWorkbenchMessageResponse { message }).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/workbench/runs/{id}/complete",
    request_body = CompleteWorkbenchRunRequest,
    responses((status = 200, description = "Completed run", body = WorkbenchRunRow))
)]
pub async fn complete_workbench_run(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<CompleteWorkbenchRunRequest>,
) -> impl IntoResponse {
    if !actor_can_access_run(&state.db, &actor, id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"not_found"})),
        )
            .into_response();
    }

    let status = req.status.trim().to_ascii_lowercase();
    if !matches!(status.as_str(), "success" | "failed" | "cancelled") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"invalid_status"})),
        )
            .into_response();
    }

    // Merge metadata (best-effort).
    let meta = req.metadata.unwrap_or_else(|| serde_json::json!({}));
    let _ = sqlx::query(
        r#"
        UPDATE workbench_runs
        SET
          status = $2,
          completed_at = NOW(),
          ttft_ms = COALESCE($3, ttft_ms),
          duration_ms = COALESCE($4, duration_ms),
          error_message = COALESCE($5, error_message),
          metadata = COALESCE(metadata, '{}'::jsonb) || $6::jsonb
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&status)
    .bind(req.ttft_ms)
    .bind(req.duration_ms)
    .bind(req.error_message.as_deref())
    .bind(sqlx::types::Json(meta))
    .execute(&state.db)
    .await;

    let Some(run) = fetch_run(&state.db, id).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"not_found"})),
        )
            .into_response();
    };

    Json(run).into_response()
}


