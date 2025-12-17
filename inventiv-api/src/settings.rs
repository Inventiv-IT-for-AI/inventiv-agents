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
use sqlx::FromRow;

// --- DTOs ---

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateProviderRequest {
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateRegionRequest {
    pub provider_id: Uuid,
    pub name: String,
    pub code: String,
    pub is_active: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateZoneRequest {
    pub region_id: Uuid,
    pub name: String,
    pub code: String,
    pub is_active: Option<bool>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateInstanceTypeRequest {
    pub provider_id: Uuid,
    pub name: String,
    pub code: String,
    pub gpu_count: i32,
    pub vram_per_gpu_gb: i32,
    pub cpu_count: Option<i32>,
    pub ram_gb: Option<i32>,
    pub bandwidth_bps: Option<i64>,
    pub cost_per_hour: Option<f64>,
    pub is_active: Option<bool>,
    pub allocation_params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct RegionSearchRow {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub provider_name: String,
    pub provider_code: Option<String>,
    pub name: String,
    pub code: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct ZoneSearchRow {
    pub id: Uuid,
    pub region_id: Uuid,
    pub region_name: String,
    pub region_code: Option<String>,
    pub provider_id: Uuid,
    pub provider_name: String,
    pub provider_code: Option<String>,
    pub name: String,
    pub code: Option<String>,
    pub is_active: bool,
}

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

#[derive(Debug, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct InstanceTypeSearchRow {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub provider_name: String,
    pub provider_code: Option<String>,
    pub name: String,
    pub code: Option<String>,
    pub gpu_count: i32,
    pub vram_per_gpu_gb: i32,
    pub is_active: bool,
    #[serde(default)]
    pub cost_per_hour: Option<f64>,
    #[serde(default)]
    pub cpu_count: i32,
    #[serde(default)]
    pub ram_gb: i32,
    #[serde(default)]
    pub bandwidth_bps: i64,
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
    /// Provider-specific allocation parameters (JSON).
    /// Example for Scaleway L4: {"scaleway":{"boot_image_id":"<uuid>"}}.
    pub allocation_params: Option<serde_json::Value>,
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
    post,
    path = "/regions",
    tag = "Settings",
    request_body = CreateRegionRequest,
    responses(
        (status = 201, description = "Region created", body = inventiv_common::Region),
        (status = 409, description = "Conflict")
    )
)]
pub async fn create_region(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRegionRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let is_active = req.is_active.unwrap_or(true);
    let res = sqlx::query_as::<_, Region>(
        r#"INSERT INTO regions (id, provider_id, name, code, is_active)
           VALUES ($1,$2,$3,$4,$5)
           RETURNING id, provider_id, name, code, is_active"#,
    )
    .bind(id)
    .bind(req.provider_id)
    .bind(req.name)
    .bind(req.code)
    .bind(is_active)
    .fetch_one(&state.db)
    .await;

    match res {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(e) => {
            let msg = e.to_string();
            let code = if msg.contains("duplicate key") { StatusCode::CONFLICT } else { StatusCode::INTERNAL_SERVER_ERROR };
            (code, Json(serde_json::json!({"error":"db_error","message": msg}))).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/regions/search",
    tag = "Settings",
    params(SearchQuery),
    responses((status = 200, description = "Search regions", body = SearchResponse<RegionSearchRow>))
)]
pub async fn search_regions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse<RegionSearchRow>> {
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
        FROM regions r
        JOIN providers p ON p.id = r.provider_id
        WHERE ($1::uuid IS NULL OR r.provider_id = $1)
          AND ($2::bool IS NULL OR r.is_active = $2)
          AND ($3::text IS NULL OR r.name ILIKE $3 OR r.code ILIKE $3)
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
        SELECT
          r.id,
          r.provider_id,
          p.name as provider_name,
          p.code as provider_code,
          r.name,
          r.code,
          r.is_active
        FROM regions r
        JOIN providers p ON p.id = r.provider_id
        WHERE ($1::uuid IS NULL OR r.provider_id = $1)
          AND ($2::bool IS NULL OR r.is_active = $2)
          AND ($3::text IS NULL OR r.name ILIKE $3 OR r.code ILIKE $3)
        ORDER BY {order_by} {dir}, id {dir}
        LIMIT $4 OFFSET $5
        "#
    );
    let rows: Vec<RegionSearchRow> = sqlx::query_as(&sql)
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
    post,
    path = "/zones",
    tag = "Settings",
    request_body = CreateZoneRequest,
    responses(
        (status = 201, description = "Zone created", body = inventiv_common::Zone),
        (status = 409, description = "Conflict")
    )
)]
pub async fn create_zone(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateZoneRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let is_active = req.is_active.unwrap_or(true);
    let res = sqlx::query_as::<_, Zone>(
        r#"INSERT INTO zones (id, region_id, name, code, is_active)
           VALUES ($1,$2,$3,$4,$5)
           RETURNING id, region_id, name, code, is_active"#,
    )
    .bind(id)
    .bind(req.region_id)
    .bind(req.name)
    .bind(req.code)
    .bind(is_active)
    .fetch_one(&state.db)
    .await;

    match res {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(e) => {
            let msg = e.to_string();
            let code = if msg.contains("duplicate key") { StatusCode::CONFLICT } else { StatusCode::INTERNAL_SERVER_ERROR };
            (code, Json(serde_json::json!({"error":"db_error","message": msg}))).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/zones/search",
    tag = "Settings",
    params(SearchQuery),
    responses((status = 200, description = "Search zones", body = SearchResponse<ZoneSearchRow>))
)]
pub async fn search_zones(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse<ZoneSearchRow>> {
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
        FROM zones z
        JOIN regions r ON r.id = z.region_id
        JOIN providers p ON p.id = r.provider_id
        WHERE ($1::uuid IS NULL OR z.region_id = $1)
          AND ($2::uuid IS NULL OR r.provider_id = $2)
          AND ($3::bool IS NULL OR z.is_active = $3)
          AND ($4::text IS NULL OR z.name ILIKE $4 OR z.code ILIKE $4)
        "#,
    )
    .bind(params.region_id)
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
        SELECT
          z.id,
          z.region_id,
          r.name as region_name,
          r.code as region_code,
          r.provider_id,
          p.name as provider_name,
          p.code as provider_code,
          z.name,
          z.code,
          z.is_active
        FROM zones z
        JOIN regions r ON r.id = z.region_id
        JOIN providers p ON p.id = r.provider_id
        WHERE ($1::uuid IS NULL OR z.region_id = $1)
          AND ($2::uuid IS NULL OR r.provider_id = $2)
          AND ($3::bool IS NULL OR z.is_active = $3)
          AND ($4::text IS NULL OR z.name ILIKE $4 OR z.code ILIKE $4)
        ORDER BY {order_by} {dir}, id {dir}
        LIMIT $5 OFFSET $6
        "#
    );
    let rows: Vec<ZoneSearchRow> = sqlx::query_as(&sql)
        .bind(params.region_id)
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
    post,
    path = "/instance_types",
    tag = "Settings",
    request_body = CreateInstanceTypeRequest,
    responses(
        (status = 201, description = "Instance type created", body = inventiv_common::InstanceType),
        (status = 409, description = "Conflict")
    )
)]
pub async fn create_instance_type(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateInstanceTypeRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let is_active = req.is_active.unwrap_or(true);
    let cpu_count = req.cpu_count.unwrap_or(0);
    let ram_gb = req.ram_gb.unwrap_or(0);
    let bandwidth_bps = req.bandwidth_bps.unwrap_or(0);
    let allocation_params = req
        .allocation_params
        .map(sqlx::types::Json)
        .unwrap_or_else(|| sqlx::types::Json(serde_json::json!({})));

    // Note: cost_per_hour is numeric in DB; SQLx maps it to numeric; in common it is Option<f64>.
    // We'll cast to float8 on read paths; for insert we bind f64 and rely on sqlx to cast.
    let _ = sqlx::query(
        r#"INSERT INTO instance_types
           (id, provider_id, name, code, gpu_count, vram_per_gpu_gb, cpu_count, ram_gb, bandwidth_bps, is_active, cost_per_hour, allocation_params)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"#,
    )
    .bind(id)
    .bind(req.provider_id)
    .bind(req.name)
    .bind(req.code)
    .bind(req.gpu_count)
    .bind(req.vram_per_gpu_gb)
    .bind(cpu_count)
    .bind(ram_gb)
    .bind(bandwidth_bps)
    .bind(is_active)
    .bind(req.cost_per_hour)
    .bind(allocation_params)
    .execute(&state.db)
    .await;

    // Return the row using the existing struct shape
    let row = sqlx::query_as::<_, InstanceType>(
        r#"SELECT id, provider_id, name, code, gpu_count, vram_per_gpu_gb, is_active,
                  CAST(cost_per_hour as float8) as cost_per_hour,
                  cpu_count, ram_gb, bandwidth_bps
           FROM instance_types WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await;

    match row {
        Ok(it) => (StatusCode::CREATED, Json(it)).into_response(),
        Err(e) => {
            let msg = e.to_string();
            let code = if msg.contains("duplicate key") { StatusCode::CONFLICT } else { StatusCode::INTERNAL_SERVER_ERROR };
            (code, Json(serde_json::json!({"error":"db_error","message": msg}))).into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/instance_types/search",
    tag = "Settings",
    params(SearchQuery),
    responses((status = 200, description = "Search instance types", body = SearchResponse<InstanceTypeSearchRow>))
)]
pub async fn search_instance_types(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Json<SearchResponse<InstanceTypeSearchRow>> {
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
        FROM instance_types it
        JOIN providers p ON p.id = it.provider_id
        WHERE ($1::uuid IS NULL OR it.provider_id = $1)
          AND ($2::bool IS NULL OR it.is_active = $2)
          AND ($3::text IS NULL OR it.name ILIKE $3 OR it.code ILIKE $3)
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
          it.id,
          it.provider_id,
          p.name as provider_name,
          p.code as provider_code,
          it.name,
          it.code,
          it.gpu_count,
          it.vram_per_gpu_gb,
          it.cpu_count,
          it.ram_gb,
          it.bandwidth_bps,
          it.is_active,
          CAST(it.cost_per_hour AS DOUBLE PRECISION) as "cost_per_hour"
        FROM instance_types it
        JOIN providers p ON p.id = it.provider_id
        WHERE ($1::uuid IS NULL OR it.provider_id = $1)
          AND ($2::bool IS NULL OR it.is_active = $2)
          AND ($3::text IS NULL OR it.name ILIKE $3 OR it.code ILIKE $3)
        ORDER BY {order_by} {dir}, id {dir}
        LIMIT $4 OFFSET $5
        "#
    );
    let rows: Vec<InstanceTypeSearchRow> = sqlx::query_as(&sql)
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
            cost_per_hour = COALESCE($4, cost_per_hour),
            allocation_params = COALESCE($5, allocation_params)
         WHERE id = $6"
    )
    .bind(req.code)
    .bind(req.name)
    .bind(req.is_active)
    .bind(req.cost_per_hour)
    .bind(req.allocation_params)
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
    post,
    path = "/providers",
    tag = "Settings",
    request_body = CreateProviderRequest,
    responses(
        (status = 201, description = "Provider created", body = inventiv_common::Provider),
        (status = 409, description = "Conflict")
    )
)]
pub async fn create_provider(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateProviderRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let is_active = req.is_active.unwrap_or(true);
    let res = sqlx::query_as::<_, Provider>(
        r#"INSERT INTO providers (id, name, code, description, is_active)
           VALUES ($1,$2,$3,$4,$5)
           RETURNING id, name, code, description, is_active"#,
    )
    .bind(id)
    .bind(req.name)
    .bind(req.code)
    .bind(req.description)
    .bind(is_active)
    .fetch_one(&state.db)
    .await;

    match res {
        Ok(row) => (StatusCode::CREATED, Json(row)).into_response(),
        Err(e) => {
            let msg = e.to_string();
            let code = if msg.contains("duplicate key") { StatusCode::CONFLICT } else { StatusCode::INTERNAL_SERVER_ERROR };
            (code, Json(serde_json::json!({"error":"db_error","message": msg}))).into_response()
        }
    }
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
