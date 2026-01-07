// Monitoring handlers (runtime models, GPU activity, system activity)
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use std::sync::Arc;
use utoipa::IntoParams;

use crate::app::AppState;
use crate::handlers::openai::openai_worker_stale_seconds_db;

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct RuntimeModelRow {
    model_id: String,
    first_seen_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: chrono::DateTime<chrono::Utc>,
    instances_available: i64,
    gpus_available: i64,
    vram_total_gb: i64,
    total_requests: i64,
    failed_requests: i64,
}

#[utoipa::path(
    get,
    path = "/runtime/models",
    responses((status = 200, description = "Runtime models (live capacity + history + counters)", body = Vec<RuntimeModelRow>))
)]
pub async fn list_runtime_models(State(state): State<Arc<AppState>>) -> Json<Vec<RuntimeModelRow>> {
    let stale = openai_worker_stale_seconds_db(&state.db).await;

    // Live capacity aggregation (only "ready" + recent heartbeats).
    // Note: instance_types may be null in some edge cases; we treat missing as 0.
    let rows = sqlx::query_as::<Postgres, RuntimeModelRow>(
        r#"
        WITH live AS (
          SELECT
            i.worker_model_id AS model_id,
            COUNT(*)::bigint AS instances_available,
            COALESCE(SUM(COALESCE(it.gpu_count, 0))::bigint, 0) AS gpus_available,
            COALESCE(SUM(COALESCE(it.gpu_count, 0) * COALESCE(it.vram_per_gpu_gb, 0))::bigint, 0) AS vram_total_gb
          FROM instances i
          LEFT JOIN instance_types it ON it.id = i.instance_type_id
          WHERE i.status::text = 'ready'
            AND i.ip_address IS NOT NULL
            AND i.worker_model_id IS NOT NULL
            AND (i.worker_status = 'ready' OR i.worker_status IS NULL)
            AND GREATEST(
              COALESCE(i.worker_last_heartbeat, 'epoch'::timestamptz),
              COALESCE(i.last_health_check, 'epoch'::timestamptz),
              COALESCE((i.last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
            ) > NOW() - ($1::bigint * INTERVAL '1 second')
          GROUP BY i.worker_model_id
        )
        SELECT
          rm.model_id,
          rm.first_seen_at,
          rm.last_seen_at,
          COALESCE(l.instances_available, 0) AS instances_available,
          COALESCE(l.gpus_available, 0) AS gpus_available,
          COALESCE(l.vram_total_gb, 0) AS vram_total_gb,
          rm.total_requests,
          rm.failed_requests
        FROM runtime_models rm
        LEFT JOIN live l ON l.model_id = rm.model_id
        ORDER BY COALESCE(l.instances_available, 0) DESC, rm.last_seen_at DESC
        "#,
    )
    .bind(stale)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}

#[derive(Deserialize, IntoParams)]
pub struct GpuActivityParams {
    /// How far back to query (seconds). Default 300.
    window_s: Option<i64>,
    /// Optional filter (single instance).
    instance_id: Option<uuid::Uuid>,
    /// "second" | "minute" | "hour" | "day"
    granularity: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct GpuSampleRow {
    time: chrono::DateTime<chrono::Utc>,
    instance_id: uuid::Uuid,
    gpu_index: i32,
    gpu_utilization: Option<f64>,
    vram_used_mb: Option<f64>,
    vram_total_mb: Option<f64>,
    temp_c: Option<f64>,
    power_w: Option<f64>,
    power_limit_w: Option<f64>,
    instance_name: Option<String>,
    provider_name: Option<String>,
    gpu_count: Option<i32>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct GpuActivitySample {
    ts: String,
    gpu_pct: Option<f64>,
    vram_pct: Option<f64>,
    temp_c: Option<f64>,
    power_w: Option<f64>,
    power_limit_w: Option<f64>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct GpuActivityGpuSeries {
    gpu_index: i32,
    samples: Vec<GpuActivitySample>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct GpuActivityInstanceSeries {
    instance_id: uuid::Uuid,
    instance_name: Option<String>,
    provider_name: Option<String>,
    gpu_count: Option<i32>,
    gpus: Vec<GpuActivityGpuSeries>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct GpuActivityResponse {
    window_s: i64,
    generated_at: String,
    instances: Vec<GpuActivityInstanceSeries>,
}

#[utoipa::path(
    get,
    path = "/gpu/activity",
    params(GpuActivityParams),
    responses((status = 200, description = "GPU activity (per-GPU time series)", body = GpuActivityResponse))
)]
pub async fn list_gpu_activity(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GpuActivityParams>,
) -> impl IntoResponse {
    let window_s = params.window_s.unwrap_or(300).clamp(30, 3600);
    let gran = params
        .granularity
        .as_deref()
        .unwrap_or("second")
        .trim()
        .to_ascii_lowercase();
    let instance_filter = params.instance_id;

    // Note: keep this handler resilient; it is frequently queried by the UI.

    let rows: Vec<GpuSampleRow> = match gran.as_str() {
        "minute" => {
            let res = sqlx::query_as::<Postgres, GpuSampleRow>(
                r#"
                SELECT
                  gs.bucket as time,
                  gs.instance_id,
                  gs.gpu_index,
                  gs.gpu_utilization,
                  gs.vram_used_mb,
                  gs.vram_total_mb,
                  gs.temp_c,
                  gs.power_w,
                  gs.power_limit_w,
                  i.provider_instance_id::text as instance_name,
                  p.name as provider_name,
                  it.gpu_count as gpu_count
                FROM gpu_samples_1m gs
                JOIN instances i ON i.id = gs.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                LEFT JOIN instance_types it ON it.id = i.instance_type_id
                WHERE gs.bucket > NOW() - ($1::bigint * INTERVAL '1 second')
                  AND ($2::uuid IS NULL OR gs.instance_id = $2)
                ORDER BY gs.instance_id, gs.gpu_index, gs.bucket ASC
                "#,
            )
            .bind(window_s)
            .bind(instance_filter)
            .fetch_all(&state.db)
            .await;
            match res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ gpu/activity query (minute) failed: {}", e);
                    vec![]
                }
            }
        }
        "hour" => {
            let res = sqlx::query_as::<Postgres, GpuSampleRow>(
                r#"
                SELECT
                  gs.bucket as time,
                  gs.instance_id,
                  gs.gpu_index,
                  gs.gpu_utilization,
                  gs.vram_used_mb,
                  gs.vram_total_mb,
                  gs.temp_c,
                  gs.power_w,
                  gs.power_limit_w,
                  i.provider_instance_id::text as instance_name,
                  p.name as provider_name,
                  it.gpu_count as gpu_count
                FROM gpu_samples_1h gs
                JOIN instances i ON i.id = gs.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                LEFT JOIN instance_types it ON it.id = i.instance_type_id
                WHERE gs.bucket > NOW() - ($1::bigint * INTERVAL '1 second')
                  AND ($2::uuid IS NULL OR gs.instance_id = $2)
                ORDER BY gs.instance_id, gs.gpu_index, gs.bucket ASC
                "#,
            )
            .bind(window_s)
            .bind(instance_filter)
            .fetch_all(&state.db)
            .await;
            match res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ gpu/activity query (hour) failed: {}", e);
                    vec![]
                }
            }
        }
        "day" => {
            let res = sqlx::query_as::<Postgres, GpuSampleRow>(
                r#"
                SELECT
                  gs.bucket as time,
                  gs.instance_id,
                  gs.gpu_index,
                  gs.gpu_utilization,
                  gs.vram_used_mb,
                  gs.vram_total_mb,
                  gs.temp_c,
                  gs.power_w,
                  gs.power_limit_w,
                  i.provider_instance_id::text as instance_name,
                  p.name as provider_name,
                  it.gpu_count as gpu_count
                FROM gpu_samples_1d gs
                JOIN instances i ON i.id = gs.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                LEFT JOIN instance_types it ON it.id = i.instance_type_id
                WHERE gs.bucket > NOW() - ($1::bigint * INTERVAL '1 second')
                  AND ($2::uuid IS NULL OR gs.instance_id = $2)
                ORDER BY gs.instance_id, gs.gpu_index, gs.bucket ASC
                "#,
            )
            .bind(window_s)
            .bind(instance_filter)
            .fetch_all(&state.db)
            .await;
            match res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ gpu/activity query (day) failed: {}", e);
                    vec![]
                }
            }
        }
        // second (default): raw table (still can be sparse depending on heartbeat interval)
        _ => {
            let res = sqlx::query_as::<Postgres, GpuSampleRow>(
                r#"
                SELECT
                  gs.time,
                  gs.instance_id,
                  gs.gpu_index,
                  gs.gpu_utilization,
                  gs.vram_used_mb,
                  gs.vram_total_mb,
                  gs.temp_c,
                  gs.power_w,
                  gs.power_limit_w,
                  i.provider_instance_id::text as instance_name,
                  p.name as provider_name,
                  it.gpu_count as gpu_count
                FROM gpu_samples gs
                JOIN instances i ON i.id = gs.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                LEFT JOIN instance_types it ON it.id = i.instance_type_id
                WHERE gs.time > NOW() - ($1::bigint * INTERVAL '1 second')
                  AND ($2::uuid IS NULL OR gs.instance_id = $2)
                ORDER BY gs.instance_id, gs.gpu_index, gs.time ASC
                "#,
            )
            .bind(window_s)
            .bind(instance_filter)
            .fetch_all(&state.db)
            .await;
            match res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ gpu/activity query (second) failed: {}", e);
                    vec![]
                }
            }
        }
    };

    use std::collections::BTreeMap;
    let mut map: BTreeMap<
        uuid::Uuid,
        (
            Option<String>,
            Option<String>,
            Option<i32>,
            BTreeMap<i32, Vec<GpuActivitySample>>,
        ),
    > = BTreeMap::new();

    for r in rows {
        let vram_pct = match (r.vram_used_mb, r.vram_total_mb) {
            (Some(u), Some(t)) if t > 0.0 => Some((u / t) * 100.0),
            _ => None,
        };
        let sample = GpuActivitySample {
            ts: r.time.to_rfc3339(),
            gpu_pct: r.gpu_utilization,
            vram_pct,
            temp_c: r.temp_c,
            power_w: r.power_w,
            power_limit_w: r.power_limit_w,
        };
        let entry = map.entry(r.instance_id).or_insert((
            r.instance_name.clone(),
            r.provider_name.clone(),
            r.gpu_count,
            BTreeMap::new(),
        ));
        entry.3.entry(r.gpu_index).or_default().push(sample);
    }

    let instances = map
        .into_iter()
        .map(
            |(instance_id, (instance_name, provider_name, gpu_count, gmap))| {
                let gpus = gmap
                    .into_iter()
                    .map(|(gpu_index, samples)| GpuActivityGpuSeries { gpu_index, samples })
                    .collect();
                GpuActivityInstanceSeries {
                    instance_id,
                    instance_name,
                    provider_name,
                    gpu_count,
                    gpus,
                }
            },
        )
        .collect();

    (
        StatusCode::OK,
        Json(GpuActivityResponse {
            window_s,
            generated_at: chrono::Utc::now().to_rfc3339(),
            instances,
        }),
    )
        .into_response()
}

#[derive(Deserialize, IntoParams)]
pub struct SystemActivityParams {
    /// How far back to query (seconds). Default 300.
    window_s: Option<i64>,
    /// Optional filter (single instance).
    instance_id: Option<uuid::Uuid>,
    /// "second" | "minute" | "hour" | "day"
    granularity: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct SystemSampleRow {
    time: chrono::DateTime<chrono::Utc>,
    instance_id: uuid::Uuid,
    cpu_usage_pct: Option<f64>,
    load1: Option<f64>,
    mem_used_bytes: Option<i64>,
    mem_total_bytes: Option<i64>,
    disk_used_bytes: Option<i64>,
    disk_total_bytes: Option<i64>,
    net_rx_bps: Option<f64>,
    net_tx_bps: Option<f64>,
    instance_name: Option<String>,
    provider_name: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct SystemActivitySample {
    ts: String,
    cpu_pct: Option<f64>,
    load1: Option<f64>,
    mem_pct: Option<f64>,
    disk_pct: Option<f64>,
    net_rx_mbps: Option<f64>,
    net_tx_mbps: Option<f64>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct SystemActivityInstanceSeries {
    instance_id: uuid::Uuid,
    instance_name: Option<String>,
    provider_name: Option<String>,
    samples: Vec<SystemActivitySample>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SystemActivityResponse {
    window_s: i64,
    generated_at: String,
    instances: Vec<SystemActivityInstanceSeries>,
}

#[utoipa::path(
    get,
    path = "/system/activity",
    params(SystemActivityParams),
    responses((status = 200, description = "System activity (CPU/Mem/Disk/Network time series)", body = SystemActivityResponse))
)]
pub async fn list_system_activity(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SystemActivityParams>,
) -> impl IntoResponse {
    let window_s = params.window_s.unwrap_or(300).clamp(30, 3600);
    let gran = params
        .granularity
        .as_deref()
        .unwrap_or("second")
        .trim()
        .to_ascii_lowercase();
    let instance_filter = params.instance_id;

    let rows: Vec<SystemSampleRow> = match gran.as_str() {
        "minute" => {
            let res = sqlx::query_as::<Postgres, SystemSampleRow>(
                r#"
                SELECT
                  ss.bucket as time,
                  ss.instance_id,
                  ss.cpu_usage_pct,
                  ss.load1,
                  ss.mem_used_bytes,
                  ss.mem_total_bytes,
                  ss.disk_used_bytes,
                  ss.disk_total_bytes,
                  ss.net_rx_bps,
                  ss.net_tx_bps,
                  i.provider_instance_id::text as instance_name,
                  p.name as provider_name
                FROM system_samples_1m ss
                JOIN instances i ON i.id = ss.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                WHERE ss.bucket > NOW() - ($1::bigint * INTERVAL '1 second')
                  AND ($2::uuid IS NULL OR ss.instance_id = $2)
                ORDER BY ss.instance_id, ss.bucket ASC
                "#,
            )
            .bind(window_s)
            .bind(instance_filter)
            .fetch_all(&state.db)
            .await;
            match res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ system/activity query (minute) failed: {}", e);
                    vec![]
                }
            }
        }
        "hour" => {
            let res = sqlx::query_as::<Postgres, SystemSampleRow>(
                r#"
                SELECT
                  ss.bucket as time,
                  ss.instance_id,
                  ss.cpu_usage_pct,
                  ss.load1,
                  ss.mem_used_bytes,
                  ss.mem_total_bytes,
                  ss.disk_used_bytes,
                  ss.disk_total_bytes,
                  ss.net_rx_bps,
                  ss.net_tx_bps,
                  i.provider_instance_id::text as instance_name,
                  p.name as provider_name
                FROM system_samples_1h ss
                JOIN instances i ON i.id = ss.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                WHERE ss.bucket > NOW() - ($1::bigint * INTERVAL '1 second')
                  AND ($2::uuid IS NULL OR ss.instance_id = $2)
                ORDER BY ss.instance_id, ss.bucket ASC
                "#,
            )
            .bind(window_s)
            .bind(instance_filter)
            .fetch_all(&state.db)
            .await;
            match res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ system/activity query (hour) failed: {}", e);
                    vec![]
                }
            }
        }
        "day" => {
            let res = sqlx::query_as::<Postgres, SystemSampleRow>(
                r#"
                SELECT
                  ss.bucket as time,
                  ss.instance_id,
                  ss.cpu_usage_pct,
                  ss.load1,
                  ss.mem_used_bytes,
                  ss.mem_total_bytes,
                  ss.disk_used_bytes,
                  ss.disk_total_bytes,
                  ss.net_rx_bps,
                  ss.net_tx_bps,
                  i.provider_instance_id::text as instance_name,
                  p.name as provider_name
                FROM system_samples_1d ss
                JOIN instances i ON i.id = ss.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                WHERE ss.bucket > NOW() - ($1::bigint * INTERVAL '1 second')
                  AND ($2::uuid IS NULL OR ss.instance_id = $2)
                ORDER BY ss.instance_id, ss.bucket ASC
                "#,
            )
            .bind(window_s)
            .bind(instance_filter)
            .fetch_all(&state.db)
            .await;
            match res {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ system/activity query (day) failed: {}", e);
                    vec![]
                }
            }
        }
        // second (default): raw samples
        _ => sqlx::query_as::<Postgres, SystemSampleRow>(
            r#"
            SELECT
              s.time,
              s.instance_id,
              s.cpu_usage_pct,
              s.load1,
              s.mem_used_bytes,
              s.mem_total_bytes,
              s.disk_used_bytes,
              s.disk_total_bytes,
              s.net_rx_bps,
              s.net_tx_bps,
              i.provider_instance_id::text as instance_name,
              p.name as provider_name
            FROM system_samples s
            JOIN instances i ON i.id = s.instance_id
            LEFT JOIN providers p ON p.id = i.provider_id
            WHERE s.time > NOW() - make_interval(secs => $1)
              AND ($2::uuid IS NULL OR s.instance_id = $2)
            ORDER BY s.instance_id, s.time ASC
            "#,
        )
        .bind(window_s)
        .bind(instance_filter)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default(),
    };

    use std::collections::BTreeMap;
    let mut map: BTreeMap<uuid::Uuid, (Option<String>, Option<String>, Vec<SystemActivitySample>)> =
        BTreeMap::new();

    for r in rows {
        let mem_pct = match (r.mem_used_bytes, r.mem_total_bytes) {
            (Some(u), Some(t)) if t > 0 => Some((u as f64 / t as f64) * 100.0),
            _ => None,
        };
        let disk_pct = match (r.disk_used_bytes, r.disk_total_bytes) {
            (Some(u), Some(t)) if t > 0 => Some((u as f64 / t as f64) * 100.0),
            _ => None,
        };
        let net_rx_mbps = r.net_rx_bps.map(|bps| (bps * 8.0) / 1_000_000.0);
        let net_tx_mbps = r.net_tx_bps.map(|bps| (bps * 8.0) / 1_000_000.0);
        let sample = SystemActivitySample {
            ts: r.time.to_rfc3339(),
            cpu_pct: r.cpu_usage_pct,
            load1: r.load1,
            mem_pct,
            disk_pct,
            net_rx_mbps,
            net_tx_mbps,
        };
        let entry = map.entry(r.instance_id).or_insert((
            r.instance_name.clone(),
            r.provider_name.clone(),
            Vec::new(),
        ));
        entry.2.push(sample);
    }

    let instances = map
        .into_iter()
        .map(|(instance_id, (instance_name, provider_name, samples))| {
            SystemActivityInstanceSeries {
                instance_id,
                instance_name,
                provider_name,
                samples,
            }
        })
        .collect();

    (
        StatusCode::OK,
        Json(SystemActivityResponse {
            window_s,
            generated_at: chrono::Utc::now().to_rfc3339(),
            instances,
        }),
    )
        .into_response()
}
