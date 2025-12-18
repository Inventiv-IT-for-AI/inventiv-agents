use axum::body::Body;
use axum::body::Bytes;
use axum::middleware;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::Response;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use inventiv_common::LlmModel;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tower_http::cors::{Any, CorsLayer};

// Swagger
use utoipa::{IntoParams, OpenApi};
use utoipa_swagger_ui::SwaggerUi;
mod action_logs_search;
mod api_docs;
mod api_keys;
mod auth;
mod auth_endpoints;
mod bootstrap_admin;
mod locales_endpoint;
mod user_locale;
mod finops;
mod instance_type_zones; // Module for zone associations
mod provider_settings;
mod settings; // Module
mod simple_logger;
mod users_endpoint;
 // Simple logger without sqlx macros

// use audit_log::AuditLogger; // Commented out due to DATABASE_URL build issues

#[derive(Clone)]
struct AppState {
    redis_client: redis::Client,
    db: Pool<Postgres>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let client = redis::Client::open(redis_url).unwrap();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    // Run migrations (source of truth is /migrations at workspace root)
    // Safe to run on startup; sqlx uses the _sqlx_migrations table + lock.
    // Note: migrations are embedded at compile-time; code change forces rebuild.
    sqlx::migrate!("../sqlx-migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Optional dev convenience: auto-seed catalog when DB is empty.
    // Guarded by env var to avoid accidental seeding in staging/prod.
    maybe_seed_catalog(&pool).await;
    // Ensure default admin exists (dev/staging/prod)
    bootstrap_admin::ensure_default_admin(&pool).await;

    let state = Arc::new(AppState {
        redis_client: client,
        db: pool,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no user auth)
    let public = Router::new()
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", api_docs::ApiDoc::openapi()),
        )
        .route("/", get(root))
        .route("/auth/login", post(auth_endpoints::login))
        .route("/auth/logout", post(auth_endpoints::logout));

    // Worker routes (worker auth handled in handler + orchestrator)
    let worker = Router::new()
        .route("/internal/worker/register", post(proxy_worker_register))
        .route("/internal/worker/heartbeat", post(proxy_worker_heartbeat));

    // OpenAI-compatible proxy routes (auth = cookie/JWT OR API key)
    let openai = Router::new()
        .route("/v1/models", get(openai_list_models))
        .route("/v1/chat/completions", post(openai_proxy_chat_completions))
        .route("/v1/completions", post(openai_proxy_completions))
        .route("/v1/embeddings", post(openai_proxy_embeddings))
        .route_layer(middleware::from_fn_with_state(
            state.db.clone(),
            auth::require_user_or_api_key,
        ));

    // Protected routes (require user session)
    let protected = Router::new()
        .route(
            "/auth/me",
            get(auth_endpoints::me).put(auth_endpoints::update_me),
        )
        .route(
            "/auth/me/password",
            axum::routing::put(auth_endpoints::change_password),
        )
        // Locales
        .route("/locales", get(locales_endpoint::list_locales))
        // API Keys (dashboard-managed)
        .route(
            "/api_keys",
            get(api_keys::list_api_keys).post(api_keys::create_api_key),
        )
        .route(
            "/api_keys/:id",
            axum::routing::put(api_keys::update_api_key).delete(api_keys::revoke_api_key),
        )
        // Runtime models (models in service + historical + counters)
        .route("/runtime/models", get(list_runtime_models))
        // GPU activity (nvtop-like)
        .route("/gpu/activity", get(list_gpu_activity))
        .route("/deployments", post(create_deployment))
        // Realtime (SSE)
        .route("/events/stream", get(events_stream))
        // Models (catalog)
        .route("/models", get(list_models).post(create_model))
        .route(
            "/models/:id",
            get(get_model).put(update_model).delete(delete_model),
        )
        // Instances
        .route("/instances", get(list_instances))
        .route("/instances/search", get(search_instances))
        .route(
            "/instances/:id/archive",
            axum::routing::put(archive_instance),
        )
        .route(
            "/instances/:id",
            get(get_instance).delete(terminate_instance),
        )
        .route(
            "/instances/:id/reinstall",
            axum::routing::post(reinstall_instance),
        )
        // Action logs
        .route("/action_logs", get(list_action_logs))
        .route(
            "/action_logs/search",
            get(action_logs_search::search_action_logs),
        )
        .route("/action_types", get(list_action_types))
        // Commands
        .route("/reconcile", post(manual_reconcile_trigger))
        .route("/catalog/sync", post(manual_catalog_sync_trigger))
        // Settings
        .route(
            "/providers",
            get(settings::list_providers).post(settings::create_provider),
        )
        .route("/providers/search", get(settings::search_providers))
        .route(
            "/providers/:id",
            axum::routing::put(settings::update_provider),
        )
        .route(
            "/settings/definitions",
            get(provider_settings::list_settings_definitions),
        )
        .route(
            "/settings/global",
            get(provider_settings::list_global_settings)
                .put(provider_settings::upsert_global_setting),
        )
        // Provider-scoped params
        .route(
            "/providers/params",
            get(provider_settings::list_provider_params),
        )
        .route(
            "/providers/:id/params",
            axum::routing::put(provider_settings::update_provider_params),
        )
        .route(
            "/regions",
            get(settings::list_regions).post(settings::create_region),
        )
        .route("/regions/search", get(settings::search_regions))
        .route("/regions/:id", axum::routing::put(settings::update_region))
        .route(
            "/zones",
            get(settings::list_zones).post(settings::create_zone),
        )
        .route("/zones/search", get(settings::search_zones))
        .route("/zones/:id", axum::routing::put(settings::update_zone))
        .route(
            "/instance_types",
            get(settings::list_instance_types).post(settings::create_instance_type),
        )
        .route(
            "/instance_types/search",
            get(settings::search_instance_types),
        )
        .route(
            "/instance_types/:id",
            axum::routing::put(settings::update_instance_type),
        )
        // Instance Type <-> Zones
        .route(
            "/instance_types/:id/zones",
            get(instance_type_zones::list_instance_type_zones),
        )
        .route(
            "/instance_types/:id/zones",
            axum::routing::put(instance_type_zones::associate_zones_to_instance_type),
        )
        .route(
            "/zones/:zone_id/instance_types",
            get(instance_type_zones::list_instance_types_for_zone),
        )
        // Finops
        .route("/finops/cost/current", get(finops::get_cost_current))
        .route(
            "/finops/dashboard/costs/current",
            get(finops::get_costs_dashboard_current),
        )
        .route(
            "/finops/dashboard/costs/summary",
            get(finops::get_costs_dashboard_summary),
        )
        .route(
            "/finops/dashboard/costs/window",
            get(finops::get_costs_dashboard_window),
        )
        .route(
            "/finops/cost/forecast/minute",
            get(finops::get_cost_forecast_series),
        )
        .route(
            "/finops/cost/actual/minute",
            get(finops::get_cost_actual_series),
        )
        .route(
            "/finops/cost/cumulative/minute",
            get(finops::get_cost_cumulative_series),
        )
        // Users management
        .route(
            "/users",
            get(users_endpoint::list_users).post(users_endpoint::create_user),
        )
        .route(
            "/users/:id",
            get(users_endpoint::get_user)
                .put(users_endpoint::update_user)
                .delete(users_endpoint::delete_user),
        )
        .route_layer(middleware::from_fn(auth::require_user));

    let app = Router::new()
        .merge(public)
        .merge(worker)
        .merge(openai)
        .merge(protected)
        .layer(cors) // Apply CORS to ALL routes
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8003));
    println!("Backend listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn orchestrator_internal_url() -> String {
    std::env::var("ORCHESTRATOR_INTERNAL_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "http://orchestrator:8001".to_string())
        .trim_end_matches('/')
        .to_string()
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ReadyWorkerRow {
    id: uuid::Uuid,
    ip_address: String,
    worker_vllm_port: Option<i32>,
    worker_queue_depth: Option<i32>,
    worker_last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn stable_hash_u64(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn openai_worker_stale_seconds_env() -> i64 {
    std::env::var("OPENAI_WORKER_STALE_SECONDS")
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|v| *v >= 10 && *v <= 24 * 60 * 60)
        .unwrap_or(300) // 5 minutes
}

async fn openai_worker_stale_seconds_db(db: &Pool<Postgres>) -> i64 {
    // Global settings override (DB) -> env -> settings_definitions default -> hard default.
    let from_db: Option<i64> = sqlx::query_scalar(
        "SELECT value_int FROM global_settings WHERE key = 'OPENAI_WORKER_STALE_SECONDS'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    if let Some(v) = from_db {
        return v.clamp(10, 24 * 60 * 60);
    }

    let env_v = openai_worker_stale_seconds_env();
    if env_v > 0 {
        return env_v;
    }

    let def_v: Option<i64> = sqlx::query_scalar(
        "SELECT default_int FROM settings_definitions WHERE key = 'OPENAI_WORKER_STALE_SECONDS' AND scope = 'global'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    def_v.unwrap_or(300).clamp(10, 24 * 60 * 60)
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
struct RuntimeModelRow {
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
async fn list_runtime_models(State(state): State<Arc<AppState>>) -> Json<Vec<RuntimeModelRow>> {
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
struct GpuActivityParams {
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

#[derive(Serialize)]
struct GpuActivitySample {
    ts: String,
    gpu_pct: Option<f64>,
    vram_pct: Option<f64>,
    temp_c: Option<f64>,
    power_w: Option<f64>,
    power_limit_w: Option<f64>,
}

#[derive(Serialize)]
struct GpuActivityGpuSeries {
    gpu_index: i32,
    samples: Vec<GpuActivitySample>,
}

#[derive(Serialize)]
struct GpuActivityInstanceSeries {
    instance_id: uuid::Uuid,
    instance_name: Option<String>,
    provider_name: Option<String>,
    gpu_count: Option<i32>,
    gpus: Vec<GpuActivityGpuSeries>,
}

#[derive(Serialize)]
struct GpuActivityResponse {
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
async fn list_gpu_activity(
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

    let rows: Vec<GpuSampleRow> = match gran.as_str() {
        "minute" => sqlx::query_as::<Postgres, GpuSampleRow>(
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
                  i.gpu_count as gpu_count
                FROM gpu_samples_1m gs
                JOIN instances i ON i.id = gs.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                WHERE gs.bucket > NOW() - make_interval(secs => $1)
                  AND ($2::uuid IS NULL OR gs.instance_id = $2)
                ORDER BY gs.instance_id, gs.gpu_index, gs.bucket ASC
                "#,
        )
        .bind(window_s)
        .bind(instance_filter)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default(),
        "hour" => sqlx::query_as::<Postgres, GpuSampleRow>(
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
                  i.gpu_count as gpu_count
                FROM gpu_samples_1h gs
                JOIN instances i ON i.id = gs.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                WHERE gs.bucket > NOW() - make_interval(secs => $1)
                  AND ($2::uuid IS NULL OR gs.instance_id = $2)
                ORDER BY gs.instance_id, gs.gpu_index, gs.bucket ASC
                "#,
        )
        .bind(window_s)
        .bind(instance_filter)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default(),
        "day" => sqlx::query_as::<Postgres, GpuSampleRow>(
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
                  i.gpu_count as gpu_count
                FROM gpu_samples_1d gs
                JOIN instances i ON i.id = gs.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                WHERE gs.bucket > NOW() - make_interval(secs => $1)
                  AND ($2::uuid IS NULL OR gs.instance_id = $2)
                ORDER BY gs.instance_id, gs.gpu_index, gs.bucket ASC
                "#,
        )
        .bind(window_s)
        .bind(instance_filter)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default(),
        // second (default): raw table (still can be sparse depending on heartbeat interval)
        _ => sqlx::query_as::<Postgres, GpuSampleRow>(
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
                  i.gpu_count as gpu_count
                FROM gpu_samples gs
                JOIN instances i ON i.id = gs.instance_id
                LEFT JOIN providers p ON p.id = i.provider_id
                WHERE gs.time > NOW() - make_interval(secs => $1)
                  AND ($2::uuid IS NULL OR gs.instance_id = $2)
                ORDER BY gs.instance_id, gs.gpu_index, gs.time ASC
                "#,
        )
        .bind(window_s)
        .bind(instance_filter)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default(),
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

async fn bump_runtime_model_counters(db: &Pool<Postgres>, model_id: &str, ok: bool) {
    let mid = model_id.trim();
    if mid.is_empty() {
        return;
    }
    let _ = sqlx::query(
        r#"
        INSERT INTO runtime_models (model_id, first_seen_at, last_seen_at, total_requests, failed_requests)
        VALUES ($1, NOW(), NOW(), 1, CASE WHEN $2 THEN 0 ELSE 1 END)
        ON CONFLICT (model_id) DO UPDATE
          SET last_seen_at = GREATEST(runtime_models.last_seen_at, NOW()),
              total_requests = runtime_models.total_requests + 1,
              failed_requests = runtime_models.failed_requests + (CASE WHEN $2 THEN 0 ELSE 1 END)
        "#,
    )
    .bind(mid)
    .bind(ok)
    .execute(db)
    .await;
}

async fn select_ready_worker_for_model(
    db: &Pool<Postgres>,
    model: &str,
    sticky_key: Option<&str>,
) -> Option<(uuid::Uuid, String)> {
    // `model` here is the vLLM/OpenAI model id (HF repo id).
    // We route based on `instances.worker_model_id` (set by worker heartbeat/register).
    let model = model.trim();
    let stale = openai_worker_stale_seconds_db(db).await;
    let rows = sqlx::query_as::<Postgres, ReadyWorkerRow>(
        r#"
        SELECT
          id,
          ip_address::text as ip_address,
          worker_vllm_port,
          worker_queue_depth,
          worker_last_heartbeat
        FROM instances
        WHERE status::text = 'ready'
          AND ip_address IS NOT NULL
          AND (worker_status = 'ready' OR worker_status IS NULL)
          AND ($1::text = '' OR worker_model_id = $1)
          -- Use the same freshness signal as /v1/models + /runtime/models:
          -- allow either worker heartbeat OR orchestrator health timestamps to keep the instance routable.
          AND GREATEST(
              COALESCE(worker_last_heartbeat, 'epoch'::timestamptz),
              COALESCE(last_health_check, 'epoch'::timestamptz),
              COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
            ) > NOW() - ($2::bigint * INTERVAL '1 second')
        ORDER BY worker_queue_depth NULLS LAST,
                 GREATEST(
                   COALESCE(worker_last_heartbeat, 'epoch'::timestamptz),
                   COALESCE(last_health_check, 'epoch'::timestamptz),
                   COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
                 ) DESC,
                 created_at DESC
        LIMIT 50
        "#,
    )
    .bind(model)
    .bind(stale)
    .fetch_all(db)
    .await
    .ok()?;

    if rows.is_empty() {
        return None;
    }

    let chosen = if let Some(key) = sticky_key.filter(|k| !k.trim().is_empty()) {
        // Stable-ish affinity to an instance across requests (best effort).
        let mut sorted = rows;
        sorted.sort_by_key(|r| r.id);
        let idx = (stable_hash_u64(key) as usize) % sorted.len();
        sorted[idx].clone()
    } else {
        rows[0].clone()
    };

    let ip = chosen
        .ip_address
        .split('/')
        .next()
        .unwrap_or(&chosen.ip_address)
        .to_string();
    let port = chosen.worker_vllm_port.unwrap_or(8000).max(1) as i32;
    Some((chosen.id, format!("http://{}:{}", ip, port)))
}

async fn resolve_openai_model_id(db: &Pool<Postgres>, requested: Option<&str>) -> Option<String> {
    // Accept either:
    // - HF repo id (models.model_id)
    // - UUID (models.id) as string
    // If missing, fallback to WORKER_MODEL_ID env (dev convenience).
    let Some(raw) = requested
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            std::env::var("WORKER_MODEL_ID")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
    else {
        return None;
    };

    // If it looks like a UUID, try resolve -> HF repo id.
    if let Ok(uid) = uuid::Uuid::parse_str(&raw) {
        let hf: Option<String> =
            sqlx::query_scalar("SELECT model_id FROM models WHERE id = $1 AND is_active = true")
                .bind(uid)
                .fetch_optional(db)
                .await
                .ok()
                .flatten();
        return hf.or(Some(raw));
    }

    // If it matches an active catalog entry by HF repo id, return it; else keep as-is.
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM models WHERE model_id = $1 AND is_active = true)",
    )
    .bind(&raw)
    .fetch_one(db)
    .await
    .unwrap_or(false);
    if exists {
        Some(raw)
    } else {
        Some(raw)
    }
}

async fn openai_list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Return *live* models based on worker heartbeats:
    // - if at least 1 READY worker serves model_id and heartbeat is recent -> exposed in /v1/models
    // - if no workers for a model for a while -> disappears (staleness window)
    #[derive(Serialize, sqlx::FromRow)]
    struct Row {
        model_id: String,
        last_seen: chrono::DateTime<chrono::Utc>,
    }
    #[derive(Serialize)]
    struct ModelObj {
        id: String,
        object: &'static str,
        created: i64,
        owned_by: &'static str,
    }
    #[derive(Serialize)]
    struct Resp {
        object: &'static str,
        data: Vec<ModelObj>,
    }

    let stale = openai_worker_stale_seconds_db(&state.db).await;
    let rows = sqlx::query_as::<Postgres, Row>(
        r#"
        SELECT
          worker_model_id as model_id,
          MAX(
            GREATEST(
              COALESCE(worker_last_heartbeat, 'epoch'::timestamptz),
              COALESCE(last_health_check, 'epoch'::timestamptz),
              COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
            )
          ) as last_seen
        FROM instances
        WHERE status::text = 'ready'
          AND ip_address IS NOT NULL
          AND (worker_status = 'ready' OR worker_status IS NULL)
          AND worker_model_id IS NOT NULL
          AND GREATEST(
              COALESCE(worker_last_heartbeat, 'epoch'::timestamptz),
              COALESCE(last_health_check, 'epoch'::timestamptz),
              COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz)
            ) > NOW() - ($1::bigint * INTERVAL '1 second')
        GROUP BY worker_model_id
        ORDER BY worker_model_id
        "#,
    )
    .bind(stale)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let data = rows
        .into_iter()
        .map(|r| ModelObj {
            id: r.model_id,
            object: "model",
            created: r.last_seen.timestamp(),
            owned_by: "inventiv",
        })
        .collect();

    Json(Resp {
        object: "list",
        data,
    })
}

async fn openai_proxy_chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    openai_proxy_to_worker(&state, "/v1/chat/completions", headers, body).await
}

async fn openai_proxy_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    openai_proxy_to_worker(&state, "/v1/completions", headers, body).await
}

async fn openai_proxy_embeddings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    openai_proxy_to_worker(&state, "/v1/embeddings", headers, body).await
}

async fn openai_proxy_to_worker(
    state: &Arc<AppState>,
    path: &str,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let v: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_json"})),
            )
                .into_response();
        }
    };
    let requested_model = v.get("model").and_then(|m| m.as_str());
    let model_id = match resolve_openai_model_id(&state.db, requested_model).await {
        Some(m) => m,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"missing_model"})),
            )
                .into_response();
        }
    };
    let stream = v.get("stream").and_then(|b| b.as_bool()).unwrap_or(false);

    // Sticky key: user-provided; forwarded to worker-local HAProxy to keep affinity in multi-vLLM mode.
    // Also used best-effort for instance selection (stable hashing).
    let sticky = header_value(&headers, "X-Inventiv-Session");

    let Some((instance_id, base_url)) =
        select_ready_worker_for_model(&state.db, &model_id, sticky.as_deref()).await
    else {
        bump_runtime_model_counters(&state.db, &model_id, false).await;
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error":"no_ready_worker",
                "message":"No READY worker found for requested model",
                "model": model_id
            })),
        )
            .into_response();
    };

    let target = format!("{}{}", base_url.trim_end_matches('/'), path);

    let mut client_builder =
        reqwest::Client::builder().connect_timeout(std::time::Duration::from_secs(3));
    // Non-stream responses should be bounded; stream can be long-lived.
    if stream {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(0));
    } else {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(60));
    }
    let client = client_builder.build();
    let Ok(client) = client else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"http_client_build_failed"})),
        )
            .into_response();
    };

    // Forward headers (allowlist).
    //
    // IMPORTANT:
    // - Do NOT forward client Authorization (API key) to workers.
    // - Keep headers minimal to avoid hop-by-hop / proxy-only headers causing issues.
    let mut out_headers = reqwest::header::HeaderMap::new();
    if let Some(ct) = headers.get(axum::http::header::CONTENT_TYPE) {
        out_headers.insert(reqwest::header::CONTENT_TYPE, ct.clone());
    } else {
        out_headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
    }
    if let Some(acc) = headers.get(axum::http::header::ACCEPT) {
        out_headers.insert(reqwest::header::ACCEPT, acc.clone());
    } else {
        out_headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
    }
    // Ensure sticky header is forwarded (HAProxy config uses it).
    if let Some(sid) = sticky.as_deref() {
        if let Ok(val) = reqwest::header::HeaderValue::from_str(sid) {
            out_headers.insert(
                reqwest::header::HeaderName::from_static("x-inventiv-session"),
                val,
            );
        }
    }

    // Proxy request.
    let upstream = match client
        .post(&target)
        .headers(out_headers)
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            bump_runtime_model_counters(&state.db, &model_id, false).await;
            let _ = simple_logger::log_action_with_metadata(
                &state.db,
                "OPENAI_PROXY",
                "failed",
                Some(instance_id),
                Some("upstream_request_failed"),
                Some(json!({"target": target, "error": e.to_string()})),
            )
            .await;
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error":"upstream_unreachable","message":e.to_string()})),
            )
                .into_response();
        }
    };

    let status = upstream.status();
    let mut resp_headers = axum::http::HeaderMap::new();
    // Preserve content-type for SSE streaming.
    if let Some(ct) = upstream.headers().get(reqwest::header::CONTENT_TYPE) {
        if let Ok(cts) = ct.to_str() {
            if let Ok(v) = axum::http::HeaderValue::from_str(cts) {
                resp_headers.insert(axum::http::header::CONTENT_TYPE, v);
            }
        }
    }

    if stream {
        // Count as success if upstream accepted (2xx). We'll treat other codes as failed.
        bump_runtime_model_counters(&state.db, &model_id, status.is_success()).await;
        let byte_stream = upstream.bytes_stream().map(|chunk| {
            chunk.map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "upstream_stream_error")
            })
        });
        return (status, resp_headers, Body::from_stream(byte_stream)).into_response();
    }

    let bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => {
            bump_runtime_model_counters(&state.db, &model_id, false).await;
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error":"upstream_read_failed","message":e.to_string()})),
            )
                .into_response();
        }
    };
    bump_runtime_model_counters(&state.db, &model_id, status.is_success()).await;
    (status, resp_headers, bytes).into_response()
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return None;
    };
    let Ok(auth) = auth.to_str() else {
        return None;
    };
    let auth = auth.trim();
    let prefix = "Bearer ";
    if auth.len() <= prefix.len() || !auth.starts_with(prefix) {
        return None;
    }
    Some(auth[prefix.len()..].trim().to_string())
}

async fn verify_worker_token_db(db: &Pool<Postgres>, instance_id: uuid::Uuid, token: &str) -> bool {
    // Compare hash in DB using pgcrypto digest; avoids adding crypto deps in Rust.
    let ok: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM worker_auth_tokens
          WHERE instance_id = $1
            AND revoked_at IS NULL
            AND token_hash = encode(digest($2::text, 'sha256'), 'hex')
        )
        "#,
    )
    .bind(instance_id)
    .bind(token)
    .fetch_one(db)
    .await
    .unwrap_or(false);

    if ok {
        let _ = sqlx::query(
            "UPDATE worker_auth_tokens SET last_seen_at = NOW() WHERE instance_id = $1",
        )
            .bind(instance_id)
            .execute(db)
            .await;
    }

    ok
}

async fn verify_worker_auth_api(
    db: &Pool<Postgres>,
    headers: &HeaderMap,
    instance_id: uuid::Uuid,
) -> bool {
    // Backward-compat: allow a global token (useful for early bringup).
    let expected = std::env::var("WORKER_AUTH_TOKEN").unwrap_or_default();
    if !expected.trim().is_empty() {
        if let Some(tok) = extract_bearer(headers) {
            if tok.trim() == expected.trim() {
                return true;
            }
        }
    }

    let Some(tok) = extract_bearer(headers) else {
        return false;
    };
    verify_worker_token_db(db, instance_id, &tok).await
}

async fn proxy_post_to_orchestrator(
    path: &str,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    let base = orchestrator_internal_url();
    let url = format!("{}/{}", base, path.trim_start_matches('/'));

    let mut req = reqwest::Client::new().post(url).body(body.to_vec());
    // Forward Authorization header (worker auth token)
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = auth.to_str() {
            req = req.header(axum::http::header::AUTHORIZATION, s);
        }
    }
    // Preserve content-type if present
    if let Some(ct) = headers.get(axum::http::header::CONTENT_TYPE) {
        if let Ok(s) = ct.to_str() {
            req = req.header(axum::http::header::CONTENT_TYPE, s);
        }
    }

    // Forward client IP chain so Orchestrator can apply bootstrap IP checks.
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(s) = xff.to_str() {
            req = req.header("x-forwarded-for", s);
        }
    }
    if let Some(xri) = headers.get("x-real-ip") {
        if let Ok(s) = xri.to_str() {
            req = req.header("x-real-ip", s);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let bytes = resp.bytes().await.unwrap_or_default();
            (status, bytes).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error":"orchestrator_unreachable","message": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct WorkerInstanceIdPayload {
    instance_id: uuid::Uuid,
}

async fn proxy_worker_register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    // Bootstrap flow: allow missing token on register (orchestrator will check IP + token existence).
    // If a token IS present, we verify it here too (defense-in-depth).
    if extract_bearer(&headers).is_some() {
        let parsed: WorkerInstanceIdPayload =
            match serde_json::from_slice(&body) {
            Ok(p) => p,
                Err(_) => return (
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error":"invalid_body","message":"missing_or_invalid_instance_id"}),
                    ),
                )
                    .into_response(),
        };
        if !verify_worker_auth_api(&state.db, &headers, parsed.instance_id).await {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":"unauthorized"})),
            )
                .into_response();
        }
    }

    proxy_post_to_orchestrator("/internal/worker/register", headers, body).await
}

async fn proxy_worker_heartbeat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    // Heartbeat always requires a valid worker token.
    let parsed: WorkerInstanceIdPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"invalid_body","message":"missing_or_invalid_instance_id"})),
            )
                .into_response()
        }
    };

    if !verify_worker_auth_api(&state.db, &headers, parsed.instance_id).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unauthorized"})),
        )
            .into_response();
    }

    proxy_post_to_orchestrator("/internal/worker/heartbeat", headers, body).await
}

async fn maybe_seed_catalog(pool: &Pool<Postgres>) {
    let enabled = std::env::var("AUTO_SEED_CATALOG")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    if !enabled {
        return;
    }
    // Important: do NOT skip seeding based on one table (e.g. providers).
    // We want seeding to be re-runnable and idempotent (the seed file should use ON CONFLICT),
    // otherwise partial resets (like TRUNCATE action_types) would leave the UI broken.

    let seed_path = std::env::var("SEED_CATALOG_PATH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "seeds/catalog_seeds.sql".to_string());

    let sql = match fs::read_to_string(&seed_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("⚠️  AUTO_SEED_CATALOG failed to read {}: {}", seed_path, e);
            return;
        }
    };

    // Very simple splitter: seed file is expected to be plain SQL statements separated by ';'
    // and may contain '--' line comments.
    let mut cleaned = String::new();
    for line in sql.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") {
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }

    let statements: Vec<String> = cleaned
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("{};", s))
        .collect();

    if statements.is_empty() {
        eprintln!(
            "⚠️  AUTO_SEED_CATALOG: no statements found in {}",
            seed_path
        );
        return;
    }

    println!(
        "🌱 AUTO_SEED_CATALOG: seeding {} statements from {}",
        statements.len(),
        seed_path
    );
    for (idx, stmt) in statements.iter().enumerate() {
        if let Err(e) = sqlx::query(stmt).execute(pool).await {
            eprintln!(
                "❌ AUTO_SEED_CATALOG failed at statement {}: {}",
                idx + 1,
                e
            );
            return;
        }
    }
    // After seeding, ensure i18n backfill exists for seeded rows (seeds run after migrations).
    // This is safe to run repeatedly and keeps the DB consistent even when seeds insert rows with NULL *_i18n_id.
    ensure_catalog_i18n_backfill(pool).await;
    println!("✅ AUTO_SEED_CATALOG done");
}

async fn ensure_catalog_i18n_backfill(pool: &Pool<Postgres>) {
    // Best-effort: never fail startup due to i18n backfill.
    let _ = sqlx::query(
        r#"
        -- Providers
        UPDATE public.providers
        SET name_i18n_id = COALESCE(name_i18n_id, gen_random_uuid()),
            description_i18n_id = COALESCE(description_i18n_id, gen_random_uuid());

        -- Regions/Zones/Instance Types
        UPDATE public.regions SET name_i18n_id = COALESCE(name_i18n_id, gen_random_uuid());
        UPDATE public.zones SET name_i18n_id = COALESCE(name_i18n_id, gen_random_uuid());
        UPDATE public.instance_types SET name_i18n_id = COALESCE(name_i18n_id, gen_random_uuid());
        UPDATE public.action_types SET label_i18n_id = COALESCE(label_i18n_id, gen_random_uuid());

        -- Ensure keys exist
        INSERT INTO public.i18n_keys (id)
        SELECT DISTINCT x.id
        FROM (
          SELECT name_i18n_id AS id FROM public.providers
          UNION ALL SELECT description_i18n_id AS id FROM public.providers
          UNION ALL SELECT name_i18n_id AS id FROM public.regions
          UNION ALL SELECT name_i18n_id AS id FROM public.zones
          UNION ALL SELECT name_i18n_id AS id FROM public.instance_types
          UNION ALL SELECT label_i18n_id AS id FROM public.action_types
        ) x
        WHERE x.id IS NOT NULL
        ON CONFLICT (id) DO NOTHING;

        -- Seed en-US texts from base columns if missing
        INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
        SELECT p.name_i18n_id, 'en-US', p.name
        FROM public.providers p
        WHERE p.name_i18n_id IS NOT NULL
          AND COALESCE(p.name, '') <> ''
        ON CONFLICT (key_id, locale_code) DO NOTHING;

        INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
        SELECT p.description_i18n_id, 'en-US', p.description
        FROM public.providers p
        WHERE p.description_i18n_id IS NOT NULL
          AND COALESCE(p.description, '') <> ''
        ON CONFLICT (key_id, locale_code) DO NOTHING;

        INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
        SELECT r.name_i18n_id, 'en-US', r.name
        FROM public.regions r
        WHERE r.name_i18n_id IS NOT NULL
          AND COALESCE(r.name, '') <> ''
        ON CONFLICT (key_id, locale_code) DO NOTHING;

        INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
        SELECT z.name_i18n_id, 'en-US', z.name
        FROM public.zones z
        WHERE z.name_i18n_id IS NOT NULL
          AND COALESCE(z.name, '') <> ''
        ON CONFLICT (key_id, locale_code) DO NOTHING;

        INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
        SELECT it.name_i18n_id, 'en-US', it.name
        FROM public.instance_types it
        WHERE it.name_i18n_id IS NOT NULL
          AND COALESCE(it.name, '') <> ''
        ON CONFLICT (key_id, locale_code) DO NOTHING;

        INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
        SELECT at.label_i18n_id, 'en-US', at.label
        FROM public.action_types at
        WHERE at.label_i18n_id IS NOT NULL
          AND COALESCE(at.label, '') <> ''
        ON CONFLICT (key_id, locale_code) DO NOTHING;
        "#
    )
    .execute(pool)
    .await;
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct InstanceResponse {
    pub id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub zone_id: Option<uuid::Uuid>,
    pub instance_type_id: Option<uuid::Uuid>,
    /// Provisioned model (catalog) selected at deployment time (optional).
    pub model_id: Option<uuid::Uuid>,
    pub model_name: Option<String>,
    /// Model code / HF repo id for the provisioned model (optional).
    pub model_code: Option<String>,
    pub provider_instance_id: Option<String>,
    pub status: String,
    pub ip_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub terminated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,
    pub last_reconciliation: Option<chrono::DateTime<chrono::Utc>>,
    pub health_check_failures: Option<i32>,
    pub deletion_reason: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    
    // Joined Fields
    pub provider_name: String,
    pub region: String,
    pub zone: String,
    pub instance_type: String,
    pub gpu_vram: Option<i32>,
    pub gpu_count: Option<i32>, // NEW: Distinct GPU count
    pub cost_per_hour: Option<f64>,
    pub total_cost: Option<f64>,
    pub is_archived: bool,
    pub deleted_by_provider: Option<bool>,
}

#[derive(Deserialize, IntoParams)]
pub struct ListInstanceParams {
    pub archived: Option<bool>,
}

#[derive(Deserialize, IntoParams, utoipa::ToSchema)]
pub struct SearchInstancesParams {
    pub archived: Option<bool>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    /// Sort field allowlist: created_at|status|provider|region|zone|type|cost_per_hour|total_cost
    pub sort_by: Option<String>,
    /// "asc" | "desc"
    pub sort_dir: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SearchInstancesResponse {
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub filtered_count: i64,
    pub rows: Vec<InstanceResponse>,
}

async fn root() -> &'static str {
    "Inventiv Backend API (Product Plane) - CQRS Enabled"
}

// ... (DeploymentRequest structs) ...

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
struct DeploymentRequest {
    /// Preferred way to select provider (stable): e.g. "scaleway", "mock"
    provider_code: Option<String>,
    /// Backward-compat (deprecated): provider UUID
    provider_id: Option<uuid::Uuid>,
    zone: String,
    instance_type: String,
    /// Optional model selection (UUID from /models). If omitted, orchestrator may fallback to env default.
    model_id: Option<uuid::Uuid>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct DeploymentResponse {
    status: String,
    instance_id: String, // Renamed from deployment_id for clarity
    message: Option<String>,
}

#[derive(Deserialize, IntoParams, utoipa::ToSchema)]
pub struct ListModelsParams {
    pub active: Option<bool>,
    /// Optional sort field (allowlist).
    pub order_by: Option<String>,
    /// "asc" | "desc"
    pub order_dir: Option<String>,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct CreateModelRequest {
    pub name: String,
    /// Hugging Face model repo id (or local path)
    pub model_id: String,
    pub required_vram_gb: i32,
    pub context_length: i32,
    pub is_active: Option<bool>,
    /// Recommended data volume size (GB) for this model (optional).
    pub data_volume_gb: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct UpdateModelRequest {
    pub name: Option<String>,
    pub model_id: Option<String>,
    pub required_vram_gb: Option<i32>,
    pub context_length: Option<i32>,
    pub is_active: Option<bool>,
    pub data_volume_gb: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

#[utoipa::path(
    get,
    path = "/models",
    params(ListModelsParams),
    responses((status = 200, description = "List models", body = [inventiv_common::LlmModel]))
)]
async fn list_models(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ListModelsParams>,
) -> impl IntoResponse {
    let dir = match params
        .order_dir
        .as_deref()
        .unwrap_or("asc")
        .to_ascii_lowercase()
        .as_str()
    {
        "desc" => "DESC",
        _ => "ASC",
    };
    let order_by = match params.order_by.as_deref() {
        Some("model_id") => "model_id",
        Some("required_vram_gb") => "required_vram_gb",
        Some("context_length") => "context_length",
        Some("data_volume_gb") => "data_volume_gb",
        Some("is_active") => "is_active",
        Some("created_at") => "created_at",
        Some("updated_at") => "updated_at",
        _ => "name",
    };

    let base = r#"SELECT id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at
                 FROM models"#;
    let where_clause = if params.active == Some(true) {
        " WHERE is_active = true"
    } else {
        ""
    };
    let sql = format!(
        r#"{base}{where_clause}
           ORDER BY {order_by} {dir}, id {dir}"#
    );

    let rows: Vec<LlmModel> = sqlx::query_as(&sql)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
    (StatusCode::OK, Json(rows)).into_response()
}

#[utoipa::path(
    get,
    path = "/models/{id}",
    responses((status = 200, description = "Get model", body = inventiv_common::LlmModel))
)]
async fn get_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uid) = uuid::Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_id"}))).into_response();
    };
    let row: Option<LlmModel> = sqlx::query_as(
        r#"SELECT id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at
           FROM models WHERE id = $1"#,
    )
    .bind(uid)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);
    match row {
        Some(m) => (StatusCode::OK, Json(m)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/models",
    request_body = CreateModelRequest,
    responses((status = 201, description = "Created", body = inventiv_common::LlmModel))
)]
async fn create_model(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateModelRequest>,
) -> impl IntoResponse {
    let id = uuid::Uuid::new_v4();
    let is_active = payload.is_active.unwrap_or(true);
    let metadata = sqlx::types::Json(payload.metadata.unwrap_or_else(|| json!({})));
    let res: Result<LlmModel, sqlx::Error> = sqlx::query_as(
        r#"INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW(),NOW())
           RETURNING id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at"#,
    )
    .bind(id)
    .bind(payload.name)
    .bind(payload.model_id)
    .bind(payload.required_vram_gb)
    .bind(payload.context_length)
    .bind(is_active)
    .bind(payload.data_volume_gb)
    .bind(metadata)
    .fetch_one(&state.db)
    .await;
    match res {
        Ok(m) => (StatusCode::CREATED, Json(m)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/models/{id}",
    request_body = UpdateModelRequest,
    responses((status = 200, description = "Updated", body = inventiv_common::LlmModel))
)]
async fn update_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateModelRequest>,
) -> impl IntoResponse {
    let Ok(uid) = uuid::Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_id"}))).into_response();
    };
    let metadata = payload.metadata.map(sqlx::types::Json);
    let row: Result<LlmModel, sqlx::Error> = sqlx::query_as(
        r#"UPDATE models
           SET name = COALESCE($2, name),
               model_id = COALESCE($3, model_id),
               required_vram_gb = COALESCE($4, required_vram_gb),
               context_length = COALESCE($5, context_length),
               is_active = COALESCE($6, is_active),
               data_volume_gb = COALESCE($7, data_volume_gb),
               metadata = COALESCE($8, metadata),
               updated_at = NOW()
           WHERE id = $1
           RETURNING id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at"#,
    )
    .bind(uid)
    .bind(payload.name)
    .bind(payload.model_id)
    .bind(payload.required_vram_gb)
    .bind(payload.context_length)
    .bind(payload.is_active)
    .bind(payload.data_volume_gb)
    .bind(metadata)
    .fetch_one(&state.db)
    .await;
    match row {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(sqlx::Error::RowNotFound) => {
            (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/models/{id}",
    responses((status = 200, description = "Deleted"))
)]
async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uid) = uuid::Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_id"}))).into_response();
    };
    let res = sqlx::query("DELETE FROM models WHERE id = $1")
        .bind(uid)
        .execute(&state.db)
        .await;
    match res {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"status":"ok"}))).into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response(),
        Err(e) => {
            // Most likely FK violation if instances still reference this model.
            let msg = e.to_string();
            let code = if msg.contains("foreign key") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (code, Json(json!({"error":"db_error","message": msg}))).into_response()
        }
    }
}

// COMMAND : CREATE DEPLOYMENT
#[utoipa::path(
    post,
    path = "/deployments",
    request_body = DeploymentRequest,
    responses(
        (status = 200, description = "Deployment Accepted", body = DeploymentResponse)
    )
)]
async fn create_deployment(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeploymentRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let instance_id_uuid = uuid::Uuid::new_v4(); // Create UUID first
    let instance_id = instance_id_uuid.to_string();

    let requested_provider_code: Option<String> = payload
        .provider_code
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());

    // Resolve provider UUID from provider_code if provided (preferred).
    // If resolution fails we still insert an instance row (traceability), but validation will fail.
    let provider_id_resolved: Option<uuid::Uuid> = if let Some(pid) = payload.provider_id {
        Some(pid)
    } else if let Some(code) = requested_provider_code.as_deref() {
        sqlx::query_scalar("SELECT id FROM providers WHERE code = $1 LIMIT 1")
            .bind(code)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None)
    } else {
        // No provider specified -> default to provider code "scaleway"
        // (no hardcoded UUIDs; seed controls the actual id)
        sqlx::query_scalar("SELECT id FROM providers WHERE code = 'scaleway' LIMIT 1")
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None)
    };

    let provider_id = match provider_id_resolved {
        Some(id) => id,
        None => {
            // Can't resolve provider -> fail early (but still keep instance row traceable).
            // We'll insert with a dummy provider_id? Not possible due FK, so we must stop here.
            return (
                StatusCode::BAD_REQUEST,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some(
                        "Unknown provider (provider_code/provider_id not found)".to_string(),
                    ),
                }),
            )
                .into_response();
        }
    };

    // We want a durable instance_id from the very first request, even when validation fails.
    // So we insert the instance row first (zone/type can be NULL), then all errors can be logged with instance_id.
    //
    // If this ever collides (extremely unlikely), we return 409 so devs notice immediately.
    let insert_initial = sqlx::query(
        "INSERT INTO instances (id, provider_id, zone_id, instance_type_id, status, created_at, gpu_profile)
         VALUES ($1, $2, NULL, NULL, 'provisioning', NOW(), '{}')"
    )
    .bind(instance_id_uuid)
    .bind(provider_id)
    .execute(&state.db)
    .await;

    if let Err(e) = insert_initial {
        // If duplicate key, surface loudly to detect any upstream bug.
        let _msg = format!("Failed to create initial instance id: {:?}", e);
        let is_unique_violation = matches!(e, sqlx::Error::Database(ref db_err) if db_err.code().as_deref() == Some("23505"));

        return (
            if is_unique_violation {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(if is_unique_violation {
                    "Instance id collision (duplicate primary key)".to_string()
                } else {
                    "Database error while creating initial instance id".to_string()
                }),
            }),
        )
            .into_response();
    }

    // LOG 1: REQUEST_CREATE (request is now traceable by instance_id even if validation fails)
    let log_id = simple_logger::log_action_with_metadata(
        &state.db,
        "REQUEST_CREATE",
        "in_progress",
        Some(instance_id_uuid),
        None,
        Some(serde_json::json!({
            "provider_id": provider_id.to_string(),
            "provider_code": requested_provider_code,
            "zone": payload.zone,
            "instance_type": payload.instance_type,
            "model_id": payload.model_id.map(|m| m.to_string()),
        })),
    )
    .await
    .ok();

    // Basic validation: even if invalid, we keep the instance row + log tied to instance_id.
    if payload.zone.trim().is_empty() || payload.instance_type.trim().is_empty() {
        let msg = "Missing zone or instance_type";
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("MISSING_PARAMS")
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": "MISSING_PARAMS"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(msg.to_string()),
            }),
        )
            .into_response();
    }

    // Model is mandatory: request cannot be created without defining the model to install.
    if payload.model_id.is_none() {
        let msg = "Missing model_id";
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("MISSING_MODEL")
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": "MISSING_MODEL"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(msg.to_string()),
            }),
        )
            .into_response();
    }

    // Provider must exist and be active.
    // If a provider_code was provided but did not resolve, treat as invalid.
    let provider_active: bool = if requested_provider_code.is_some()
        && payload.provider_id.is_none()
        && provider_id_resolved.is_none()
    {
        false
    } else {
        sqlx::query_scalar("SELECT COALESCE(is_active, false) FROM providers WHERE id = $1")
            .bind(provider_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None)
            .unwrap_or(false)
    };

    if !provider_active {
        let msg = "Invalid provider (not found or inactive)";
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("INVALID_PROVIDER")
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": "INVALID_PROVIDER"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some("Invalid provider (not found or inactive)".to_string()),
            }),
        )
            .into_response();
    }

    // Zone must be active AND belong to the provider via its region
    let zone_row: Option<(uuid::Uuid, bool, bool)> = sqlx::query_as(
        r#"SELECT z.id
                , z.is_active
                , r.is_active
           FROM zones z
           JOIN regions r ON r.id = z.region_id
           WHERE z.code = $1
             AND r.provider_id = $2"#,
    )
    .bind(&payload.zone)
    .bind(provider_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    // Instance type must be active and belong to the provider
    let type_row: Option<(uuid::Uuid, bool)> = sqlx::query_as(
        r#"SELECT it.id
                , it.is_active
           FROM instance_types it
           WHERE it.code = $1
             AND it.provider_id = $2"#,
    )
    .bind(&payload.instance_type)
    .bind(provider_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    // Persist resolved ids (even if inactive) to keep request traceable in instances table
    let resolved_zone_id: Option<uuid::Uuid> = zone_row.map(|(id, _z_active, _r_active)| id);
    let resolved_type_id: Option<uuid::Uuid> = type_row.map(|(id, _active)| id);
    let _ = sqlx::query(
        "UPDATE instances SET zone_id=$2, instance_type_id=$3, model_id=$4 WHERE id=$1",
    )
        .bind(instance_id_uuid)
        .bind(resolved_zone_id)
        .bind(resolved_type_id)
    .bind(payload.model_id)
        .execute(&state.db)
        .await;

    // Validation
    let mut validation_error: Option<(&'static str, &'static str)> = None;
    match zone_row {
        None => validation_error = Some(("INVALID_ZONE", "Invalid zone (not found for provider)")),
        Some((_id, z_active, r_active)) if !z_active || !r_active => {
            validation_error = Some(("INACTIVE_ZONE", "Zone is inactive"))
        }
        _ => {}
    }
    match type_row {
        None => {
            validation_error = Some((
                "INVALID_INSTANCE_TYPE",
                "Invalid instance type (not found for provider)",
            ))
        }
        Some((_id, active)) if !active => {
            validation_error = Some(("INACTIVE_INSTANCE_TYPE", "Instance type is inactive"))
        }
        _ => {}
    }

    // Validate model (mandatory)
    if validation_error.is_none() {
        let mid = payload.model_id.expect("validated above");
        let m: Option<bool> = sqlx::query_scalar("SELECT is_active FROM models WHERE id = $1")
            .bind(mid)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);
        match m {
            None => validation_error = Some(("INVALID_MODEL", "Invalid model_id (not found)")),
            Some(false) => validation_error = Some(("INACTIVE_MODEL", "Model is inactive")),
            Some(true) => {}
        }
    }

    if let Some((code, msg)) = validation_error {
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind(code)
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": code})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(msg.to_string()),
            }),
        )
            .into_response();
    }

    // Guardrails for Scaleway worker auto-install:
    // - instance type must be available in the zone (instance_type_zones.is_available)
    // - instance type must match the allowlist patterns when WORKER_AUTO_INSTALL=1
    let provider_code: String =
        sqlx::query_scalar("SELECT COALESCE(code, '') FROM providers WHERE id = $1")
            .bind(provider_id)
            .fetch_one(&state.db)
            .await
            .unwrap_or_default();

    let auto_install = std::env::var("WORKER_AUTO_INSTALL")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    if auto_install && provider_code.to_ascii_lowercase() == "scaleway" {
        let (Some(zid), Some(tid)) = (resolved_zone_id, resolved_type_id) else {
            // Should not happen after validation, but keep it safe.
            let msg = "Missing resolved zone/type id after validation";
            let _ = sqlx::query(
                "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                 WHERE id=$1"
            )
            .bind(instance_id_uuid)
            .bind("INVALID_CATALOG_STATE")
            .bind(msg)
            .execute(&state.db)
            .await;
            return (
                StatusCode::BAD_REQUEST,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some(msg.to_string()),
                }),
            )
                .into_response();
        };

        let is_available: bool = sqlx::query_scalar(
            r#"
            SELECT COALESCE(itz.is_available, false)
            FROM instance_type_zones itz
            WHERE itz.instance_type_id = $1
              AND itz.zone_id = $2
            "#,
        )
        .bind(tid)
        .bind(zid)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None)
        .unwrap_or(false);

        if !is_available {
            let code = "INSTANCE_TYPE_NOT_AVAILABLE_IN_ZONE";
            let msg = "Instance type is not available in this zone (catalog)";
            let _ = sqlx::query(
                "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                 WHERE id=$1"
            )
            .bind(instance_id_uuid)
            .bind(code)
            .bind(msg)
            .execute(&state.db)
            .await;

            if let Some(id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    id,
                    "failed",
                    duration,
                    Some(msg),
                    Some(serde_json::json!({"error_code": code})),
                )
                .await
                .ok();
            }

            return (
                StatusCode::BAD_REQUEST,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some(msg.to_string()),
                }),
            )
                .into_response();
        }

        let patterns = inventiv_common::worker_target::parse_instance_type_patterns(
            std::env::var("WORKER_AUTO_INSTALL_INSTANCE_PATTERNS")
                .ok()
                .as_deref(),
        );
        let is_supported = inventiv_common::worker_target::instance_type_matches_patterns(
            &payload.instance_type,
            &patterns,
        );
        if !is_supported {
            let code = "INSTANCE_TYPE_NOT_SUPPORTED";
            let msg = format!(
                "Instance type '{}' not supported for worker auto-install (patterns={:?})",
                payload.instance_type, patterns
            );
            let _ = sqlx::query(
                "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                 WHERE id=$1"
            )
            .bind(instance_id_uuid)
            .bind(code)
            .bind(&msg)
            .execute(&state.db)
            .await;

            if let Some(id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    id,
                    "failed",
                    duration,
                    Some(&msg),
                    Some(serde_json::json!({"error_code": code, "patterns": patterns})),
                )
                .await
                .ok();
            }

            return (
                StatusCode::BAD_REQUEST,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some(msg),
                }),
            )
                .into_response();
        }
    }
    
    println!("🚀 New Instance Creation Request: {}", instance_id);

    // Publish Event to Redis
    let event_payload = serde_json::json!({
        "type": "CMD:PROVISION",
        "instance_id": instance_id,
        "provider_id": provider_id.to_string(),
        "zone": payload.zone,
        "instance_type": payload.instance_type,
        "correlation_id": log_id.map(|id| id.to_string()),
    })
    .to_string();

    println!(
        "📤 Publishing provisioning event to Redis: {}",
        event_payload
    );

    match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            match conn
                .publish::<_, _, ()>("orchestrator_events", &event_payload)
                .await
            {
                Ok(_) => {
                    println!("✅ Provisioning event published successfully");
                    // Log completion
                    if let Some(id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            id,
                            "success",
                            duration,
                            None,
                            Some(serde_json::json!({"redis_published": true, "event_type": "CMD:PROVISION"})),
                        ).await.ok();
                    }

                    (
                        StatusCode::ACCEPTED,
                        Json(DeploymentResponse {
                        status: "accepted".to_string(),
                        instance_id,
                        message: None,
                        }),
                    )
                        .into_response()
                }
                Err(e) => {
                    let error_msg = format!("Failed to publish to Redis: {:?}", e);
                    println!("❌ {}", error_msg);
                    if let Some(id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            id,
                            "failed",
                            duration,
                            Some(&error_msg),
                            Some(serde_json::json!({"redis_published": false, "event_type": "CMD:PROVISION", "error_code": "REDIS_PUBLISH_FAILED"})),
                        ).await.ok();
                    }
                    let _ = sqlx::query(
                        "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                         WHERE id=$1"
                    )
                    .bind(instance_id_uuid)
                    .bind("REDIS_PUBLISH_FAILED")
                    .bind(&error_msg)
                    .execute(&state.db)
                    .await;
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(DeploymentResponse {
                            status: "failed".to_string(),
                            instance_id,
                            message: Some("Failed to queue provisioning event".to_string()),
                        }),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to connect to Redis: {:?}", e);
            println!("❌ {}", error_msg);
            if let Some(id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    id,
                    "failed",
                    duration,
                    Some(&error_msg),
                    Some(serde_json::json!({"redis_published": false, "event_type": "CMD:PROVISION", "error_code": "REDIS_CONNECT_FAILED"})),
                ).await.ok();
            }
            let _ = sqlx::query(
                "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                 WHERE id=$1"
            )
            .bind(instance_id_uuid)
            .bind("REDIS_CONNECT_FAILED")
            .bind(&error_msg)
            .execute(&state.db)
            .await;
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some("Failed to connect to Redis".to_string()),
                }),
            )
                .into_response()
        }
    }
}

/// POST /reconcile - Trigger manual reconciliation
#[utoipa::path(
    post,
    path = "/reconcile",
    responses(
        (status = 200, description = "Reconciliation triggered", body = serde_json::Value),
        (status = 500, description = "Failed to trigger reconciliation", body = serde_json::Value)
    )
)]
async fn manual_reconcile_trigger(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    println!("🔍 Manual reconciliation triggered via API");

    // Publish Redis event for orchestrator
    let event_payload = serde_json::json!({
        "type": "CMD:RECONCILE"
    })
    .to_string();

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    // Use turbofish to specify return type as unit ()
    match conn
        .publish::<_, _, ()>("orchestrator_events", &event_payload)
        .await
    {
        Ok(_) => Json(json!({
                "status": "triggered",
                "message": "Reconciliation task has been triggered"
        })),
        Err(e) => {
            eprintln!("Failed to publish reconciliation event: {:?}", e);
            Json(json!({
                "status": "error",
                "message": format!("Failed to trigger reconciliation: {:?}", e)
            }))
        }
    }
}

/// POST /catalog/sync - Trigger catalog synchronization
#[utoipa::path(
    post,
    path = "/catalog/sync",
    responses(
        (status = 200, description = "Catalog Sync triggered", body = serde_json::Value),
        (status = 500, description = "Failed to trigger sync", body = serde_json::Value)
    )
)]
async fn manual_catalog_sync_trigger(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    println!("🔄 Catalog Sync triggered via API");

    // Publish Redis event for orchestrator
    let event_payload = serde_json::json!({
        "type": "CMD:SYNC_CATALOG"
    })
    .to_string();

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    // Use turbofish to specify return type as unit ()
    match conn
        .publish::<_, _, ()>("orchestrator_events", &event_payload)
        .await
    {
        Ok(_) => Json(json!({
                "status": "triggered",
                "message": "Catalog Sync task has been triggered"
        })),
        Err(e) => {
            eprintln!("Failed to publish sync event: {:?}", e);
            Json(json!({
                "status": "error",
                "message": format!("Failed to trigger sync: {:?}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/instances",
    params(ListInstanceParams),
    responses(
        (status = 200, description = "List all instances with details", body = Vec<InstanceResponse>)
    )
)]
async fn list_instances(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ListInstanceParams>,
) -> Json<Vec<InstanceResponse>> {
    let show_archived = params.archived.unwrap_or(false);

    let instances = sqlx::query_as::<Postgres, InstanceResponse>(
        r#"
        SELECT 
            i.id, i.provider_id, i.zone_id, i.instance_type_id,
            i.model_id,
            m.name as model_name,
            m.model_id as model_code,
            i.provider_instance_id::text as provider_instance_id,
            i.status::text as status, 
            i.ip_address::text as ip_address, 
            i.created_at,
            i.terminated_at,
            i.last_health_check,
            (i.last_reconciliation AT TIME ZONE 'UTC') as last_reconciliation,
            i.health_check_failures,
            i.deletion_reason,
            i.error_code,
            i.error_message,
            i.is_archived,
            i.deleted_by_provider,
            COALESCE(p.name, 'Unknown Provider') as provider_name,
            COALESCE(z.name, 'Unknown Zone') as zone,
            COALESCE(r.name, 'Unknown Region') as region,
            COALESCE(it.name, 'Unknown Type') as instance_type,
            it.vram_per_gpu_gb as gpu_vram,
            it.gpu_count as gpu_count,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            (EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8) as total_cost
        FROM instances i
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN zones z ON i.zone_id = z.id
        LEFT JOIN regions r ON z.region_id = r.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
        LEFT JOIN models m ON m.id = i.model_id
        WHERE i.is_archived = $1
        ORDER BY i.created_at DESC
        "#
    )
    .bind(show_archived)
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    Json(instances)
}

#[utoipa::path(
    get,
    path = "/instances/search",
    params(SearchInstancesParams),
    responses((status = 200, description = "Paged search instances (virtualized UI)", body = SearchInstancesResponse))
)]
async fn search_instances(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<SearchInstancesParams>,
) -> Json<SearchInstancesResponse> {
    let show_archived = params.archived.unwrap_or(false);
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);
    let dir = match params
        .sort_dir
        .as_deref()
        .unwrap_or("desc")
        .to_ascii_lowercase()
        .as_str()
    {
        "asc" => "ASC",
        _ => "DESC",
    };

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instances")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let filtered_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instances WHERE is_archived = $1")
            .bind(show_archived)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

    let total_cost_expr = "(EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8)";
    let order_by = match params.sort_by.as_deref() {
        Some("status") => "i.status",
        Some("provider") => "p.name",
        Some("region") => "r.name",
        Some("zone") => "z.name",
        Some("type") => "it.name",
        Some("cost_per_hour") => "it.cost_per_hour",
        Some("total_cost") => total_cost_expr,
        _ => "i.created_at",
    };

    let sql = format!(
        r#"
        SELECT 
            i.id, i.provider_id, i.zone_id, i.instance_type_id,
            i.model_id,
            m.name as model_name,
            m.model_id as model_code,
            i.provider_instance_id::text as provider_instance_id,
            i.status::text as status, 
            i.ip_address::text as ip_address, 
            i.created_at,
            i.terminated_at,
            i.last_health_check,
            (i.last_reconciliation AT TIME ZONE 'UTC') as last_reconciliation,
            i.health_check_failures,
            i.deletion_reason,
            i.error_code,
            i.error_message,
            i.is_archived,
            i.deleted_by_provider,
            COALESCE(p.name, 'Unknown Provider') as provider_name,
            COALESCE(z.name, 'Unknown Zone') as zone,
            COALESCE(r.name, 'Unknown Region') as region,
            COALESCE(it.name, 'Unknown Type') as instance_type,
            it.vram_per_gpu_gb as gpu_vram,
            it.gpu_count as gpu_count,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            {total_cost_expr} as total_cost
        FROM instances i
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN zones z ON i.zone_id = z.id
        LEFT JOIN regions r ON z.region_id = r.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
        LEFT JOIN models m ON m.id = i.model_id
        WHERE i.is_archived = $1
        ORDER BY {order_by} {dir} NULLS LAST, i.id {dir}
        LIMIT $2 OFFSET $3
        "#
    );

    let rows: Vec<InstanceResponse> = sqlx::query_as::<Postgres, InstanceResponse>(&sql)
        .bind(show_archived)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(SearchInstancesResponse {
        offset,
        limit,
        total_count,
        filtered_count,
        rows,
    })
}

#[utoipa::path(
    get,
    path = "/instances/{id}",
    params(
        ("id" = Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 200, description = "Instance details", body = InstanceResponse),
        (status = 404, description = "Instance not found")
    )
)]
async fn get_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<Postgres, InstanceResponse>(
        r#"
        SELECT 
            i.id, i.provider_id, i.zone_id, i.instance_type_id,
            i.model_id,
            m.name as model_name,
            m.model_id as model_code,
            i.provider_instance_id::text as provider_instance_id,
            i.status::text as status, 
            i.ip_address::text as ip_address, 
            i.created_at,
            i.terminated_at,
            i.last_health_check,
            (i.last_reconciliation AT TIME ZONE 'UTC') as last_reconciliation,
            i.health_check_failures,
            i.deletion_reason,
            i.error_code,
            i.error_message,
            i.is_archived,
            i.deleted_by_provider,
            COALESCE(p.name, 'Unknown Provider') as provider_name,
            COALESCE(z.name, 'Unknown Zone') as zone,
            COALESCE(r.name, 'Unknown Region') as region,
            COALESCE(it.name, 'Unknown Type') as instance_type,
            it.vram_per_gpu_gb as gpu_vram,
            it.gpu_count as gpu_count,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            (EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8) as total_cost
        FROM instances i
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN zones z ON i.zone_id = z.id
        LEFT JOIN regions r ON z.region_id = r.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
        LEFT JOIN models m ON m.id = i.model_id
        WHERE i.id = $1
        LIMIT 1
        "#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(inst) => Json(inst).into_response(),
        None => (StatusCode::NOT_FOUND, "Instance not found").into_response(),
    }
}

// Archive endpoint (logged version below)
// COMMAND : ARCHIVE INSTANCE
#[utoipa::path(
    put,
    path = "/instances/{id}/archive",
    params(
        ("id" = Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 200, description = "Instance Archived"),
        (status = 400, description = "Bad Request"),
        (status = 500, description = "Server Error")
    )
)]
async fn archive_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Log start of archive action
    let start = std::time::Instant::now();
    let log_id =
        simple_logger::log_action(&state.db, "ARCHIVE_INSTANCE", "in_progress", Some(id), None)
    .await
    .ok();

    let result = sqlx::query(
        "UPDATE instances
         SET is_archived = true,
             status = 'archived'
         WHERE id = $1
           AND status IN ('terminated', 'archived')",
    )
    .bind(id)
    .execute(&state.db)
    .await;

    let response = match result {
        Ok(r) if r.rows_affected() > 0 => (StatusCode::OK, "Instance Archived"),
        Ok(_) => (
            StatusCode::BAD_REQUEST,
            "Instance not found or not terminated",
        ),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database Error"),
    };

    // Log completion
    if let Some(lid) = log_id {
        let duration = start.elapsed().as_millis() as i32;
        let status_str = match response.0 {
            StatusCode::OK => "success",
            _ => "failed",
        };
        let err_msg = if response.0 == StatusCode::OK {
            None
        } else {
            Some(response.1)
        };
        simple_logger::log_action_complete(&state.db, lid, status_str, duration, err_msg)
            .await
            .ok();
    }

    response.into_response()
}

// COMMAND : TERMINATE INSTANCE
#[utoipa::path(
    delete,
    path = "/instances/{id}",
    params(
        ("id" = Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 202, description = "Termination Accepted")
    )
)]
async fn terminate_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Log start of archive action
    let start = std::time::Instant::now();
    let log_id = simple_logger::log_action_with_metadata(
        &state.db,
        "REQUEST_TERMINATE",
        "in_progress",
        Some(id),
        None,
        Some(serde_json::json!({
            "instance_id": id.to_string(),
        })),
    )
    .await
    .ok();
    
    println!("🗑️ Termination Request: {}", id);

    // 1. Fetch instance so we can handle edge-cases safely (no provider resource, missing zone, etc.)
    let instance_row: Option<(Option<String>, Option<uuid::Uuid>, String)> = sqlx::query_as(
        "SELECT provider_instance_id::text, zone_id, status::text FROM instances WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some((provider_instance_id_opt, zone_id_opt, status)) = instance_row else {
        println!("⚠️  Instance {} not found for termination", id);
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Instance not found"),
            )
                .await
                .ok();
        }
        return (StatusCode::NOT_FOUND, "Instance not found").into_response();
    };

    if status == "terminated" {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "success",
                duration,
                None,
                Some(serde_json::json!({"already_terminated": true})),
            )
            .await
            .ok();
        }
        return (StatusCode::OK, "Already terminated").into_response();
    }

    // If there is no provider resource to delete (provider_instance_id missing), we terminate immediately.
    // This prevents "terminating forever" for failed/invalid provisioning requests.
    //
    // IMPORTANT: if provider_instance_id exists but zone_id is missing, we must NOT mark terminated:
    // we can't safely call the provider API and risk leaking resources. We keep 'terminating' and let
    // admin/operator handle the missing catalog linkage.
    if provider_instance_id_opt.as_deref().unwrap_or("").is_empty() {
        let _ = sqlx::query(
            "UPDATE instances
             SET status='terminated',
                 terminated_at = COALESCE(terminated_at, NOW()),
                 deletion_reason = COALESCE(deletion_reason, 'no_provider_resource')
             WHERE id=$1 AND status != 'terminated'",
        )
        .bind(id)
        .execute(&state.db)
        .await;

        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "success",
                duration,
                None,
                Some(serde_json::json!({
                    "immediate": true,
                    "reason": "no_provider_resource",
                    "provider_instance_id_present": provider_instance_id_opt.is_some(),
                    "zone_id_present": zone_id_opt.is_some(),
                })),
            )
            .await
            .ok();
        }

        return (StatusCode::OK, "Terminated (no provider resource)").into_response();
    }

    if zone_id_opt.is_none() {
        // Can't safely terminate on provider without a zone -> keep terminating and surface an error.
        let _ = sqlx::query(
            "UPDATE instances
             SET status='terminating',
                 error_code = COALESCE(error_code, 'MISSING_ZONE'),
                 error_message = COALESCE(error_message, 'Missing zone for termination'),
                 last_reconciliation = NULL
             WHERE id=$1 AND status != 'terminated'",
        )
        .bind(id)
        .execute(&state.db)
        .await;

        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "success",
                duration,
                Some("Missing zone: kept terminating for manual recovery"),
                Some(serde_json::json!({
                    "immediate": false,
                    "reason": "missing_zone",
                    "provider_instance_id_present": provider_instance_id_opt.is_some(),
                    "zone_id_present": zone_id_opt.is_some(),
                })),
            )
            .await
            .ok();
        }

        // Still publish CMD:TERMINATE (best effort) in case orchestrator can reconcile other metadata,
        // but the terminator job will also pick it up via status='terminating'.
        // (We don't early-return here; continue to publish.)
    }

    // 2. Update status to 'terminating' in DB (provider resource exists, orchestrator will delete it)
    let update_result = sqlx::query(
        "UPDATE instances
         SET status = 'terminating',
             last_reconciliation = NULL
         WHERE id = $1 AND status != 'terminated'",
    )
    .bind(id)
    .execute(&state.db)
    .await;

    match update_result {
        Ok(result) if result.rows_affected() > 0 => {
            println!("✅ Instance {} status set to 'terminating'", id)
        }
        Ok(_) => {
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete(
                    &state.db,
                    log_id,
                    "failed",
                    duration,
                    Some("Instance not found"),
                )
                    .await
                    .ok();
            }
            return (StatusCode::NOT_FOUND, "Instance not found").into_response();
        }
        Err(e) => {
            println!("❌ Failed to update instance status: {:?}", e);
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                let msg = format!("Database error: {:?}", e);
                simple_logger::log_action_complete(
                    &state.db,
                    log_id,
                    "failed",
                    duration,
                    Some(&msg),
                )
                    .await
                    .ok();
            }
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    }

    // 3. Send termination event to orchestrator (async)
    let event = serde_json::json!({
        "type": "CMD:TERMINATE",
        "instance_id": id.to_string(),
        "correlation_id": log_id.map(|id| id.to_string()),
    })
    .to_string();

    println!("📤 Publishing termination event to Redis: {}", event);
    
    match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            match conn
                .publish::<_, _, ()>("orchestrator_events", &event)
                .await
            {
                Ok(_) => {
                    println!("✅ Termination event published successfully");
                    // Log success
                    if let Some(log_id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            log_id,
                            "success",
                            duration,
                            None,
                            Some(serde_json::json!({"redis_published": true, "event_type": "CMD:TERMINATE"})),
                        ).await.ok();
                    }
                    (StatusCode::ACCEPTED, "Termination initiated").into_response()
                }
                Err(e) => {
                    let error_msg = format!("Failed to publish to Redis: {:?}", e);
                    println!("❌ {}", error_msg);
                    if let Some(log_id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            log_id,
                            "failed",
                            duration,
                            Some(&error_msg),
                            Some(serde_json::json!({"redis_published": false, "event_type": "CMD:TERMINATE"})),
                        ).await.ok();
                    }
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to queue termination",
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to connect to Redis: {:?}", e);
            println!("❌ {}", error_msg);
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    log_id,
                    "failed",
                    duration,
                    Some(&error_msg),
                    Some(serde_json::json!({"redis_published": false, "event_type": "CMD:TERMINATE"})),
                ).await.ok();
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to queue termination",
            )
                .into_response()
        }
    }
}

// COMMAND : REINSTALL INSTANCE (force SSH bootstrap again)
#[utoipa::path(
    post,
    path = "/instances/{id}/reinstall",
    params(
        ("id" = Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 202, description = "Reinstall Accepted")
    )
)]
async fn reinstall_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let log_id = simple_logger::log_action_with_metadata(
        &state.db,
        "REQUEST_REINSTALL",
        "in_progress",
        Some(id),
        None,
        Some(serde_json::json!({
            "instance_id": id.to_string(),
        })),
    )
    .await
    .ok();

    // Validate instance exists and is eligible
    let instance_row: Option<(Option<String>, Option<String>, String)> = sqlx::query_as(
        "SELECT provider_instance_id::text, ip_address::text, status::text FROM instances WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some((provider_instance_id_opt, ip_opt, status)) = instance_row else {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Instance not found"),
            )
            .await
            .ok();
        }
        return (StatusCode::NOT_FOUND, "Instance not found").into_response();
    };

    if status == "terminated" {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Instance is terminated"),
                Some(serde_json::json!({"status": status})),
            )
            .await
            .ok();
        }
        return (StatusCode::BAD_REQUEST, "Instance is terminated").into_response();
    }
    if status == "terminating" {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Instance is terminating"),
                Some(serde_json::json!({"status": status})),
            )
            .await
            .ok();
        }
        return (StatusCode::CONFLICT, "Instance is terminating").into_response();
    }

    // Must have a reachable VM to reinstall.
    if provider_instance_id_opt.as_deref().unwrap_or("").is_empty()
        || ip_opt.as_deref().unwrap_or("").is_empty()
    {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Missing provider_instance_id or ip_address"),
                Some(serde_json::json!({
                    "provider_instance_id_present": provider_instance_id_opt.as_deref().unwrap_or("").is_empty() == false,
                    "ip_address_present": ip_opt.as_deref().unwrap_or("").is_empty() == false,
                })),
            )
            .await
            .ok();
        }
        return (
            StatusCode::BAD_REQUEST,
            "Instance not reachable (missing provider_instance_id or ip_address)",
        )
            .into_response();
    }

    // Mark as booting again (repair workflow) to re-enable health-check flow.
    let _ = sqlx::query(
        "UPDATE instances
         SET status = 'booting',
             error_code = NULL,
             error_message = NULL
         WHERE id = $1
           AND status NOT IN ('terminated', 'terminating')",
    )
    .bind(id)
    .execute(&state.db)
    .await;

    // Publish reinstall command to orchestrator
    let event = serde_json::json!({
        "type": "CMD:REINSTALL",
        "instance_id": id.to_string(),
        "correlation_id": log_id.map(|id| id.to_string()),
    })
    .to_string();

    match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => match conn
            .publish::<_, _, ()>("orchestrator_events", &event)
            .await
        {
            Ok(_) => {
                if let Some(log_id) = log_id {
                    let duration = start.elapsed().as_millis() as i32;
                    simple_logger::log_action_complete_with_metadata(
                        &state.db,
                        log_id,
                        "success",
                        duration,
                        None,
                        Some(serde_json::json!({"redis_published": true, "event_type": "CMD:REINSTALL"})),
                    )
                    .await
                    .ok();
                }
                (StatusCode::ACCEPTED, "Reinstall initiated").into_response()
            }
            Err(e) => {
                let error_msg = format!("Failed to publish to Redis: {:?}", e);
                if let Some(log_id) = log_id {
                    let duration = start.elapsed().as_millis() as i32;
                    simple_logger::log_action_complete_with_metadata(
                        &state.db,
                        log_id,
                        "failed",
                        duration,
                        Some(&error_msg),
                        Some(serde_json::json!({"redis_published": false, "event_type": "CMD:REINSTALL"})),
                    )
                    .await
                    .ok();
                }
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to queue reinstall",
                )
                    .into_response()
            }
        },
        Err(e) => {
            let error_msg = format!("Failed to connect to Redis: {:?}", e);
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    log_id,
                    "failed",
                    duration,
                    Some(&error_msg),
                    Some(serde_json::json!({"redis_published": false, "event_type": "CMD:REINSTALL"})),
                )
                .await
                .ok();
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to queue reinstall",
            )
                .into_response()
        }
    }
}

// ============================================================================
// ACTION LOGS API
// ============================================================================

use axum::extract::Query;

#[derive(Deserialize, IntoParams)]
struct ActionLogQuery {
    instance_id: Option<uuid::Uuid>,
    component: Option<String>,
    status: Option<String>,
    action_type: Option<String>,
    limit: Option<i32>,
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
struct ActionLogResponse {
    id: uuid::Uuid,
    action_type: String,
    component: String,
    status: String,
    error_message: Option<String>,
    instance_id: Option<uuid::Uuid>,
    duration_ms: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
    metadata: Option<serde_json::Value>, // Added metadata field
    instance_status_before: Option<String>,
    instance_status_after: Option<String>,
}

#[utoipa::path(
    get,
    path = "/action_logs",
    params(ActionLogQuery),
    responses(
        (status = 200, description = "List of action logs", body = Vec<ActionLogResponse>)
    )
)]
async fn list_action_logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ActionLogQuery>,
) -> Json<Vec<ActionLogResponse>> {
    let limit = params.limit.unwrap_or(100).min(1000);
    
    let logs = sqlx::query_as::<Postgres, ActionLogResponse>(
        "SELECT 
            id, action_type, component, status, 
            error_message, instance_id, duration_ms, created_at, metadata,
            instance_status_before, instance_status_after
         FROM action_logs
         WHERE ($1::uuid IS NULL OR instance_id = $1)
           AND ($2::text IS NULL OR component = $2)
           AND ($3::text IS NULL OR status = $3)
           AND ($4::text IS NULL OR action_type = $4)
         ORDER BY created_at DESC
         LIMIT $5",
    )
    .bind(params.instance_id)
    .bind(params.component)
    .bind(params.status)
    .bind(params.action_type)
    .bind(limit as i64)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    
    Json(logs)
}

// ============================================================================
// ACTION TYPES (UI CATALOG)
// ============================================================================

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
struct ActionTypeResponse {
    code: String,
    label: String,
    icon: String,
    color_class: String,
    category: Option<String>,
    is_active: bool,
}

#[utoipa::path(
    get,
    path = "/action_types",
    responses(
        (status = 200, description = "List of action types", body = Vec<ActionTypeResponse>)
    )
)]
async fn list_action_types(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> Json<Vec<ActionTypeResponse>> {
    let locale = user_locale::preferred_locale_code(&state.db, user.user_id).await;
    let rows = sqlx::query_as::<Postgres, ActionTypeResponse>(
        "SELECT
           code,
           COALESCE(i18n_get_text(label_i18n_id, $1), label) as label,
           icon,
           color_class,
           category,
           is_active
         FROM action_types
         WHERE is_active = true
         ORDER BY category NULLS LAST, code ASC",
    )
    .bind(locale)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}

// ============================================================================
// REALTIME (SSE)
// ============================================================================

#[derive(Deserialize)]
struct EventsStreamParams {
    // Optional: narrow action log events to a specific instance
    instance_id: Option<uuid::Uuid>,
    // Comma-separated topics. Default: instances,actions
    topics: Option<String>,
}

#[derive(Serialize)]
struct InstancesChangedPayload {
    ids: Vec<uuid::Uuid>,
    emitted_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
struct ActionLogsChangedPayload {
    ids: Vec<uuid::Uuid>,
    instance_ids: Vec<uuid::Uuid>,
    emitted_at: chrono::DateTime<chrono::Utc>,
}

async fn events_stream(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<EventsStreamParams>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let topics_raw = params
        .topics
        .unwrap_or_else(|| "instances,actions".to_string());
    let topics: std::collections::HashSet<String> = topics_raw
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let db = state.db.clone();
    let instance_id_filter = params.instance_id;

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        // Start at "now" so we don't flood on connect; the UI will do an initial fetch anyway.
        let mut last_instances_ts = chrono::Utc::now();
        let mut last_actions_ts = chrono::Utc::now();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));

        // Quick handshake
        let hello = Event::default().event("hello").data(r#"{"ok":true}"#);
        if tx.send(Ok(hello)).await.is_err() {
            return;
        }

        loop {
            interval.tick().await;

            if topics.contains("instances") {
                let rows: Vec<(uuid::Uuid, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
                    r#"
                    SELECT
                      id,
                      GREATEST(
                        COALESCE(created_at, 'epoch'::timestamptz),
                        COALESCE(ready_at, 'epoch'::timestamptz),
                        COALESCE(failed_at, 'epoch'::timestamptz),
                        COALESCE(terminated_at, 'epoch'::timestamptz),
                        COALESCE(last_health_check, 'epoch'::timestamptz),
                        COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz),
                        COALESCE(worker_last_heartbeat, 'epoch'::timestamptz)
                      ) AS changed_at
                    FROM instances
                    WHERE GREATEST(
                        COALESCE(created_at, 'epoch'::timestamptz),
                        COALESCE(ready_at, 'epoch'::timestamptz),
                        COALESCE(failed_at, 'epoch'::timestamptz),
                        COALESCE(terminated_at, 'epoch'::timestamptz),
                        COALESCE(last_health_check, 'epoch'::timestamptz),
                        COALESCE((last_reconciliation AT TIME ZONE 'UTC'), 'epoch'::timestamptz),
                        COALESCE(worker_last_heartbeat, 'epoch'::timestamptz)
                      ) > $1
                    ORDER BY changed_at ASC
                    LIMIT 200
                    "#,
                )
                .bind(last_instances_ts)
                .fetch_all(&db)
                .await
                .unwrap_or_default();

                if !rows.is_empty() {
                    let mut max_ts = last_instances_ts;
                    let ids: Vec<uuid::Uuid> = rows
                        .into_iter()
                        .map(|(id, ts)| {
                            if ts > max_ts {
                                max_ts = ts;
                            }
                            id
                        })
                        .collect();
                    last_instances_ts = max_ts;

                    let payload = InstancesChangedPayload {
                        ids,
                        emitted_at: chrono::Utc::now(),
                    };
                    let ev = Event::default()
                        .event("instance.updated")
                        .data(serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()));
                    if tx.send(Ok(ev)).await.is_err() {
                        return;
                    }
                }
            }

            if topics.contains("actions") || topics.contains("action_logs") {
                // Important: action logs often "update in place" (status in_progress -> success/failed)
                // by setting completed_at + duration + metadata. Track changes using changed_at.
                let rows: Vec<(
                    uuid::Uuid,
                    Option<uuid::Uuid>,
                    chrono::DateTime<chrono::Utc>,
                )> = sqlx::query_as(
                    r#"
                    SELECT
                      id,
                      instance_id,
                      GREATEST(created_at, COALESCE(completed_at, created_at)) AS changed_at
                    FROM action_logs
                    WHERE GREATEST(created_at, COALESCE(completed_at, created_at)) > $1
                      AND ($2::uuid IS NULL OR instance_id = $2)
                    ORDER BY changed_at ASC
                    LIMIT 500
                    "#,
                )
                .bind(last_actions_ts)
                .bind(instance_id_filter)
                .fetch_all(&db)
                .await
                .unwrap_or_default();

                if !rows.is_empty() {
                    let mut max_ts = last_actions_ts;
                    let mut ids = Vec::with_capacity(rows.len());
                    let mut inst_ids = std::collections::BTreeSet::new();
                    for (id, inst, ts) in rows {
                        ids.push(id);
                        if let Some(iid) = inst {
                            inst_ids.insert(iid);
                        }
                        if ts > max_ts {
                            max_ts = ts;
                        }
                    }
                    last_actions_ts = max_ts;

                    let payload = ActionLogsChangedPayload {
                        ids,
                        instance_ids: inst_ids.into_iter().collect(),
                        emitted_at: chrono::Utc::now(),
                    };
                    // Keep event name stable for the UI: treat it as "action log changed".
                    let ev = Event::default()
                        .event("action_log.created")
                        .data(serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()));
                    if tx.send(Ok(ev)).await.is_err() {
                        return;
                    }
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    )
}
