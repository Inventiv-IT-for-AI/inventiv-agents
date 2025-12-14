use axum::{
    extract::{State, Path},
    extract::Query,
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use inventiv_common::{Provider, Region, Zone, InstanceType};
use crate::AppState;

// --- DTOs ---

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct SearchQuery {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub q: Option<String>,
    pub is_active: Option<bool>,
    pub order_by: Option<String>,
    pub order_dir: Option<String>, // "asc" | "desc"
    // Optional foreign-key filters
    pub provider_id: Option<Uuid>,
    pub region_id: Option<Uuid>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SearchResponse<T> {
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub filtered_count: i64,
    pub rows: Vec<T>,
}

fn dir_sql(dir: Option<&str>) -> &'static str {
    match dir.unwrap_or("asc").to_ascii_lowercase().as_str() {
        "desc" => "DESC",
        _ => "ASC",
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateRegionRequest {
    pub code: Option<String>,
    pub name: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateZoneRequest {
    pub code: Option<String>,
    pub name: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateInstanceTypeRequest {
    pub code: Option<String>,
    pub name: Option<String>,
    pub is_active: Option<bool>,
    pub cost_per_hour: Option<f64>,
}

// --- Handlers ---

// Regions
#[utoipa::path(
    get,
    path = "/regions",
    tag = "Settings",
    responses(
        (status = 200, description = "List all regions", body = Vec<Region>)
    )
)]
pub async fn list_regions(State(state): State<Arc<AppState>>) -> Json<Vec<Region>> {
    let regions = sqlx::query_as::<_, Region>(
        "SELECT id, provider_id, name, code, is_active FROM regions ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    Json(regions)
}

#[utoipa::path(
    get,
    path = "/regions/search",
    tag = "Settings",
    params(SearchQuery),
    responses((status = 200, description = "Search regions", body = SearchResponse<Region>))
)]
pub async fn search_regions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse<Region>> {
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);
    let q_like: Option<String> = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s));

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM regions")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let filtered_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM regions
        WHERE ($1::uuid IS NULL OR provider_id = $1)
          AND ($2::bool IS NULL OR is_active = $2)
          AND ($3::text IS NULL OR name ILIKE $3 OR code ILIKE $3)
        "#,
    )
    .bind(params.provider_id)
    .bind(params.is_active)
    .bind(q_like.as_deref())
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let order_by = match params.order_by.as_deref() {
        Some("code") => "code",
        Some("is_active") => "is_active",
        _ => "name",
    };
    let dir = dir_sql(params.order_dir.as_deref());

    let sql = format!(
        r#"
        SELECT id, provider_id, name, code, is_active
        FROM regions
        WHERE ($1::uuid IS NULL OR provider_id = $1)
          AND ($2::bool IS NULL OR is_active = $2)
          AND ($3::text IS NULL OR name ILIKE $3 OR code ILIKE $3)
        ORDER BY {order_by} {dir}, id {dir}
        LIMIT $4 OFFSET $5
        "#
    );
    let rows: Vec<Region> = sqlx::query_as(&sql)
        .bind(params.provider_id)
        .bind(params.is_active)
        .bind(q_like.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(SearchResponse { offset, limit, total_count, filtered_count, rows })
}

#[utoipa::path(
    put,
    path = "/regions/{id}",
    tag = "Settings",
    request_body = UpdateRegionRequest,
    responses(
        (status = 200, description = "Region updated"),
        (status = 404, description = "Region not found")
    )
)]
pub async fn update_region(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateRegionRequest>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "UPDATE regions SET 
            code = COALESCE($1, code), 
            name = COALESCE($2, name), 
            is_active = COALESCE($3, is_active)
         WHERE id = $4"
    )
    .bind(req.code)
    .bind(req.name)
    .bind(req.is_active)
    .bind(id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() > 0 {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            }
        },
        Err(e) => {
            eprintln!("Error updating region: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}


// Zones
#[utoipa::path(
    get,
    path = "/zones",
    tag = "Settings",
    responses(
        (status = 200, description = "List all zones", body = Vec<Zone>)
    )
)]
pub async fn list_zones(State(state): State<Arc<AppState>>) -> Json<Vec<Zone>> {
    let zones = sqlx::query_as::<_, Zone>(
        "SELECT id, region_id, name, code, is_active FROM zones ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    Json(zones)
}

#[utoipa::path(
    get,
    path = "/zones/search",
    tag = "Settings",
    params(SearchQuery),
    responses((status = 200, description = "Search zones", body = SearchResponse<Zone>))
)]
pub async fn search_zones(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse<Zone>> {
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);
    let q_like: Option<String> = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s));

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM zones")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let filtered_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM zones
        WHERE ($1::uuid IS NULL OR region_id = $1)
          AND ($2::bool IS NULL OR is_active = $2)
          AND ($3::text IS NULL OR name ILIKE $3 OR code ILIKE $3)
        "#,
    )
    .bind(params.region_id)
    .bind(params.is_active)
    .bind(q_like.as_deref())
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let order_by = match params.order_by.as_deref() {
        Some("code") => "code",
        Some("is_active") => "is_active",
        _ => "name",
    };
    let dir = dir_sql(params.order_dir.as_deref());

    let sql = format!(
        r#"
        SELECT id, region_id, name, code, is_active
        FROM zones
        WHERE ($1::uuid IS NULL OR region_id = $1)
          AND ($2::bool IS NULL OR is_active = $2)
          AND ($3::text IS NULL OR name ILIKE $3 OR code ILIKE $3)
        ORDER BY {order_by} {dir}, id {dir}
        LIMIT $4 OFFSET $5
        "#
    );
    let rows: Vec<Zone> = sqlx::query_as(&sql)
        .bind(params.region_id)
        .bind(params.is_active)
        .bind(q_like.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(SearchResponse { offset, limit, total_count, filtered_count, rows })
}

#[utoipa::path(
    put,
    path = "/zones/{id}",
    tag = "Settings",
    request_body = UpdateZoneRequest,
    responses(
        (status = 200, description = "Zone updated"),
        (status = 404, description = "Zone not found")
    )
)]
pub async fn update_zone(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateZoneRequest>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "UPDATE zones SET 
            code = COALESCE($1, code), 
            name = COALESCE($2, name), 
            is_active = COALESCE($3, is_active)
         WHERE id = $4"
    )
    .bind(req.code)
    .bind(req.name)
    .bind(req.is_active)
    .bind(id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() > 0 {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            }
        },
        Err(e) => {
            eprintln!("Error updating zone: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

// Instance Types
#[utoipa::path(
    get,
    path = "/instance_types",
    tag = "Settings",
    responses(
        (status = 200, description = "List all instance types", body = Vec<InstanceType>)
    )
)]
pub async fn list_instance_types(State(state): State<Arc<AppState>>) -> Json<Vec<InstanceType>> {
    let types = sqlx::query_as::<_, InstanceType>(
        r#"SELECT 
            id, provider_id, name, code, 
            gpu_count, vram_per_gpu_gb, 
            cpu_count, ram_gb, bandwidth_bps,
            is_active, 
            CAST(cost_per_hour AS DOUBLE PRECISION) as "cost_per_hour"
           FROM instance_types ORDER BY name"#
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    Json(types)
}

#[utoipa::path(
    get,
    path = "/instance_types/search",
    tag = "Settings",
    params(SearchQuery),
    responses((status = 200, description = "Search instance types", body = SearchResponse<InstanceType>))
)]
pub async fn search_instance_types(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse<InstanceType>> {
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);
    let q_like: Option<String> = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s));

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance_types")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let filtered_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM instance_types
        WHERE ($1::uuid IS NULL OR provider_id = $1)
          AND ($2::bool IS NULL OR is_active = $2)
          AND ($3::text IS NULL OR name ILIKE $3 OR code ILIKE $3)
        "#,
    )
    .bind(params.provider_id)
    .bind(params.is_active)
    .bind(q_like.as_deref())
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let order_by = match params.order_by.as_deref() {
        Some("code") => "code",
        Some("gpu_count") => "gpu_count",
        Some("vram_per_gpu_gb") => "vram_per_gpu_gb",
        Some("cost_per_hour") => "cost_per_hour",
        Some("is_active") => "is_active",
        _ => "name",
    };
    let dir = dir_sql(params.order_dir.as_deref());

    let sql = format!(
        r#"
        SELECT
          id, provider_id, name, code,
          gpu_count, vram_per_gpu_gb,
          cpu_count, ram_gb, bandwidth_bps,
          is_active,
          CAST(cost_per_hour AS DOUBLE PRECISION) as "cost_per_hour"
        FROM instance_types
        WHERE ($1::uuid IS NULL OR provider_id = $1)
          AND ($2::bool IS NULL OR is_active = $2)
          AND ($3::text IS NULL OR name ILIKE $3 OR code ILIKE $3)
        ORDER BY {order_by} {dir}, id {dir}
        LIMIT $4 OFFSET $5
        "#
    );
    let rows: Vec<InstanceType> = sqlx::query_as(&sql)
        .bind(params.provider_id)
        .bind(params.is_active)
        .bind(q_like.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(SearchResponse { offset, limit, total_count, filtered_count, rows })
}

#[utoipa::path(
    put,
    path = "/instance_types/{id}",
    tag = "Settings",
    request_body = UpdateInstanceTypeRequest,
    responses(
        (status = 200, description = "Instance Type updated"),
        (status = 404, description = "Instance Type not found")
    )
)]
pub async fn update_instance_type(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateInstanceTypeRequest>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "UPDATE instance_types SET 
            code = COALESCE($1, code), 
            name = COALESCE($2, name), 
            is_active = COALESCE($3, is_active),
            cost_per_hour = COALESCE($4, cost_per_hour)
         WHERE id = $5"
    )
    .bind(req.code)
    .bind(req.name)
    .bind(req.is_active)
    .bind(req.cost_per_hour)
    .bind(id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() > 0 {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            }
        },
        Err(e) => {
            eprintln!("Error updating instance type: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateProviderRequest {
    pub name: Option<String>,
    pub code: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

// Providers Handlers

#[utoipa::path(
    get,
    path = "/providers",
    tag = "Settings",
    responses(
        (status = 200, description = "List all providers", body = Vec<Provider>)
    )
)]
pub async fn list_providers(State(state): State<Arc<AppState>>) -> Json<Vec<Provider>> {
    let providers = sqlx::query_as::<_, Provider>(
        "SELECT id, name, code, description, is_active FROM providers ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    Json(providers)
}

#[utoipa::path(
    get,
    path = "/providers/search",
    tag = "Settings",
    params(SearchQuery),
    responses((status = 200, description = "Search providers", body = SearchResponse<Provider>))
)]
pub async fn search_providers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse<Provider>> {
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);
    let q_like: Option<String> = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s));

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM providers")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let filtered_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM providers
        WHERE ($1::bool IS NULL OR is_active = $1)
          AND (
            $2::text IS NULL
            OR name ILIKE $2
            OR code ILIKE $2
            OR COALESCE(description, '') ILIKE $2
          )
        "#,
    )
    .bind(params.is_active)
    .bind(q_like.as_deref())
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let order_by = match params.order_by.as_deref() {
        Some("code") => "code",
        Some("is_active") => "is_active",
        _ => "name",
    };
    let dir = dir_sql(params.order_dir.as_deref());

    let sql = format!(
        r#"
        SELECT id, name, code, description, is_active
        FROM providers
        WHERE ($1::bool IS NULL OR is_active = $1)
          AND (
            $2::text IS NULL
            OR name ILIKE $2
            OR code ILIKE $2
            OR COALESCE(description, '') ILIKE $2
          )
        ORDER BY {order_by} {dir}, id {dir}
        LIMIT $3 OFFSET $4
        "#
    );
    let rows: Vec<Provider> = sqlx::query_as(&sql)
        .bind(params.is_active)
        .bind(q_like.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(SearchResponse { offset, limit, total_count, filtered_count, rows })
}

#[utoipa::path(
    put,
    path = "/providers/{id}",
    tag = "Settings",
    request_body = UpdateProviderRequest,
    responses(
        (status = 200, description = "Provider updated"),
        (status = 404, description = "Provider not found")
    )
)]
pub async fn update_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProviderRequest>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "UPDATE providers SET 
            name = COALESCE($1, name), 
            code = COALESCE($2, code),
            description = COALESCE($3, description), 
            is_active = COALESCE($4, is_active)
         WHERE id = $5"
    )
    .bind(req.name)
    .bind(req.code)
    .bind(req.description)
    .bind(req.is_active)
    .bind(id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() > 0 {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            }
        },
        Err(e) => {
            eprintln!("Error updating provider: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
