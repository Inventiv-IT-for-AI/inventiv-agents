use axum::{
    extract::Query,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, QueryBuilder};
use std::sync::Arc;

use crate::{auth, AppState};

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema, Clone)]
pub struct ApiKeyRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub key_prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct CreateApiKeyResponse {
    pub key: ApiKeyRow,
    /// The plaintext API key. Only returned once.
    pub api_key: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateApiKeyRequest {
    pub name: String,
}

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct ApiKeysSearchQuery {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    /// Sort field allowlist: name|key_prefix|created_at|last_used_at|revoked_at
    pub sort_by: Option<String>,
    /// "asc" | "desc"
    pub sort_dir: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ApiKeysSearchResponse {
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub filtered_count: i64,
    pub rows: Vec<ApiKeyRow>,
}

fn dir_sql(dir: Option<&str>) -> &'static str {
    match dir.unwrap_or("desc").to_ascii_lowercase().as_str() {
        "asc" => "ASC",
        _ => "DESC",
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub fn generate_api_key() -> (String, String) {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let body = hex_encode(&buf);
    let key = format!("sk-inv-{}", body);
    let prefix = key.chars().take(12).collect::<String>();
    (key, prefix)
}

pub async fn insert_api_key(
    db: &Pool<Postgres>,
    user_id: uuid::Uuid,
    name: &str,
    plaintext_key: &str,
    key_prefix: &str,
) -> Result<ApiKeyRow, sqlx::Error> {
    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO api_keys (id, user_id, name, key_hash, key_prefix, metadata)
        VALUES ($1, $2, $3, encode(digest($4::text, 'sha256'), 'hex'), $5, '{}'::jsonb)
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(plaintext_key)
    .bind(key_prefix)
    .execute(db)
    .await?;

    let row = sqlx::query_as::<Postgres, ApiKeyRow>(
        r#"
        SELECT id, name, key_prefix, created_at, last_used_at, revoked_at
        FROM api_keys
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    Ok(row)
}

#[utoipa::path(
    get,
    path = "/api_keys",
    responses((status = 200, description = "List API keys", body = Vec<ApiKeyRow>))
)]
pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
) -> Json<Vec<ApiKeyRow>> {
    let rows = sqlx::query_as::<Postgres, ApiKeyRow>(
        r#"
        SELECT id, name, key_prefix, created_at, last_used_at, revoked_at
        FROM api_keys
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(user.user_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    Json(rows)
}

#[utoipa::path(
    get,
    path = "/api_keys/search",
    tag = "ApiKeys",
    params(ApiKeysSearchQuery),
    responses((status = 200, description = "Search API keys (current user)", body = ApiKeysSearchResponse))
)]
pub async fn search_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
    Query(params): Query<ApiKeysSearchQuery>,
) -> Json<ApiKeysSearchResponse> {
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_keys WHERE user_id = $1")
        .bind(user.user_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    // No extra filters for now (same as total).
    let filtered_count = total_count;

    let order_by = match params.sort_by.as_deref() {
        Some("key_prefix") => "key_prefix",
        Some("created_at") => "created_at",
        Some("last_used_at") => "last_used_at",
        Some("revoked_at") => "revoked_at",
        _ => "created_at",
    };
    let dir = dir_sql(params.sort_dir.as_deref());

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT id, name, key_prefix, created_at, last_used_at, revoked_at
        FROM api_keys
        WHERE user_id = 
        "#,
    );
    qb.push_bind(user.user_id);
    qb.push(" ORDER BY ");
    qb.push(order_by);
    qb.push(" ");
    qb.push(dir);
    qb.push(" NULLS LAST, id ");
    qb.push(dir);
    qb.push(" LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    let rows: Vec<ApiKeyRow> = qb
        .build_query_as()
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(ApiKeysSearchResponse {
        offset,
        limit,
        total_count,
        filtered_count,
        rows,
    })
}

#[utoipa::path(
    post,
    path = "/api_keys",
    request_body = CreateApiKeyRequest,
    responses((status = 200, description = "Created API key", body = CreateApiKeyResponse))
)]
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
    Json(req): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    let name = req.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"name_required"})),
        )
            .into_response();
    }
    if name.len() > 100 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"name_too_long"})),
        )
            .into_response();
    }

    let (key, prefix) = generate_api_key();
    match insert_api_key(&state.db, user.user_id, name, &key, &prefix).await {
        Ok(row) => Json(CreateApiKeyResponse {
            key: row,
            api_key: key,
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api_keys/{id}",
    request_body = UpdateApiKeyRequest,
    responses((status = 200, description = "Updated API key"))
)]
pub async fn update_api_key(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<UpdateApiKeyRequest>,
) -> impl IntoResponse {
    let name = req.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid_request","message":"name_required"})),
        )
            .into_response();
    }

    let res = sqlx::query(
        r#"
        UPDATE api_keys
        SET name = $1
        WHERE id = $2 AND user_id = $3
        "#,
    )
    .bind(name)
    .bind(id)
    .bind(user.user_id)
    .execute(&state.db)
    .await;

    match res {
        Ok(r) if r.rows_affected() > 0 => StatusCode::OK.into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"not_found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api_keys/{id}",
    responses((status = 200, description = "Revoked API key"))
)]
pub async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let res = sqlx::query(
        r#"
        UPDATE api_keys
        SET revoked_at = NOW()
        WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
        "#,
    )
    .bind(id)
    .bind(user.user_id)
    .execute(&state.db)
    .await;

    match res {
        Ok(r) if r.rows_affected() > 0 => StatusCode::OK.into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"not_found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}
