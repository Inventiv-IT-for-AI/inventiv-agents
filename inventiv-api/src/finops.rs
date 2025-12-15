use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres};
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct SeriesParams {
    pub minutes: Option<i64>,
    pub provider_id: Option<uuid::Uuid>,
    pub instance_id: Option<uuid::Uuid>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ForecastMinuteRow {
    pub bucket_minute: chrono::DateTime<chrono::Utc>,
    pub provider_id: Option<uuid::Uuid>,
    pub burn_rate_eur_per_hour: f64,
    pub forecast_eur_per_minute: f64,
    pub forecast_eur_per_hour: f64,
    pub forecast_eur_per_day: f64,
    pub forecast_eur_per_month_30d: f64,
    pub forecast_eur_per_year_365d: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ActualMinuteRow {
    pub bucket_minute: chrono::DateTime<chrono::Utc>,
    pub provider_id: Option<uuid::Uuid>,
    pub instance_id: Option<uuid::Uuid>,
    pub amount_eur: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CumulativeMinuteRow {
    pub bucket_minute: chrono::DateTime<chrono::Utc>,
    pub provider_id: Option<uuid::Uuid>,
    pub instance_id: Option<uuid::Uuid>,
    pub cumulative_amount_eur: f64,
}

#[derive(Serialize)]
pub struct CostCurrentResponse {
    pub latest_bucket_minute: Option<chrono::DateTime<chrono::Utc>>,
    pub forecast: Vec<ForecastMinuteRow>,
    pub cumulative_total: Option<CumulativeMinuteRow>,
}

fn default_minutes(v: Option<i64>) -> i64 {
    v.unwrap_or(60).clamp(1, 60 * 24 * 31)
}

pub async fn get_cost_current(State(state): State<Arc<AppState>>) -> Json<CostCurrentResponse> {
    let db = &state.db;

    let latest_bucket: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT MAX(bucket_minute) FROM finops.cost_forecast_minute",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .flatten();

    let forecast = if let Some(bucket) = latest_bucket {
        sqlx::query_as::<Postgres, ForecastMinuteRow>(
            r#"
            SELECT
              bucket_minute,
              provider_id,
              burn_rate_eur_per_hour::float8 as burn_rate_eur_per_hour,
              forecast_eur_per_minute::float8 as forecast_eur_per_minute,
              forecast_eur_per_hour::float8 as forecast_eur_per_hour,
              forecast_eur_per_day::float8 as forecast_eur_per_day,
              forecast_eur_per_month_30d::float8 as forecast_eur_per_month_30d,
              forecast_eur_per_year_365d::float8 as forecast_eur_per_year_365d
            FROM finops.cost_forecast_minute
            WHERE bucket_minute = $1
            ORDER BY provider_id NULLS FIRST
            "#,
        )
        .bind(bucket)
        .fetch_all(db)
        .await
        .unwrap_or_default()
    } else {
        vec![]
    };

    let cumulative_total = sqlx::query_as::<Postgres, CumulativeMinuteRow>(
        r#"
        SELECT
          bucket_minute,
          provider_id,
          instance_id,
          cumulative_amount_eur::float8 as cumulative_amount_eur
        FROM finops.cost_actual_cumulative_minute
        WHERE provider_id IS NULL AND instance_id IS NULL
        ORDER BY bucket_minute DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    Json(CostCurrentResponse {
        latest_bucket_minute: latest_bucket,
        forecast,
        cumulative_total,
    })
}

pub async fn get_cost_forecast_series(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SeriesParams>,
) -> Json<Vec<ForecastMinuteRow>> {
    let minutes = default_minutes(params.minutes);

    let rows = sqlx::query_as::<Postgres, ForecastMinuteRow>(
        r#"
        SELECT
          bucket_minute,
          provider_id,
          burn_rate_eur_per_hour::float8 as burn_rate_eur_per_hour,
          forecast_eur_per_minute::float8 as forecast_eur_per_minute,
          forecast_eur_per_hour::float8 as forecast_eur_per_hour,
          forecast_eur_per_day::float8 as forecast_eur_per_day,
          forecast_eur_per_month_30d::float8 as forecast_eur_per_month_30d,
          forecast_eur_per_year_365d::float8 as forecast_eur_per_year_365d
        FROM finops.cost_forecast_minute
        WHERE provider_id IS NOT DISTINCT FROM $1
        ORDER BY bucket_minute DESC
        LIMIT $2
        "#,
    )
    .bind(params.provider_id)
    .bind(minutes)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}

// -----------------------------------------------------------------------------
// Dashboard helpers: breakdown (provider/region/type/instance) + window rollups
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct BreakdownParams {
    pub limit_instances: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ProviderCostRow {
    pub provider_id: uuid::Uuid,
    pub provider_code: Option<String>,
    pub provider_name: String,
    pub amount_eur: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RegionCostRow {
    pub provider_id: uuid::Uuid,
    pub provider_code: Option<String>,
    pub region_id: uuid::Uuid,
    pub region_code: Option<String>,
    pub region_name: String,
    pub amount_eur: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct InstanceTypeCostRow {
    pub provider_id: uuid::Uuid,
    pub provider_code: Option<String>,
    pub instance_type_id: uuid::Uuid,
    pub instance_type_code: Option<String>,
    pub instance_type_name: String,
    pub amount_eur: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct InstanceCostRow {
    pub instance_id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub provider_code: Option<String>,
    pub provider_name: String,
    pub region_name: Option<String>,
    pub zone_name: Option<String>,
    pub instance_type_name: Option<String>,
    pub amount_eur: f64,
}

#[derive(Serialize)]
pub struct CostsDashboardResponse {
    pub bucket_minute: Option<chrono::DateTime<chrono::Utc>>,
    pub total_minute_eur: f64,
    pub by_provider_minute: Vec<ProviderCostRow>,
    pub by_region_minute: Vec<RegionCostRow>,
    pub by_instance_type_minute: Vec<InstanceTypeCostRow>,
    pub by_instance_minute: Vec<InstanceCostRow>,
}

fn default_limit_instances(v: Option<i64>) -> i64 {
    v.unwrap_or(100).clamp(1, 2000)
}

pub async fn get_costs_dashboard_current(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BreakdownParams>,
) -> Json<CostsDashboardResponse> {
    let db = &state.db;
    let limit_instances = default_limit_instances(params.limit_instances);

    let bucket: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT MAX(bucket_minute) FROM finops.cost_actual_minute")
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .flatten();

    if bucket.is_none() {
        return Json(CostsDashboardResponse {
            bucket_minute: None,
            total_minute_eur: 0.0,
            by_provider_minute: vec![],
            by_region_minute: vec![],
            by_instance_type_minute: vec![],
            by_instance_minute: vec![],
        });
    }
    let bucket = bucket.unwrap();

    let total_minute_eur: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(amount_eur::float8, 0)
        FROM finops.cost_actual_minute
        WHERE bucket_minute = $1
          AND provider_id IS NULL
          AND instance_id IS NULL
        "#,
    )
    .bind(bucket)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0.0);

    let by_provider_minute = sqlx::query_as::<Postgres, ProviderCostRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          p.name as provider_name,
          m.amount_eur::float8 as amount_eur
        FROM finops.cost_actual_minute m
        JOIN providers p ON p.id = m.provider_id
        WHERE m.bucket_minute = $1
          AND m.provider_id IS NOT NULL
          AND m.instance_id IS NULL
        ORDER BY amount_eur DESC
        "#,
    )
    .bind(bucket)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    // region/type/instance rely on per-instance minute rows
    let by_region_minute = sqlx::query_as::<Postgres, RegionCostRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          r.id as region_id,
          r.code as region_code,
          r.name as region_name,
          SUM(m.amount_eur)::float8 as amount_eur
        FROM finops.cost_actual_minute m
        JOIN instances i ON i.id = m.instance_id
        LEFT JOIN zones z ON z.id = i.zone_id
        LEFT JOIN regions r ON r.id = z.region_id
        JOIN providers p ON p.id = i.provider_id
        WHERE m.bucket_minute = $1
          AND m.instance_id IS NOT NULL
        GROUP BY p.id, p.code, r.id, r.code, r.name
        ORDER BY amount_eur DESC
        "#,
    )
    .bind(bucket)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let by_instance_type_minute = sqlx::query_as::<Postgres, InstanceTypeCostRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          it.id as instance_type_id,
          it.code as instance_type_code,
          it.name as instance_type_name,
          SUM(m.amount_eur)::float8 as amount_eur
        FROM finops.cost_actual_minute m
        JOIN instances i ON i.id = m.instance_id
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        JOIN providers p ON p.id = i.provider_id
        WHERE m.bucket_minute = $1
          AND m.instance_id IS NOT NULL
        GROUP BY p.id, p.code, it.id, it.code, it.name
        ORDER BY amount_eur DESC
        "#,
    )
    .bind(bucket)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let by_instance_minute = sqlx::query_as::<Postgres, InstanceCostRow>(
        r#"
        SELECT
          i.id as instance_id,
          p.id as provider_id,
          p.code as provider_code,
          p.name as provider_name,
          r.name as region_name,
          z.name as zone_name,
          it.name as instance_type_name,
          m.amount_eur::float8 as amount_eur
        FROM finops.cost_actual_minute m
        JOIN instances i ON i.id = m.instance_id
        JOIN providers p ON p.id = i.provider_id
        LEFT JOIN zones z ON z.id = i.zone_id
        LEFT JOIN regions r ON r.id = z.region_id
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE m.bucket_minute = $1
          AND m.instance_id IS NOT NULL
        ORDER BY amount_eur DESC
        LIMIT $2
        "#,
    )
    .bind(bucket)
    .bind(limit_instances)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    Json(CostsDashboardResponse {
        bucket_minute: Some(bucket),
        total_minute_eur,
        by_provider_minute,
        by_region_minute,
        by_instance_type_minute,
        by_instance_minute,
    })
}

pub async fn get_cost_actual_series(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SeriesParams>,
) -> Json<Vec<ActualMinuteRow>> {
    let minutes = default_minutes(params.minutes);

    let rows = sqlx::query_as::<Postgres, ActualMinuteRow>(
        r#"
        SELECT
          bucket_minute,
          provider_id,
          instance_id,
          amount_eur::float8 as amount_eur
        FROM finops.cost_actual_minute
        WHERE provider_id IS NOT DISTINCT FROM $1
          AND instance_id IS NOT DISTINCT FROM $2
        ORDER BY bucket_minute DESC
        LIMIT $3
        "#,
    )
    .bind(params.provider_id)
    .bind(params.instance_id)
    .bind(minutes)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}

pub async fn get_cost_cumulative_series(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SeriesParams>,
) -> Json<Vec<CumulativeMinuteRow>> {
    let minutes = default_minutes(params.minutes);

    let rows = sqlx::query_as::<Postgres, CumulativeMinuteRow>(
        r#"
        SELECT
          bucket_minute,
          provider_id,
          instance_id,
          cumulative_amount_eur::float8 as cumulative_amount_eur
        FROM finops.cost_actual_cumulative_minute
        WHERE provider_id IS NOT DISTINCT FROM $1
          AND instance_id IS NOT DISTINCT FROM $2
        ORDER BY bucket_minute DESC
        LIMIT $3
        "#,
    )
    .bind(params.provider_id)
    .bind(params.instance_id)
    .bind(minutes)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}

