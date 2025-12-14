use axum::{
    extract::{State, Path},
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use inventiv_common::{Provider, Region, Zone, InstanceType};
use crate::AppState;

// --- DTOs ---

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
            cpu_count, ram_gb, n_gpu, bandwidth_bps,
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
