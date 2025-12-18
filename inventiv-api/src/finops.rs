use axum::extract::{Query, State};
use axum::Json;
use chrono::Timelike;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use std::sync::Arc;

use crate::AppState;

// -----------------------------------------------------------------------------
// Shared query params
// -----------------------------------------------------------------------------

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

    let latest_bucket: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT MAX(bucket_minute) FROM finops.cost_forecast_minute")
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

// -----------------------------------------------------------------------------
// FinOps Dashboard v2: allocation (current) + spend windows + breakdown windows
// -----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct WindowSpendRow {
    pub window: String, // "minute" | "hour" | "day" | "month_30d" | "year_365d"
    pub minutes: i64,
    pub actual_spend_eur: f64,
}

#[derive(Serialize)]
pub struct ForecastTotalsRow {
    pub burn_rate_eur_per_hour: f64,
    pub forecast_eur_per_minute: f64,
    pub forecast_eur_per_hour: f64,
    pub forecast_eur_per_day: f64,
    pub forecast_eur_per_month_30d: f64,
    pub forecast_eur_per_year_365d: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AllocationProviderRow {
    pub provider_id: uuid::Uuid,
    pub provider_code: Option<String>,
    pub provider_name: String,
    pub burn_rate_eur_per_hour: f64,
    pub forecast_eur_per_minute: f64,
    pub forecast_eur_per_hour: f64,
    pub forecast_eur_per_day: f64,
    pub forecast_eur_per_month_30d: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AllocationRegionRow {
    pub provider_id: uuid::Uuid,
    pub provider_code: Option<String>,
    pub region_id: Option<uuid::Uuid>,
    pub region_code: Option<String>,
    pub region_name: Option<String>,
    pub burn_rate_eur_per_hour: f64,
    pub forecast_eur_per_minute: f64,
    pub forecast_eur_per_hour: f64,
    pub forecast_eur_per_day: f64,
    pub forecast_eur_per_month_30d: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AllocationInstanceTypeRow {
    pub provider_id: uuid::Uuid,
    pub provider_code: Option<String>,
    pub instance_type_id: Option<uuid::Uuid>,
    pub instance_type_code: Option<String>,
    pub instance_type_name: Option<String>,
    pub burn_rate_eur_per_hour: f64,
    pub forecast_eur_per_minute: f64,
    pub forecast_eur_per_hour: f64,
    pub forecast_eur_per_day: f64,
    pub forecast_eur_per_month_30d: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AllocationInstanceRow {
    pub instance_id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub provider_code: Option<String>,
    pub provider_name: String,
    pub region_name: Option<String>,
    pub zone_name: Option<String>,
    pub instance_type_name: Option<String>,
    pub burn_rate_eur_per_hour: f64,
    pub forecast_eur_per_minute: f64,
    pub forecast_eur_per_hour: f64,
    pub forecast_eur_per_day: f64,
    pub forecast_eur_per_month_30d: f64,
}

#[derive(Serialize)]
pub struct AllocationDashboardResponse {
    pub at_minute: chrono::DateTime<chrono::Utc>,
    pub total: ForecastTotalsRow,
    pub by_provider: Vec<AllocationProviderRow>,
    pub by_region: Vec<AllocationRegionRow>,
    pub by_instance_type: Vec<AllocationInstanceTypeRow>,
    pub by_instance: Vec<AllocationInstanceRow>,
}

#[derive(Serialize)]
pub struct CostsDashboardSummaryResponse {
    pub latest_bucket_minute: Option<chrono::DateTime<chrono::Utc>>,
    pub allocation: AllocationDashboardResponse,
    pub actual_spend_windows: Vec<WindowSpendRow>,
    pub cumulative_total: Option<CumulativeMinuteRow>,
}

#[derive(Deserialize)]
pub struct SummaryParams {
    pub limit_instances: Option<i64>,
}

fn now_minute_bucket(now: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    now.with_second(0)
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(now)
}

fn window_to_minutes(window: &str) -> Option<i64> {
    match window.to_ascii_lowercase().as_str() {
        "minute" | "1m" => Some(1),
        "hour" | "1h" => Some(60),
        "day" | "1d" => Some(60 * 24),
        "month_30d" | "30d" => Some(60 * 24 * 30),
        "year_365d" | "365d" => Some(60 * 24 * 365),
        _ => None,
    }
}

async fn cumulative_total_at_or_before(
    db: &sqlx::Pool<Postgres>,
    bucket: chrono::DateTime<chrono::Utc>,
) -> Option<f64> {
    sqlx::query_scalar(
        r#"
        SELECT cumulative_amount_eur::float8
        FROM finops.cost_actual_cumulative_minute
        WHERE provider_id IS NULL AND instance_id IS NULL
          AND bucket_minute <= $1
        ORDER BY bucket_minute DESC
        LIMIT 1
        "#,
    )
    .bind(bucket)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

fn forecast_totals_from_burn_rate(burn_rate_eur_per_hour: f64) -> ForecastTotalsRow {
    let per_minute = burn_rate_eur_per_hour / 60.0;
    let per_hour = burn_rate_eur_per_hour;
    let per_day = burn_rate_eur_per_hour * 24.0;
    let per_month_30d = per_day * 30.0;
    let per_year_365d = per_day * 365.0;
    ForecastTotalsRow {
        burn_rate_eur_per_hour,
        forecast_eur_per_minute: per_minute,
        forecast_eur_per_hour: per_hour,
        forecast_eur_per_day: per_day,
        forecast_eur_per_month_30d: per_month_30d,
        forecast_eur_per_year_365d: per_year_365d,
    }
}

pub async fn get_costs_dashboard_summary(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SummaryParams>,
) -> Json<CostsDashboardSummaryResponse> {
    let db = &state.db;
    let limit_instances = default_limit_instances(params.limit_instances);

    // Latest computed minute (actual/cumulative are for "last complete minute").
    let latest_bucket: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT MAX(bucket_minute) FROM finops.cost_actual_cumulative_minute WHERE provider_id IS NULL AND instance_id IS NULL",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .flatten();

    let cumulative_total = if let Some(b) = latest_bucket {
        sqlx::query_as::<Postgres, CumulativeMinuteRow>(
            r#"
            SELECT
              bucket_minute,
              provider_id,
              instance_id,
              cumulative_amount_eur::float8 as cumulative_amount_eur
            FROM finops.cost_actual_cumulative_minute
            WHERE bucket_minute = $1
              AND provider_id IS NULL AND instance_id IS NULL
            LIMIT 1
            "#,
        )
        .bind(b)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // Actual spend windows computed from cumulative deltas (best-effort if buckets are missing).
    let mut actual_spend_windows: Vec<WindowSpendRow> = Vec::new();
    let windows = [
        ("minute", 1),
        ("hour", 60),
        ("day", 60 * 24),
        ("month_30d", 60 * 24 * 30),
        ("year_365d", 60 * 24 * 365),
    ];

    if let Some(end_bucket) = latest_bucket {
        let end_cum = cumulative_total_at_or_before(db, end_bucket)
            .await
            .unwrap_or(0.0);
        for (label, mins) in windows {
            let start_bucket = end_bucket - chrono::Duration::minutes((mins - 1).max(0));
            let prev_bucket = start_bucket - chrono::Duration::minutes(1);
            let prev_cum = cumulative_total_at_or_before(db, prev_bucket)
                .await
                .unwrap_or(0.0);
            actual_spend_windows.push(WindowSpendRow {
                window: label.to_string(),
                minutes: mins,
                actual_spend_eur: (end_cum - prev_cum).max(0.0),
            });
        }
    } else {
        for (label, mins) in windows {
            actual_spend_windows.push(WindowSpendRow {
                window: label.to_string(),
                minutes: mins,
                actual_spend_eur: 0.0,
            });
        }
    }

    // Allocation (current) snapshot: derived directly from instances + instance_types cost_per_hour.
    // Use "now minute bucket" so the UI has a stable timestamp.
    let at_minute = now_minute_bucket(chrono::Utc::now());

    let total_burn_rate: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0)::float8
        FROM instances i
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE i.is_archived = false
          AND i.provider_instance_id IS NOT NULL
          AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
          AND i.created_at <= $1
          AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        "#,
    )
    .bind(at_minute)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0.0);

    let by_provider = sqlx::query_as::<Postgres, AllocationProviderRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          p.name as provider_name,
          COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0)::float8 as burn_rate_eur_per_hour,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) / 60.0)::float8 as forecast_eur_per_minute,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0))::float8 as forecast_eur_per_hour,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) * 24.0)::float8 as forecast_eur_per_day,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) * 24.0 * 30.0)::float8 as forecast_eur_per_month_30d
        FROM instances i
        JOIN providers p ON p.id = i.provider_id
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE i.is_archived = false
          AND i.provider_instance_id IS NOT NULL
          AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
          AND i.created_at <= $1
          AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        GROUP BY p.id, p.code, p.name
        ORDER BY burn_rate_eur_per_hour DESC
        "#,
    )
    .bind(at_minute)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let by_region = sqlx::query_as::<Postgres, AllocationRegionRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          r.id as region_id,
          r.code as region_code,
          r.name as region_name,
          COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0)::float8 as burn_rate_eur_per_hour,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) / 60.0)::float8 as forecast_eur_per_minute,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0))::float8 as forecast_eur_per_hour,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) * 24.0)::float8 as forecast_eur_per_day,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) * 24.0 * 30.0)::float8 as forecast_eur_per_month_30d
        FROM instances i
        JOIN providers p ON p.id = i.provider_id
        LEFT JOIN zones z ON z.id = i.zone_id
        LEFT JOIN regions r ON r.id = z.region_id
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE i.is_archived = false
          AND i.provider_instance_id IS NOT NULL
          AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
          AND i.created_at <= $1
          AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        GROUP BY p.id, p.code, r.id, r.code, r.name
        ORDER BY burn_rate_eur_per_hour DESC
        "#,
    )
    .bind(at_minute)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let by_instance_type = sqlx::query_as::<Postgres, AllocationInstanceTypeRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          it.id as instance_type_id,
          it.code as instance_type_code,
          it.name as instance_type_name,
          COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0)::float8 as burn_rate_eur_per_hour,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) / 60.0)::float8 as forecast_eur_per_minute,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0))::float8 as forecast_eur_per_hour,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) * 24.0)::float8 as forecast_eur_per_day,
          (COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) * 24.0 * 30.0)::float8 as forecast_eur_per_month_30d
        FROM instances i
        JOIN providers p ON p.id = i.provider_id
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE i.is_archived = false
          AND i.provider_instance_id IS NOT NULL
          AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
          AND i.created_at <= $1
          AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        GROUP BY p.id, p.code, it.id, it.code, it.name
        ORDER BY burn_rate_eur_per_hour DESC
        "#,
    )
    .bind(at_minute)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let by_instance = sqlx::query_as::<Postgres, AllocationInstanceRow>(
        r#"
        SELECT
          i.id as instance_id,
          p.id as provider_id,
          p.code as provider_code,
          p.name as provider_name,
          r.name as region_name,
          z.name as zone_name,
          it.name as instance_type_name,
          COALESCE(it.cost_per_hour, 0)::float8 as burn_rate_eur_per_hour,
          (COALESCE(it.cost_per_hour, 0) / 60.0)::float8 as forecast_eur_per_minute,
          (COALESCE(it.cost_per_hour, 0))::float8 as forecast_eur_per_hour,
          (COALESCE(it.cost_per_hour, 0) * 24.0)::float8 as forecast_eur_per_day,
          (COALESCE(it.cost_per_hour, 0) * 24.0 * 30.0)::float8 as forecast_eur_per_month_30d
        FROM instances i
        JOIN providers p ON p.id = i.provider_id
        LEFT JOIN zones z ON z.id = i.zone_id
        LEFT JOIN regions r ON r.id = z.region_id
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE i.is_archived = false
          AND i.provider_instance_id IS NOT NULL
          AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
          AND i.created_at <= $1
          AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        ORDER BY burn_rate_eur_per_hour DESC, i.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(at_minute)
    .bind(limit_instances)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let allocation = AllocationDashboardResponse {
        at_minute,
        total: forecast_totals_from_burn_rate(total_burn_rate),
        by_provider,
        by_region,
        by_instance_type,
        by_instance,
    };

    Json(CostsDashboardSummaryResponse {
        latest_bucket_minute: latest_bucket,
        allocation,
        actual_spend_windows,
        cumulative_total,
    })
}

#[derive(Deserialize)]
pub struct BreakdownWindowParams {
    pub window: Option<String>,
    pub minutes: Option<i64>,
    pub limit_instances: Option<i64>,
}

#[derive(Serialize)]
pub struct CostsDashboardWindowResponse {
    pub window: String,
    pub window_minutes: i64,
    pub bucket_end_minute: Option<chrono::DateTime<chrono::Utc>>,
    pub bucket_start_minute: Option<chrono::DateTime<chrono::Utc>>,
    pub total_eur: f64,
    pub by_provider_eur: Vec<ProviderCostRow>,
    pub by_region_eur: Vec<RegionCostRow>,
    pub by_instance_type_eur: Vec<InstanceTypeCostRow>,
    pub by_instance_eur: Vec<InstanceCostRow>,
}

pub async fn get_costs_dashboard_window(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BreakdownWindowParams>,
) -> Json<CostsDashboardWindowResponse> {
    let db = &state.db;
    let limit_instances = default_limit_instances(params.limit_instances);

    let window_minutes = if let Some(w) = params.window.as_deref() {
        window_to_minutes(w).unwrap_or(60)
    } else {
        params.minutes.unwrap_or(60).clamp(1, 60 * 24 * 365)
    };
    let window_label = params
        .window
        .clone()
        .unwrap_or_else(|| format!("{}m", window_minutes));

    let bucket_end: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT MAX(bucket_minute) FROM finops.cost_actual_minute")
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .flatten();

    let Some(bucket_end) = bucket_end else {
        return Json(CostsDashboardWindowResponse {
            window: window_label,
            window_minutes,
            bucket_end_minute: None,
            bucket_start_minute: None,
            total_eur: 0.0,
            by_provider_eur: vec![],
            by_region_eur: vec![],
            by_instance_type_eur: vec![],
            by_instance_eur: vec![],
        });
    };

    let bucket_start = bucket_end - chrono::Duration::minutes((window_minutes - 1).max(0));

    let total_eur: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(amount_eur)::float8, 0)
        FROM finops.cost_actual_minute
        WHERE bucket_minute >= $1 AND bucket_minute <= $2
          AND provider_id IS NULL
          AND instance_id IS NULL
        "#,
    )
    .bind(bucket_start)
    .bind(bucket_end)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0.0);

    let by_provider_eur = sqlx::query_as::<Postgres, ProviderCostRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          p.name as provider_name,
          COALESCE(SUM(m.amount_eur), 0)::float8 as amount_eur
        FROM finops.cost_actual_minute m
        JOIN providers p ON p.id = m.provider_id
        WHERE m.bucket_minute >= $1 AND m.bucket_minute <= $2
          AND m.provider_id IS NOT NULL
          AND m.instance_id IS NULL
        GROUP BY p.id, p.code, p.name
        ORDER BY amount_eur DESC
        "#,
    )
    .bind(bucket_start)
    .bind(bucket_end)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let by_region_eur = sqlx::query_as::<Postgres, RegionCostRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          r.id as region_id,
          r.code as region_code,
          r.name as region_name,
          COALESCE(SUM(m.amount_eur), 0)::float8 as amount_eur
        FROM finops.cost_actual_minute m
        JOIN instances i ON i.id = m.instance_id
        LEFT JOIN zones z ON z.id = i.zone_id
        LEFT JOIN regions r ON r.id = z.region_id
        JOIN providers p ON p.id = i.provider_id
        WHERE m.bucket_minute >= $1 AND m.bucket_minute <= $2
          AND m.instance_id IS NOT NULL
        GROUP BY p.id, p.code, r.id, r.code, r.name
        ORDER BY amount_eur DESC
        "#,
    )
    .bind(bucket_start)
    .bind(bucket_end)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let by_instance_type_eur = sqlx::query_as::<Postgres, InstanceTypeCostRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.code as provider_code,
          it.id as instance_type_id,
          it.code as instance_type_code,
          it.name as instance_type_name,
          COALESCE(SUM(m.amount_eur), 0)::float8 as amount_eur
        FROM finops.cost_actual_minute m
        JOIN instances i ON i.id = m.instance_id
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        JOIN providers p ON p.id = i.provider_id
        WHERE m.bucket_minute >= $1 AND m.bucket_minute <= $2
          AND m.instance_id IS NOT NULL
        GROUP BY p.id, p.code, it.id, it.code, it.name
        ORDER BY amount_eur DESC
        "#,
    )
    .bind(bucket_start)
    .bind(bucket_end)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let by_instance_eur = sqlx::query_as::<Postgres, InstanceCostRow>(
        r#"
        SELECT
          i.id as instance_id,
          p.id as provider_id,
          p.code as provider_code,
          p.name as provider_name,
          r.name as region_name,
          z.name as zone_name,
          it.name as instance_type_name,
          COALESCE(SUM(m.amount_eur), 0)::float8 as amount_eur
        FROM finops.cost_actual_minute m
        JOIN instances i ON i.id = m.instance_id
        JOIN providers p ON p.id = i.provider_id
        LEFT JOIN zones z ON z.id = i.zone_id
        LEFT JOIN regions r ON r.id = z.region_id
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE m.bucket_minute >= $1 AND m.bucket_minute <= $2
          AND m.instance_id IS NOT NULL
        GROUP BY i.id, p.id, p.code, p.name, r.name, z.name, it.name
        ORDER BY amount_eur DESC
        LIMIT $3
        "#,
    )
    .bind(bucket_start)
    .bind(bucket_end)
    .bind(limit_instances)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    Json(CostsDashboardWindowResponse {
        window: window_label,
        window_minutes,
        bucket_end_minute: Some(bucket_end),
        bucket_start_minute: Some(bucket_start),
        total_eur,
        by_provider_eur,
        by_region_eur,
        by_instance_type_eur,
        by_instance_eur,
    })
}
