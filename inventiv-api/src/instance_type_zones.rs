use axum::{
    extract::{State, Path},
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use inventiv_common::InstanceType;
use crate::{AppState, auth, user_locale};

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
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Path(instance_type_id): Path<Uuid>,
) -> Json<Vec<InstanceTypeZoneAssociation>> {
    let locale = user_locale::preferred_locale_code(&state.db, user.user_id).await;
    let associations = sqlx::query_as::<_, (Uuid, Uuid, bool, String, String)>(
        r#"SELECT 
            itz.instance_type_id,
            itz.zone_id,
            itz.is_available,
            COALESCE(i18n_get_text(z.name_i18n_id, $2), z.name) as zone_name,
            z.code as zone_code
           FROM instance_type_zones itz
           JOIN zones z ON itz.zone_id = z.id
           WHERE itz.instance_type_id = $1
           ORDER BY z.name"#
    )
    .bind(instance_type_id)
    .bind(locale)
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
) -> impl IntoResponse {
    // Domain safety: instance types are provider-scoped.
    // Reject any association where zones are not from the same provider as the instance type.
    let it_provider: Option<Uuid> = sqlx::query_scalar("SELECT provider_id FROM instance_types WHERE id = $1")
        .bind(instance_type_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    let Some(it_provider) = it_provider else {
        return (StatusCode::NOT_FOUND, "Instance type not found".to_string());
    };

    if !req.zone_ids.is_empty() {
        // Count zones that belong to the instance type provider.
        let ok_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM zones z
            JOIN regions r ON r.id = z.region_id
            WHERE z.id = ANY($1)
              AND r.provider_id = $2
            "#,
        )
        .bind(&req.zone_ids)
        .bind(it_provider)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

        if ok_count != req.zone_ids.len() as i64 {
            return (
                StatusCode::BAD_REQUEST,
                "Invalid zone_ids: zones must belong to the same provider as the instance type".to_string(),
            );
        }
    }

    // Delete existing associations
    let delete_result = sqlx::query("DELETE FROM instance_type_zones WHERE instance_type_id = $1")
        .bind(instance_type_id)
        .execute(&state.db)
        .await;

    if delete_result.is_err() {
        eprintln!("Error deleting existing associations: {:?}", delete_result.err());
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update associations".to_string());
    }

    // Insert new associations
    for zone_id in req.zone_ids {
        let insert_new = sqlx::query(
            "INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available) VALUES ($1, $2, true)"
        )
        .bind(instance_type_id)
        .bind(zone_id)
        .execute(&state.db)
        .await;

        if insert_new.is_err() {
            eprintln!("Error inserting instance_type_zones association: {:?}", insert_new.err());
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update associations".to_string());
        }

    }

    (StatusCode::OK, "OK".to_string())
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
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Path(zone_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<ListInstanceTypesForZoneQuery>,
) -> Json<Vec<InstanceType>> {
    let locale = user_locale::preferred_locale_code(&state.db, user.user_id).await;
    // Prefer provider_code, keep provider_id for backward compatibility.
    let provider_filter: Option<Uuid> = if let Some(pid) = params.provider_id {
        Some(pid)
    } else if let Some(code) = params
        .provider_code
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        sqlx::query_scalar("SELECT id FROM providers WHERE code = $1 LIMIT 1")
            .bind(code.to_ascii_lowercase())
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None)
    } else {
        None
    };

    let types = sqlx::query_as::<_, InstanceType>(
        r#"SELECT DISTINCT
            it.id,
            it.provider_id,
            COALESCE(i18n_get_text(it.name_i18n_id, $3), it.name) as name,
            it.code, 
            it.gpu_count, it.vram_per_gpu_gb, 
            it.cpu_count, it.ram_gb, it.bandwidth_bps,
            it.is_active,
            CAST(it.cost_per_hour AS DOUBLE PRECISION) as "cost_per_hour"
           FROM instance_types it
           JOIN instance_type_zones itz
             ON it.id = itz.instance_type_id AND itz.zone_id = $1
           JOIN providers p ON p.id = it.provider_id
           WHERE it.is_active = true
             AND p.is_active = true
             AND ($2::uuid IS NULL OR it.provider_id = $2::uuid)
             AND itz.is_available = true
           ORDER BY COALESCE(i18n_get_text(it.name_i18n_id, $3), it.name)"#
    )
    .bind(zone_id)
    .bind(provider_filter)
    .bind(locale)
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    Json(types)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ListInstanceTypesForZoneQuery {
    pub provider_code: Option<String>,
    pub provider_id: Option<Uuid>, // deprecated
}
