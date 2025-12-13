use axum::{
    extract::{State, Path},
    routing::{get, put},
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use inventiv_common::{Provider, Region, Zone, InstanceType};
use crate::AppState;

// --- DTOs for Zone Associations ---

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct InstanceTypeZoneAssociation {
    pub instance_type_id: Uuid,
    pub zone_id: Uuid,
    pub is_available: bool,
    pub zone_name: String,
    pub zone_code: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AssociateZoneRequest {
    pub zone_ids: Vec<Uuid>,
}

// Get zones associated with an instance type
#[utoipa::path(
    get,
    path = "/instance_types/{id}/zones",
    tag = "Settings",
    params(
        ("id" = Uuid, Path, description = "Instance Type ID")
    ),
    responses(
        (status = 200, description = "List of associated zones", body = Vec<InstanceTypeZoneAssociation>)
    )
)]
pub async fn list_instance_type_zones(
    State(state): State<Arc<AppState>>,
    Path(instance_type_id): Path<Uuid>,
) -> Json<Vec<InstanceTypeZoneAssociation>> {
    let associations = sqlx::query_as::<_, (Uuid, Uuid, bool, String, String)>(
        r#"SELECT 
            itz.instance_type_id,
            itz.zone_id,
            itz.is_available,
            z.name as zone_name,
            z.code as zone_code
           FROM instance_type_zones itz
           JOIN zones z ON itz.zone_id = z.id
           WHERE itz.instance_type_id = $1
           ORDER BY z.name"#
    )
    .bind(instance_type_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![])
    .into_iter()
    .map(|(instance_type_id, zone_id, is_available, zone_name, zone_code)| {
        InstanceTypeZoneAssociation {
            instance_type_id,
            zone_id,
            is_available,
            zone_name,
            zone_code,
        }
    })
    .collect();

    Json(associations)
}

// Associate zones with an instance type
#[utoipa::path(
    put,
    path = "/instance_types/{id}/zones",
    tag = "Settings",
    params(
        ("id" = Uuid, Path, description = "Instance Type ID")
    ),
    request_body = AssociateZoneRequest,
    responses(
        (status = 200, description = "Zones associated successfully"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn associate_zones_to_instance_type(
    State(state): State<Arc<AppState>>,
    Path(instance_type_id): Path<Uuid>,
    Json(req): Json<AssociateZoneRequest>,
) -> StatusCode {
    // Delete existing associations
    let delete_result = sqlx::query("DELETE FROM instance_type_zones WHERE instance_type_id = $1")
        .bind(instance_type_id)
        .execute(&state.db)
        .await;

    if delete_result.is_err() {
        eprintln!("Error deleting existing associations: {:?}", delete_result.err());
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Insert new associations
    for zone_id in req.zone_ids {
        let insert_result = sqlx::query(
            "INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available) VALUES ($1, $2, true)"
        )
        .bind(instance_type_id)
        .bind(zone_id)
        .execute(&state.db)
        .await;

        if insert_result.is_err() {
            eprintln!("Error inserting association: {:?}", insert_result.err());
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    StatusCode::OK
}

// Get instance types available in a specific zone (for dashboard filtering)
#[utoipa::path(
    get,
    path = "/zones/{zone_id}/instance_types",
    tag = "Settings",
    params(
        ("zone_id" = Uuid, Path, description = "Zone ID")
    ),
    responses(
        (status = 200, description = "List of available instance types", body = Vec<InstanceType>)
    )
)]
pub async fn list_instance_types_for_zone(
    State(state): State<Arc<AppState>>,
    Path(zone_id): Path<Uuid>,
) -> Json<Vec<InstanceType>> {
    let types = sqlx::query_as::<_, InstanceType>(
        r#"SELECT DISTINCT
            it.id, it.provider_id, it.name, it.code, 
            it.gpu_count, it.vram_per_gpu_gb, 
            it.cpu_count, it.ram_gb, it.n_gpu, it.bandwidth_bps,
            it.is_active,
            CAST(it.cost_per_hour AS DOUBLE PRECISION) as "cost_per_hour"
           FROM instance_types it
           JOIN instance_type_zones itz ON it.id = itz.instance_type_id
           WHERE itz.zone_id = $1
             AND it.is_active = true
             AND itz.is_available = true
           ORDER BY it.name"#
    )
    .bind(zone_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    Json(types)
}
