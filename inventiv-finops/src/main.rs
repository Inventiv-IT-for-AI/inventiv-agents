use anyhow::Context;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Timelike, Utc};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

use axum::{routing::get, Router};

#[derive(Clone)]
struct AppState {
    db: Pool<Postgres>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    // Note: sqlx::migrate! embeds migrations at compile-time. If you add new SQL files,
    // the binary must be recompiled (in Docker dev, cargo-watch will restart but Cargo
    // may not rebuild if only SQL files changed).
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let redis_url = std::env::var("REDIS_URL").context("REDIS_URL must be set")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to Postgres")?;

    // Run shared migrations (workspace root)
    sqlx::migrate!("../sqlx-migrations")
        .run(&pool)
        .await
        .context("Failed to run migrations")?;

    let state = Arc::new(AppState { db: pool });

    // Background FinOps calculator: runs every minute
    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = run_one_minute_tick(&state.db).await {
                    error!("finops minute tick failed: {:?}", e);
                }
                // sleep until next minute boundary
                sleep_to_next_minute().await;
            }
        });
    }

    // Redis consumer: react ASAP to create/terminate commands to refresh the CURRENT minute bucket
    // This does NOT replace the minute job; it just prevents waiting up to 60s for the next tick.
    {
        let state = state.clone();
        let redis_url = redis_url.clone();
        tokio::spawn(async move {
            if let Err(e) = run_cmd_consumer(&redis_url, &state.db).await {
                error!("finops cmd consumer stopped: {:?}", e);
            }
        });
    }

    // FinOps events consumer: consumes EVT:* published specifically for FinOps
    // Stores events in finops.events (append-only) + triggers immediate recompute for impacted buckets.
    {
        let state = state.clone();
        let redis_url = redis_url.clone();
        tokio::spawn(async move {
            if let Err(e) = run_finops_events_consumer(&redis_url, &state.db).await {
                error!("finops events consumer stopped: {:?}", e);
            }
        });
    }

    // Minimal HTTP health endpoint (helps ops)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new().route("/health", get(health)).layer(cors);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8005));
    info!("FinOps service listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn sleep_to_next_minute() {
    let now = Utc::now();
    let next = (now + Duration::minutes(1))
        .with_second(0)
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(now + Duration::seconds(60));
    let delta = (next - now)
        .to_std()
        .unwrap_or(std::time::Duration::from_secs(60));
    tokio::time::sleep(delta).await;
}

fn last_complete_minute(now: DateTime<Utc>) -> DateTime<Utc> {
    // we compute for the last full minute to avoid partial ingestion windows
    let floored = now
        .with_second(0)
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(now);
    floored - Duration::minutes(1)
}

fn current_minute_bucket(now: DateTime<Utc>) -> DateTime<Utc> {
    now.with_second(0)
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(now)
}

async fn run_one_minute_tick(db: &Pool<Postgres>) -> anyhow::Result<()> {
    let now = Utc::now();
    let bucket = last_complete_minute(now);
    let bucket_end = bucket + Duration::minutes(1);

    // 1) Forecast/burn-rate: based on active instances allocation
    compute_and_store_forecast(db, bucket).await?;

    // 2) Actual minute costs: aggregate provider_costs into per-minute buckets
    //    This is safe even if provider_costs is empty.
    compute_and_store_actual_minute(db, bucket, bucket_end).await?;

    // 3) Cumulative actual minute: running sum based on previous cumulative + current minute
    compute_and_store_actual_cumulative(db, bucket).await?;

    Ok(())
}

async fn run_cmd_consumer(redis_url: &str, db: &Pool<Postgres>) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe("orchestrator_events").await?;
    info!("FinOps listening on Redis channel 'orchestrator_events'...");

    use futures_util::StreamExt;
    let mut stream = pubsub.on_message();

    while let Some(msg) = stream.next().await {
        let payload: String = msg.get_payload()?;

        // Best effort parse; ignore unknown events
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&payload) {
            let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
            // For FinOps: we care mostly about provisioning/terminate to refresh allocation burn-rate.
            if event_type == "CMD:PROVISION" || event_type == "CMD:TERMINATE" {
                let bucket = current_minute_bucket(Utc::now());
                if let Err(e) = compute_and_store_forecast(db, bucket).await {
                    error!("forecast refresh on {} failed: {:?}", event_type, e);
                }
            }
        }
    }

    Ok(())
}

use inventiv_common::bus::{FinopsEventEnvelope, FinopsEventType};

async fn run_finops_events_consumer(redis_url: &str, db: &Pool<Postgres>) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe("finops_events").await?;
    info!("FinOps listening on Redis channel 'finops_events'...");

    use futures_util::StreamExt;
    let mut stream = pubsub.on_message();

    while let Some(msg) = stream.next().await {
        let payload: String = msg.get_payload()?;
        let Ok(evt) = serde_json::from_str::<FinopsEventEnvelope>(&payload) else {
            continue;
        };

        // 1) Persist raw event (idempotent)
        let _ = sqlx::query(
            r#"
            INSERT INTO finops.events (event_id, occurred_at, event_type, source, payload)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (event_id) DO NOTHING
            "#,
        )
        .bind(evt.event_id)
        .bind(evt.occurred_at)
        .bind(evt.event_type.as_str())
        .bind(&evt.source)
        .bind(&evt.payload)
        .execute(db)
        .await;

        // 2) React fast for cost start/stop to update the current bucket forecast
        if evt.event_type == FinopsEventType::InstanceCostStart
            || evt.event_type == FinopsEventType::InstanceCostStop
        {
            let bucket = current_minute_bucket(evt.occurred_at);
            let _ = compute_and_store_forecast(db, bucket).await;
        }
    }

    Ok(())
}

async fn compute_and_store_forecast(
    db: &Pool<Postgres>,
    bucket: DateTime<Utc>,
) -> anyhow::Result<()> {
    // Active statuses: anything not terminal/failure/archived.
    // We treat terminating as still allocated (still costing) until terminated_at is set.
    // We only count allocated resources (provider_instance_id present).
    //
    // per provider
    let rows: Vec<(Option<uuid::Uuid>, BigDecimal)> = sqlx::query_as(
        r#"
        SELECT
          i.provider_id,
          COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) AS burn_rate_per_hour
        FROM instances i
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE i.is_archived = false
          AND i.provider_instance_id IS NOT NULL
          AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
          AND i.created_at <= $1
          AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        GROUP BY i.provider_id
        "#
    )
    .bind(bucket)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    // total (provider_id = NULL) computed in SQL for consistency
    let total: (BigDecimal,) = sqlx::query_as(
        r#"
        SELECT
          COALESCE(SUM(COALESCE(it.cost_per_hour, 0)), 0) AS burn_rate_per_hour
        FROM instances i
        LEFT JOIN instance_types it ON it.id = i.instance_type_id
        WHERE i.is_archived = false
          AND i.provider_instance_id IS NOT NULL
          AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
          AND i.created_at <= $1
          AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        "#
    )
    .bind(bucket)
    .fetch_one(db)
    .await
    .unwrap_or((BigDecimal::from(0),));

    // store per provider (skip NULL provider_id rows defensively)
    for (provider_id, burn_rate_per_hour) in rows.into_iter() {
        if let Some(pid) = provider_id {
            upsert_forecast_row(db, bucket, Some(pid), burn_rate_per_hour).await?;
        }
    }

    // store total
    upsert_forecast_row(db, bucket, None, total.0).await?;

    Ok(())
}

async fn upsert_forecast_row(
    db: &Pool<Postgres>,
    bucket: DateTime<Utc>,
    provider_id: Option<uuid::Uuid>,
    burn_rate_per_hour: BigDecimal,
) -> anyhow::Result<()> {
    // projections
    let sixty = BigDecimal::from(60);
    let twenty_four = BigDecimal::from(24);
    let thirty = BigDecimal::from(30);
    let three_sixty_five = BigDecimal::from(365);

    let per_minute = &burn_rate_per_hour / &sixty;
    let per_hour = burn_rate_per_hour.clone();
    let per_day = &burn_rate_per_hour * &twenty_four;
    let per_month_30 = &per_day * &thirty;
    let per_year_365 = &per_day * &three_sixty_five;

    sqlx::query(
        r#"
        INSERT INTO finops.cost_forecast_minute (
          bucket_minute, provider_id,
          burn_rate_eur_per_hour,
          forecast_eur_per_minute, forecast_eur_per_hour, forecast_eur_per_day, forecast_eur_per_month_30d, forecast_eur_per_year_365d
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (bucket_minute, provider_id_key)
        DO UPDATE SET
          burn_rate_eur_per_hour = EXCLUDED.burn_rate_eur_per_hour,
          forecast_eur_per_minute = EXCLUDED.forecast_eur_per_minute,
          forecast_eur_per_hour = EXCLUDED.forecast_eur_per_hour,
          forecast_eur_per_day = EXCLUDED.forecast_eur_per_day,
          forecast_eur_per_month_30d = EXCLUDED.forecast_eur_per_month_30d,
          forecast_eur_per_year_365d = EXCLUDED.forecast_eur_per_year_365d
        "#
    )
    .bind(bucket)
    .bind(provider_id)
    .bind(burn_rate_per_hour)
    .bind(per_minute)
    .bind(per_hour)
    .bind(per_day)
    .bind(per_month_30)
    .bind(per_year_365)
    .execute(db)
    .await?;

    Ok(())
}

async fn compute_and_store_actual_minute(
    db: &Pool<Postgres>,
    bucket: DateTime<Utc>,
    bucket_end: DateTime<Utc>,
) -> anyhow::Result<()> {
    // For dashboard now: compute "actual" from allocated instances and provider catalog pricing.
    // This is a precise, prorated allocation cost (overlap seconds within the minute) using instance_types.cost_per_hour.
    //
    // Later we can add a separate pipeline to ingest provider billing lines into finops.provider_costs.
    //
    // We store 3 levels:
    // - total: provider_id NULL, instance_id NULL
    // - provider: provider_id set, instance_id NULL
    // - instance: provider_id set, instance_id set

    // total
    let total: (BigDecimal,) = sqlx::query_as(
        r#"
        WITH active AS (
          SELECT
            COALESCE(it.cost_per_hour, 0) AS cost_per_hour,
            GREATEST(i.created_at, $1) AS start_ts,
            LEAST(COALESCE(i.terminated_at, $2), $2) AS end_ts
          FROM instances i
          LEFT JOIN instance_types it ON it.id = i.instance_type_id
          WHERE i.is_archived = false
            AND i.provider_instance_id IS NOT NULL
            AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
            AND i.created_at < $2
            AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        )
        SELECT COALESCE(SUM((EXTRACT(EPOCH FROM (end_ts - start_ts)) / 3600.0) * cost_per_hour), 0) AS amount
        FROM active
        WHERE end_ts > start_ts
        "#
    )
    .bind(bucket)
    .bind(bucket_end)
    .fetch_one(db)
    .await
    .unwrap_or((BigDecimal::from(0),));

    upsert_actual_minute_row(db, bucket, None, None, total.0).await?;

    // provider
    let provider_rows: Vec<(uuid::Uuid, BigDecimal)> = sqlx::query_as(
        r#"
        WITH active AS (
          SELECT
            i.provider_id,
            COALESCE(it.cost_per_hour, 0) AS cost_per_hour,
            GREATEST(i.created_at, $1) AS start_ts,
            LEAST(COALESCE(i.terminated_at, $2), $2) AS end_ts
          FROM instances i
          LEFT JOIN instance_types it ON it.id = i.instance_type_id
          WHERE i.is_archived = false
            AND i.provider_instance_id IS NOT NULL
            AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
            AND i.created_at < $2
            AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        )
        SELECT provider_id,
               COALESCE(SUM((EXTRACT(EPOCH FROM (end_ts - start_ts)) / 3600.0) * cost_per_hour), 0) AS amount
        FROM active
        WHERE end_ts > start_ts
        GROUP BY provider_id
        "#
    )
    .bind(bucket)
    .bind(bucket_end)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (provider_id, amount) in provider_rows {
        upsert_actual_minute_row(db, bucket, Some(provider_id), None, amount).await?;
    }

    // instance
    let instance_rows: Vec<(uuid::Uuid, uuid::Uuid, BigDecimal)> = sqlx::query_as(
        r#"
        WITH active AS (
          SELECT
            i.provider_id,
            i.id AS instance_id,
            COALESCE(it.cost_per_hour, 0) AS cost_per_hour,
            GREATEST(i.created_at, $1) AS start_ts,
            LEAST(COALESCE(i.terminated_at, $2), $2) AS end_ts
          FROM instances i
          LEFT JOIN instance_types it ON it.id = i.instance_type_id
          WHERE i.is_archived = false
            AND i.provider_instance_id IS NOT NULL
            AND (i.status::text NOT IN ('terminated','failed','provisioning_failed','startup_failed','archived'))
            AND i.created_at < $2
            AND (i.terminated_at IS NULL OR i.terminated_at > $1)
        )
        SELECT provider_id,
               instance_id,
               COALESCE(SUM((EXTRACT(EPOCH FROM (end_ts - start_ts)) / 3600.0) * cost_per_hour), 0) AS amount
        FROM active
        WHERE end_ts > start_ts
        GROUP BY provider_id, instance_id
        "#
    )
    .bind(bucket)
    .bind(bucket_end)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (provider_id, instance_id, amount) in instance_rows {
        upsert_actual_minute_row(db, bucket, Some(provider_id), Some(instance_id), amount).await?;
    }

    Ok(())
}

async fn upsert_actual_minute_row(
    db: &Pool<Postgres>,
    bucket: DateTime<Utc>,
    provider_id: Option<uuid::Uuid>,
    instance_id: Option<uuid::Uuid>,
    amount: BigDecimal,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO finops.cost_actual_minute (bucket_minute, provider_id, instance_id, amount_eur)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (bucket_minute, provider_id_key, instance_id_key)
        DO UPDATE SET amount_eur = EXCLUDED.amount_eur
        "#,
    )
    .bind(bucket)
    .bind(provider_id)
    .bind(instance_id)
    .bind(amount)
    .execute(db)
    .await?;

    Ok(())
}

async fn compute_and_store_actual_cumulative(
    db: &Pool<Postgres>,
    bucket: DateTime<Utc>,
) -> anyhow::Result<()> {
    // We build cumulative for three levels by:
    // cumulative(bucket) = cumulative(bucket-1min) + actual_minute(bucket)
    let prev_bucket = bucket - Duration::minutes(1);

    // Total cumulative
    let prev_total: BigDecimal = sqlx::query_scalar(
        r#"
        SELECT cumulative_amount_eur
        FROM finops.cost_actual_cumulative_minute
        WHERE bucket_minute = $1 AND provider_id IS NULL AND instance_id IS NULL
        "#,
    )
    .bind(prev_bucket)
    .fetch_optional(db)
    .await?
    .unwrap_or(BigDecimal::from(0));

    let curr_total: BigDecimal = sqlx::query_scalar(
        r#"
        SELECT amount_eur
        FROM finops.cost_actual_minute
        WHERE bucket_minute = $1 AND provider_id IS NULL AND instance_id IS NULL
        "#,
    )
    .bind(bucket)
    .fetch_optional(db)
    .await?
    .unwrap_or(BigDecimal::from(0));

    upsert_cumulative_row(db, bucket, None, None, prev_total + curr_total).await?;

    // Provider cumulative
    let provider_curr: Vec<(Option<uuid::Uuid>, BigDecimal)> = sqlx::query_as(
        r#"
        SELECT provider_id, amount_eur
        FROM finops.cost_actual_minute
        WHERE bucket_minute = $1 AND instance_id IS NULL
          AND provider_id IS NOT NULL
        "#,
    )
    .bind(bucket)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (provider_id, amount) in provider_curr {
        let prev: BigDecimal = sqlx::query_scalar(
            r#"
            SELECT cumulative_amount_eur
            FROM finops.cost_actual_cumulative_minute
            WHERE bucket_minute = $1 AND provider_id = $2 AND instance_id IS NULL
            "#,
        )
        .bind(prev_bucket)
        .bind(provider_id)
        .fetch_optional(db)
        .await?
        .unwrap_or(BigDecimal::from(0));

        upsert_cumulative_row(db, bucket, provider_id, None, prev + amount).await?;
    }

    // Instance cumulative
    let instance_curr: Vec<(Option<uuid::Uuid>, Option<uuid::Uuid>, BigDecimal)> = sqlx::query_as(
        r#"
        SELECT provider_id, instance_id, amount_eur
        FROM finops.cost_actual_minute
        WHERE bucket_minute = $1 AND instance_id IS NOT NULL
        "#,
    )
    .bind(bucket)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (provider_id, instance_id, amount) in instance_curr {
        let prev: BigDecimal = sqlx::query_scalar(
            r#"
            SELECT cumulative_amount_eur
            FROM finops.cost_actual_cumulative_minute
            WHERE bucket_minute = $1 AND provider_id = $2 AND instance_id = $3
            "#,
        )
        .bind(prev_bucket)
        .bind(provider_id)
        .bind(instance_id)
        .fetch_optional(db)
        .await?
        .unwrap_or(BigDecimal::from(0));

        upsert_cumulative_row(db, bucket, provider_id, instance_id, prev + amount).await?;
    }

    Ok(())
}

async fn upsert_cumulative_row(
    db: &Pool<Postgres>,
    bucket: DateTime<Utc>,
    provider_id: Option<uuid::Uuid>,
    instance_id: Option<uuid::Uuid>,
    cumulative: BigDecimal,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO finops.cost_actual_cumulative_minute (
          bucket_minute, provider_id, instance_id, cumulative_amount_eur
        )
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (bucket_minute, provider_id_key, instance_id_key)
        DO UPDATE SET cumulative_amount_eur = EXCLUDED.cumulative_amount_eur
        "#,
    )
    .bind(bucket)
    .bind(provider_id)
    .bind(instance_id)
    .bind(cumulative)
    .execute(db)
    .await?;

    Ok(())
}
