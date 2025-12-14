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
    pub burn_rate_usd_per_hour: f64,
    pub forecast_usd_per_minute: f64,
    pub forecast_usd_per_day: f64,
    pub forecast_usd_per_month_30d: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ActualMinuteRow {
    pub bucket_minute: chrono::DateTime<chrono::Utc>,
    pub provider_id: Option<uuid::Uuid>,
    pub instance_id: Option<uuid::Uuid>,
    pub amount_usd: f64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CumulativeMinuteRow {
    pub bucket_minute: chrono::DateTime<chrono::Utc>,
    pub provider_id: Option<uuid::Uuid>,
    pub instance_id: Option<uuid::Uuid>,
    pub cumulative_amount_usd: f64,
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
              burn_rate_usd_per_hour::float8 as burn_rate_usd_per_hour,
              forecast_usd_per_minute::float8 as forecast_usd_per_minute,
              forecast_usd_per_day::float8 as forecast_usd_per_day,
              forecast_usd_per_month_30d::float8 as forecast_usd_per_month_30d
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
          cumulative_amount_usd::float8 as cumulative_amount_usd
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
          burn_rate_usd_per_hour::float8 as burn_rate_usd_per_hour,
          forecast_usd_per_minute::float8 as forecast_usd_per_minute,
          forecast_usd_per_day::float8 as forecast_usd_per_day,
          forecast_usd_per_month_30d::float8 as forecast_usd_per_month_30d
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
          amount_usd::float8 as amount_usd
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
          cumulative_amount_usd::float8 as cumulative_amount_usd
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

