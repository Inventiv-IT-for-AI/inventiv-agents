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
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by_user_id: Option<uuid::Uuid>,
    pub created_via_api_key_id: Option<uuid::Uuid>,
    pub organization_id: Option<uuid::Uuid>,
    pub shared_with_org: bool,
    pub project_id: Option<uuid::Uuid>,
    pub title: Option<String>,
    pub model_id: String,
    pub mode: String,
    pub status: String,
    pub ttft_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub error_message: Option<String>,
    pub metadata: sqlx::types::Json<serde_json::Value>,
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct WorkbenchProjectRow {
    pub id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub owner_user_id: Option<uuid::Uuid>,
    pub organization_id: Option<uuid::Uuid>,
    pub name: String,
    pub shared_with_org: bool,
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
    /// Optional: human title for history
    pub title: Option<String>,
    /// Optional: assign to project
    pub project_id: Option<uuid::Uuid>,
    /// Optional: share with organization members (only if organization_id is set)
    pub shared_with_org: Option<bool>,
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

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateWorkbenchRunRequest {
    pub title: Option<String>,
    pub project_id: Option<uuid::Uuid>,
    pub shared_with_org: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateWorkbenchProjectRequest {
    pub name: String,
    pub shared_with_org: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateWorkbenchProjectRequest {
    pub name: Option<String>,
    pub shared_with_org: Option<bool>,
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

async fn actor_can_access_run(db: &Pool<Postgres>, actor: &Actor, run_id: uuid::Uuid) -> bool {
    match actor {
        Actor::ApiKey(k) => {
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM workbench_runs WHERE id=$1 AND created_via_api_key_id=$2 AND deleted_at IS NULL)",
            )
            .bind(run_id)
            .bind(k.api_key_id)
            .fetch_one(db)
            .await
            .unwrap_or(false);
            ok
        }
        Actor::User(u) => {
            // User can access:
            // - runs they created
            // - runs created through any of their API keys
            // - runs shared with an org they are a member of
            let ok: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                  SELECT 1
                  FROM workbench_runs r
                  WHERE r.id = $1
                    AND r.deleted_at IS NULL
                    AND (
                      r.created_by_user_id = $2
                      OR r.created_via_api_key_id IN (SELECT id FROM api_keys WHERE user_id = $2)
                      OR (
                        r.organization_id IS NOT NULL
                        AND r.shared_with_org = true
                        AND EXISTS(
                          SELECT 1 FROM organization_memberships om
                          WHERE om.user_id = $2 AND om.organization_id = r.organization_id
                        )
                      )
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
          id, created_at, started_at, completed_at, deleted_at,
          created_by_user_id, created_via_api_key_id,
          organization_id, shared_with_org, project_id, title,
          model_id, mode, status,
          ttft_ms, duration_ms, error_message,
          metadata
        FROM workbench_runs
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(run_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

async fn actor_can_access_project(db: &Pool<Postgres>, user: &auth::AuthUser, project_id: uuid::Uuid) -> bool {
    let ok: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM workbench_projects p
          WHERE p.id = $1
            AND p.deleted_at IS NULL
            AND (
              p.owner_user_id = $2
              OR (
                p.organization_id IS NOT NULL
                AND p.shared_with_org = true
                AND EXISTS(
                  SELECT 1 FROM organization_memberships om
                  WHERE om.user_id = $2 AND om.organization_id = p.organization_id
                )
              )
            )
        )
        "#,
    )
    .bind(project_id)
    .bind(user.user_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);
    ok
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
    let title = req
        .title
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| if s.len() > 200 { s[..200].to_string() } else { s });
    let shared_with_org = req.shared_with_org.unwrap_or(false);

    let (org_id, user_for_checks) = match &actor {
        Actor::User(u) => (u.current_organization_id, Some(u.clone())),
        _ => (None, None),
    };

    // If project_id provided, ensure user can access it (user-only feature).
    if let (Some(pid), Some(u)) = (req.project_id, user_for_checks.as_ref()) {
        if !actor_can_access_project(&state.db, u, pid).await {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error":"forbidden","message":"project_not_allowed"})),
            )
                .into_response();
        }
    }

    // Sharing only makes sense in org context.
    let shared_with_org = shared_with_org && org_id.is_some();

    let run_id = uuid::Uuid::new_v4();
    let res = sqlx::query(
        r#"
        INSERT INTO workbench_runs (
          id, created_by_user_id, created_via_api_key_id,
          organization_id, shared_with_org, project_id, title,
          model_id, mode, status, ttft_ms, duration_ms, error_message, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'in_progress', NULL, NULL, NULL, $10)
        "#,
    )
    .bind(run_id)
    .bind(ids.user_id)
    // Prefer explicit api_key_id if provided; else if actor is api key, link it.
    .bind(provided_api_key_id.or(ids.api_key_id))
    .bind(org_id)
    .bind(shared_with_org)
    .bind(req.project_id)
    .bind(title)
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
                  id, created_at, started_at, completed_at, deleted_at,
                  created_by_user_id, created_via_api_key_id,
                  organization_id, shared_with_org, project_id, title,
                  model_id, mode, status,
                  ttft_ms, duration_ms, error_message,
                  metadata
                FROM workbench_runs
                WHERE created_via_api_key_id = $1
                  AND deleted_at IS NULL
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
                  id, created_at, started_at, completed_at, deleted_at,
                  created_by_user_id, created_via_api_key_id,
                  organization_id, shared_with_org, project_id, title,
                  model_id, mode, status,
                  ttft_ms, duration_ms, error_message,
                  metadata
                FROM workbench_runs
                WHERE deleted_at IS NULL
                  AND (
                    created_by_user_id = $1
                    OR created_via_api_key_id IN (SELECT id FROM api_keys WHERE user_id = $1)
                    OR (
                      organization_id IS NOT NULL
                      AND shared_with_org = true
                      AND EXISTS(
                        SELECT 1 FROM organization_memberships om
                        WHERE om.user_id = $1 AND om.organization_id = workbench_runs.organization_id
                      )
                    )
                  )
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
    put,
    path = "/workbench/runs/{id}",
    request_body = UpdateWorkbenchRunRequest,
    responses((status = 200, description = "Updated run", body = WorkbenchRunRow))
)]
pub async fn update_workbench_run(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<UpdateWorkbenchRunRequest>,
) -> impl IntoResponse {
    // Only users can update run metadata.
    let Actor::User(u) = actor else {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error":"forbidden","message":"user_required"})),
        )
            .into_response();
    };

    if !actor_can_access_run(&state.db, &Actor::User(u.clone()), id).await {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not_found"}))).into_response();
    }

    if let Some(pid) = req.project_id {
        if !actor_can_access_project(&state.db, &u, pid).await {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error":"forbidden","message":"project_not_allowed"})),
            )
                .into_response();
        }
    }

    // Share toggle is only allowed if run is org-scoped and user is member of that org.
    // We keep it simple: shared_with_org can only be true when run.organization_id is set.
    let run_org: Option<uuid::Uuid> = sqlx::query_scalar("SELECT organization_id FROM workbench_runs WHERE id=$1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

    let mut shared_with_org = req.shared_with_org;
    if let Some(true) = shared_with_org {
        let org_id = match run_org {
            Some(x) => x,
            None => {
                shared_with_org = Some(false);
                uuid::Uuid::nil()
            }
        };
        if org_id.is_nil() {
            // org-less run cannot be shared
        } else {
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM organization_memberships WHERE user_id=$1 AND organization_id=$2)",
        )
        .bind(u.user_id)
        .bind(org_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);
        if !is_member {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error":"forbidden","message":"not_a_member"})),
            )
                .into_response();
        }
        }
    }

    let title = req
        .title
        .map(|s| s.trim().to_string())
        .map(|s| if s.is_empty() { None } else { Some(s) })
        .unwrap_or(None)
        .map(|s| if s.len() > 200 { s[..200].to_string() } else { s });

    let _ = sqlx::query(
        r#"
        UPDATE workbench_runs
        SET title = COALESCE($2, title),
            project_id = COALESCE($3, project_id),
            shared_with_org = COALESCE($4, shared_with_org)
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(title)
    .bind(req.project_id)
    .bind(shared_with_org)
    .execute(&state.db)
    .await;

    let Some(run) = fetch_run(&state.db, id).await else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not_found"}))).into_response();
    };
    Json(run).into_response()
}

#[utoipa::path(
    delete,
    path = "/workbench/runs/{id}",
    responses((status = 200, description = "Deleted run", body = WorkbenchRunRow))
)]
pub async fn delete_workbench_run(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Only users can delete.
    let Actor::User(u) = actor else {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error":"forbidden","message":"user_required"})),
        )
            .into_response();
    };
    // Only allow deleting runs the user "owns" (created_by or via their api keys).
    let ok: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM workbench_runs r
          WHERE r.id = $1
            AND r.deleted_at IS NULL
            AND (
              r.created_by_user_id = $2
              OR r.created_via_api_key_id IN (SELECT id FROM api_keys WHERE user_id = $2)
            )
        )
        "#,
    )
    .bind(id)
    .bind(u.user_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);
    if !ok {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not_found"}))).into_response();
    }

    let _ = sqlx::query("UPDATE workbench_runs SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .execute(&state.db)
        .await;

    // Return last visible snapshot (best-effort)
    let run: Option<WorkbenchRunRow> = sqlx::query_as::<Postgres, WorkbenchRunRow>(
        r#"
        SELECT
          id, created_at, started_at, completed_at, deleted_at,
          created_by_user_id, created_via_api_key_id,
          organization_id, shared_with_org, project_id, title,
          model_id, mode, status,
          ttft_ms, duration_ms, error_message,
          metadata
        FROM workbench_runs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match run {
        Some(r) => Json(r).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not_found"}))).into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/workbench/projects",
    responses((status = 200, description = "List projects", body = Vec<WorkbenchProjectRow>))
)]
pub async fn list_workbench_projects(
    State(state): State<Arc<AppState>>,
    actor: Actor,
) -> impl IntoResponse {
    let Actor::User(u) = actor else {
        return (StatusCode::OK, Json(Vec::<WorkbenchProjectRow>::new())).into_response();
    };
    let rows: Vec<WorkbenchProjectRow> = sqlx::query_as(
        r#"
        SELECT id, created_at, updated_at, deleted_at, owner_user_id, organization_id, name, shared_with_org
        FROM workbench_projects p
        WHERE p.deleted_at IS NULL
          AND (
            p.owner_user_id = $1
            OR (
              p.organization_id IS NOT NULL
              AND p.shared_with_org = true
              AND EXISTS(
                SELECT 1 FROM organization_memberships om
                WHERE om.user_id = $1 AND om.organization_id = p.organization_id
              )
            )
          )
        ORDER BY COALESCE(p.organization_id::text, ''), p.name ASC, p.created_at DESC
        "#,
    )
    .bind(u.user_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    (StatusCode::OK, Json(rows)).into_response()
}

#[utoipa::path(
    post,
    path = "/workbench/projects",
    request_body = CreateWorkbenchProjectRequest,
    responses((status = 200, description = "Created project", body = WorkbenchProjectRow))
)]
pub async fn create_workbench_project(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Json(req): Json<CreateWorkbenchProjectRequest>,
) -> impl IntoResponse {
    let Actor::User(u) = actor else {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error":"forbidden","message":"user_required"})),
        )
            .into_response();
    };
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"name_required"})),
        )
            .into_response();
    }
    let name = if name.len() > 120 { name[..120].to_string() } else { name };

    let org_id = u.current_organization_id;
    let shared = req.shared_with_org.unwrap_or(false) && org_id.is_some();

    let row: Option<WorkbenchProjectRow> = sqlx::query_as(
        r#"
        INSERT INTO workbench_projects (owner_user_id, organization_id, name, shared_with_org)
        VALUES ($1, $2, $3, $4)
        RETURNING id, created_at, updated_at, deleted_at, owner_user_id, organization_id, name, shared_with_org
        "#,
    )
    .bind(u.user_id)
    .bind(org_id)
    .bind(name)
    .bind(shared)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(p) => (StatusCode::OK, Json(p)).into_response(),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"db_error","message":"project_create_failed"})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/workbench/projects/{id}",
    request_body = UpdateWorkbenchProjectRequest,
    responses((status = 200, description = "Updated project", body = WorkbenchProjectRow))
)]
pub async fn update_workbench_project(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<UpdateWorkbenchProjectRequest>,
) -> impl IntoResponse {
    let Actor::User(u) = actor else {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error":"forbidden","message":"user_required"})),
        )
            .into_response();
    };

    // Only allow updating projects the user owns.
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workbench_projects WHERE id=$1 AND owner_user_id=$2 AND deleted_at IS NULL)",
    )
    .bind(id)
    .bind(u.user_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);
    if !owned {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not_found"}))).into_response();
    }

    let name = req
        .name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| if s.len() > 120 { s[..120].to_string() } else { s });

    // shared_with_org only if org_id is set
    let org_id: Option<uuid::Uuid> = sqlx::query_scalar("SELECT organization_id FROM workbench_projects WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    let shared = req.shared_with_org.map(|b| b && org_id.is_some());

    let _ = sqlx::query(
        r#"
        UPDATE workbench_projects
        SET name = COALESCE($2, name),
            shared_with_org = COALESCE($3, shared_with_org),
            updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(shared)
    .execute(&state.db)
    .await;

    let row: Option<WorkbenchProjectRow> = sqlx::query_as(
        r#"
        SELECT id, created_at, updated_at, deleted_at, owner_user_id, organization_id, name, shared_with_org
        FROM workbench_projects
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(p) => (StatusCode::OK, Json(p)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not_found"}))).into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/workbench/projects/{id}",
    responses((status = 200, description = "Deleted project", body = WorkbenchProjectRow))
)]
pub async fn delete_workbench_project(
    State(state): State<Arc<AppState>>,
    actor: Actor,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let Actor::User(u) = actor else {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error":"forbidden","message":"user_required"})),
        )
            .into_response();
    };

    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workbench_projects WHERE id=$1 AND owner_user_id=$2 AND deleted_at IS NULL)",
    )
    .bind(id)
    .bind(u.user_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);
    if !owned {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not_found"}))).into_response();
    }

    let _ = sqlx::query("UPDATE workbench_projects SET deleted_at = NOW(), updated_at = NOW() WHERE id=$1 AND deleted_at IS NULL")
        .bind(id)
        .execute(&state.db)
        .await;

    // Unassign runs from deleted project (best-effort)
    let _ = sqlx::query("UPDATE workbench_runs SET project_id = NULL WHERE project_id = $1")
        .bind(id)
        .execute(&state.db)
        .await;

    let row: Option<WorkbenchProjectRow> = sqlx::query_as(
        r#"
        SELECT id, created_at, updated_at, deleted_at, owner_user_id, organization_id, name, shared_with_org
        FROM workbench_projects
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(p) => (StatusCode::OK, Json(p)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"not_found"}))).into_response(),
    }
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


