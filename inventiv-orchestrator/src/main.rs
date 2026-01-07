use axum::http::{HeaderMap, StatusCode};
use axum::{
    extract::{ConnectInfo, State},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use uuid::Uuid;
mod finops_events;
mod health_check_job;
mod logger;
mod models;
mod provider_manager; // NEW
mod provisioning_job;
mod recovery_job;
mod services; // NEW
mod terminator_job;
mod volume_reconciliation_job;
mod watch_dog_job;
// worker_storage moved to inventiv-common
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use tokio::time::{sleep, Duration};

struct AppState {
    db: Pool<Postgres>,
    redis_client: redis::Client,
}

#[derive(Deserialize, Debug)]
struct WorkerRegisterRequest {
    instance_id: Uuid,
    worker_id: Option<Uuid>,
    model_id: Option<String>,
    vllm_port: Option<i32>,
    health_port: Option<i32>,
    /// Optional: worker-reported reachable IP (useful for local/dev, and for providers where the worker is best source of truth).
    ip_address: Option<String>,
    metadata: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug)]
struct AgentInfo {
    version: Option<String>,
    build_date: Option<String>,
    checksum: Option<String>,
}

#[derive(Deserialize, Debug)]
struct WorkerHeartbeatRequest {
    instance_id: Uuid,
    worker_id: Option<Uuid>,
    status: String, // starting|ready|draining (lowercase)
    model_id: Option<String>,
    queue_depth: Option<i32>,
    gpu_utilization: Option<f64>,
    gpu_mem_used_mb: Option<f64>,
    /// Optional: worker-reported reachable IP.
    ip_address: Option<String>,
    /// Agent version/checksum information
    agent_info: Option<AgentInfo>,
    metadata: Option<serde_json::Value>,
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

fn request_client_ip(headers: &HeaderMap, connect: &SocketAddr) -> String {
    // Prefer X-Forwarded-For (edge/proxy), fallback to socket addr (direct).
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
        if let Some(first) = xff
            .split(',')
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            return first.to_string();
        }
    }
    connect.ip().to_string()
}

async fn verify_worker_token_db(db: &Pool<Postgres>, instance_id: Uuid, token: &str) -> bool {
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

async fn verify_worker_auth(db: &Pool<Postgres>, headers: &HeaderMap, instance_id: Uuid) -> bool {
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

async fn instance_bootstrap_allowed(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    client_ip: &str,
) -> bool {
    // Allow bootstrap when:
    // - instance exists
    // - no token exists yet
    // - and client_ip matches instance.ip_address
    let token_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM worker_auth_tokens WHERE instance_id = $1 AND revoked_at IS NULL)",
    )
    .bind(instance_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);

    if token_exists {
        return false;
    }

    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT i.ip_address::text as ip, p.code as provider_code
        FROM instances i
        JOIN providers p ON p.id = i.provider_id
        WHERE i.id = $1
        "#,
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let Some((ip_opt, provider_code_opt)) = row else {
        return false;
    };

    let _ = provider_code_opt; // keep for future audit logging if needed
    let Some(ip) = ip_opt else {
        return false;
    };
    let clean_ip = ip.split('/').next().unwrap_or(ip.as_str()).trim();
    clean_ip == client_ip.trim()
}

async fn issue_worker_token(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    worker_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
) -> Option<(String, String)> {
    let token = format!("wk_{}_{}", Uuid::new_v4(), Uuid::new_v4());
    let prefix = token.chars().take(12).collect::<String>();

    let res = sqlx::query(
        r#"
        INSERT INTO worker_auth_tokens (instance_id, token_hash, token_prefix, worker_id, metadata)
        VALUES ($1, encode(digest($2::text, 'sha256'), 'hex'), $3, $4, $5)
        ON CONFLICT (instance_id) DO NOTHING
        "#,
    )
    .bind(instance_id)
    .bind(&token)
    .bind(&prefix)
    .bind(worker_id)
    .bind(metadata)
    .execute(db)
    .await
    .ok()?;

    if res.rows_affected() == 0 {
        return None;
    }

    Some((token, prefix))
}

#[derive(serde::Deserialize, Debug)]
struct CommandProvision {
    instance_id: String,
    zone: String,
    instance_type: String,
    correlation_id: Option<String>,
}

mod health_check_flow;
mod migrations; // NEW
mod state_machine;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let redis_client = redis::Client::open(redis_url.clone()).unwrap();

    // Connect to Postgres
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    // Check connection
    sqlx::query("SELECT 1").execute(&pool).await.unwrap();
    println!("✅ Connected to Database");

    // Run migrations (source of truth is /migrations at workspace root)
    sqlx::migrate!("../sqlx-migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let state = Arc::new(AppState {
        db: pool,
        redis_client: redis_client.clone(),
    });

    // Kick an initial catalog sync shortly after startup so the Settings UI has data
    // (providers/regions/zones/instance types) without manual seeding on staging/prod.
    let db_catalog = state.db.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        services::process_catalog_sync(db_catalog).await;
    });

    // 3. Start Scaling Engine Loop (Background Task)
    let state_clone = state.clone();
    tokio::spawn(async move {
        scaling_engine_loop(state_clone).await;
    });

    // 4. Start Event Listener (Redis Subscriber)
    // Use dedicated PubSub connection
    let mut pubsub = redis_client.get_async_pubsub().await.unwrap();
    pubsub.subscribe("orchestrator_events").await.unwrap();
    println!("🎧 Orchestrator listening on Redis channel 'orchestrator_events'...");

    let state_redis = state.clone();
    tokio::spawn(async move {
        use futures_util::StreamExt;
        let mut stream = pubsub.on_message();

        while let Some(msg) = stream.next().await {
            let payload: String = msg.get_payload().unwrap();
            println!("📩 Received Event: {}", payload);

            if let Ok(event_json) = serde_json::from_str::<serde_json::Value>(&payload) {
                let event_type = event_json["type"].as_str().unwrap_or("");

                match event_type {
                    "CMD:PROVISION" => {
                        if let Ok(cmd) =
                            serde_json::from_value::<CommandProvision>(event_json.clone())
                        {
                            let instance_id = cmd.instance_id.clone();
                            eprintln!("📥 [Redis] Received CMD:PROVISION for instance {} (zone={}, type={})", 
                                instance_id, cmd.zone, cmd.instance_type);
                            let pool = state_redis.db.clone();
                            let redis_client = state_redis.redis_client.clone();
                            tokio::spawn(async move {
                                eprintln!(
                                    "🔵 [Redis] Spawning process_provisioning task for instance {}",
                                    instance_id
                                );
                                services::process_provisioning(
                                    pool,
                                    redis_client,
                                    cmd.instance_id,
                                    cmd.zone,
                                    cmd.instance_type,
                                    cmd.correlation_id,
                                )
                                .await;
                                eprintln!("🔵 [Redis] process_provisioning task completed for instance {}", instance_id);
                            });
                        } else {
                            eprintln!(
                                "⚠️ [Redis] Failed to parse CMD:PROVISION event: {}",
                                payload
                            );
                        }
                    }
                    "CMD:TERMINATE" => {
                        if let Ok(cmd) =
                            serde_json::from_value::<CommandTerminate>(event_json.clone())
                        {
                            eprintln!(
                                "📥 [Redis] Received CMD:TERMINATE for instance {}",
                                cmd.instance_id
                            );
                            let pool = state_redis.db.clone();
                            let redis_client = state_redis.redis_client.clone();
                            tokio::spawn(async move {
                                services::process_termination(
                                    pool,
                                    redis_client,
                                    cmd.instance_id,
                                    cmd.correlation_id,
                                )
                                .await;
                            });
                        } else {
                            eprintln!(
                                "⚠️ [Redis] Failed to parse CMD:TERMINATE event: {}",
                                payload
                            );
                        }
                    }
                    "CMD:REINSTALL" => {
                        if let Ok(cmd) =
                            serde_json::from_value::<CommandReinstall>(event_json.clone())
                        {
                            println!("📥 Received Reinstall Command");
                            let pool = state_redis.db.clone();
                            let redis_client = state_redis.redis_client.clone();
                            tokio::spawn(async move {
                                services::process_reinstall(
                                    pool,
                                    redis_client,
                                    cmd.instance_id,
                                    cmd.correlation_id,
                                )
                                .await;
                            });
                        }
                    }
                    "CMD:SYNC_CATALOG" => {
                        println!("📥 Received Sync Catalog Command");
                        let pool = state_redis.db.clone();
                        tokio::spawn(async move {
                            services::process_catalog_sync(pool).await;
                        });
                    }
                    "CMD:RECONCILE" => {
                        println!("📥 Received Manual Reconciliation Command");
                        let pool = state_redis.db.clone();
                        tokio::spawn(async move {
                            services::process_full_reconciliation(pool).await;
                        });
                    }
                    _ => eprintln!("⚠️  Unknown event type: {}", event_type),
                }
            }
        }
    });

    // job-watch-dog (READY)
    let pool_watchdog = state.db.clone();
    let redis_watchdog = state.redis_client.clone();
    tokio::spawn(async move {
        watch_dog_job::run(pool_watchdog, redis_watchdog).await;
    });

    // job-terminator (TERMINATING)
    let pool_terminator = state.db.clone();
    let redis_terminator = state.redis_client.clone();
    tokio::spawn(async move {
        terminator_job::run(pool_terminator, redis_terminator).await;
    });

    // job-health-check (BOOTING)
    let db_health = state.db.clone();
    tokio::spawn(async move {
        health_check_job::run(db_health).await;
    });

    // job-provisioning (requeue PROVISIONING when pubsub events were missed)
    let db_prov = state.db.clone();
    let redis_prov = state.redis_client.clone();
    tokio::spawn(async move {
        provisioning_job::run(db_prov, redis_prov).await;
    });

    // job-recovery (recover stuck instances in various states)
    let db_recovery = state.db.clone();
    let redis_recovery = state.redis_client.clone();
    tokio::spawn(async move {
        recovery_job::run(db_recovery, redis_recovery).await;
    });

    // job-volume-reconciliation (reconcile volumes between DB and provider)
    let db_volume_reconciliation = state.db.clone();
    tokio::spawn(async move {
        volume_reconciliation_job::run(db_volume_reconciliation).await;
    });

    // 5. Start HTTP Server (Admin API - Simplified for internal health/debug only)
    let app = Router::new()
        .route("/", get(root))
        .route("/admin/status", get(get_status))
        .route("/internal/worker/register", post(worker_register))
        .route("/internal/worker/heartbeat", post(worker_heartbeat))
        // NO MORE PUBLIC API FOR INSTANCES
        // .route("/instances", get(list_instances))
        // .route("/instances/:id", axum::routing::delete(delete_instance_handler))
        .with_state(state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    println!("Orchestrator listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn root() -> &'static str {
    "Inventiv Orchestrator Online (Postgres Backed)"
}

#[derive(serde::Deserialize, Debug)]
struct CommandTerminate {
    instance_id: String,
    correlation_id: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct CommandReinstall {
    instance_id: String,
    correlation_id: Option<String>,
}

// DELETED HANDLERS (Moved to services.rs)

async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM instances WHERE status != 'terminated'")
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

    Json(json!({
        "cloud_instances_count": count,
        "message": "Full details available via GET /instances"
    }))
    .into_response()
}

async fn worker_register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ConnectInfo(connect): ConnectInfo<SocketAddr>,
    Json(payload): Json<WorkerRegisterRequest>,
) -> impl IntoResponse {
    let client_ip = request_client_ip(&headers, &connect);

    // Either:
    // - authenticated (existing token or global token), OR
    // - bootstrap (no token yet + IP matches instance/ip) -> issue token and return it.
    let authed = verify_worker_auth(&state.db, &headers, payload.instance_id).await;
    let mut issued_token: Option<(String, String)> = None;
    if !authed {
        let can_bootstrap =
            instance_bootstrap_allowed(&state.db, payload.instance_id, &client_ip).await;
        if !can_bootstrap {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "unauthorized"})),
            )
                .into_response();
        }
        issued_token = issue_worker_token(
            &state.db,
            payload.instance_id,
            payload.worker_id,
            payload.metadata.clone(),
        )
        .await;
        if issued_token.is_none() {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error":"token_already_exists_or_race"})),
            )
                .into_response();
        }
    }

    println!(
        "🧩 [Worker] REGISTER: instance_id={} worker_id={:?} model_id={:?} health_port={:?} vllm_port={:?} ip={:?} client_ip={}",
        payload.instance_id, payload.worker_id, payload.model_id, payload.health_port, payload.vllm_port, payload.ip_address, client_ip
    );

    // Log payload summary for debugging
    let payload_summary = json!({
        "instance_id": payload.instance_id,
        "worker_id": payload.worker_id,
        "model_id": payload.model_id,
        "health_port": payload.health_port,
        "vllm_port": payload.vllm_port,
        "ip_address": payload.ip_address,
        "has_metadata": payload.metadata.is_some()
    });
    println!(
        "🔵 [Worker] REGISTER payload summary: {}",
        serde_json::to_string(&payload_summary).unwrap_or_default()
    );

    let res = sqlx::query(
        r#"
        UPDATE instances
        SET worker_status = COALESCE(worker_status, 'starting'),
            worker_model_id = COALESCE($2, worker_model_id),
            worker_vllm_port = COALESCE($3, worker_vllm_port),
            worker_health_port = COALESCE($4, worker_health_port),
            ip_address = CASE
              WHEN ip_address IS NULL AND $5 IS NOT NULL AND btrim($5) <> '' THEN $5::inet
              ELSE ip_address
            END,
            worker_metadata = COALESCE($6, worker_metadata),
            worker_last_heartbeat = NOW(),
            -- Generic recovery: if a worker shows up after we timed out, allow the instance to recover.
            status = CASE
              WHEN status = 'startup_failed' AND error_code = 'STARTUP_TIMEOUT' THEN 'booting'
              ELSE status
            END,
            boot_started_at = CASE
              WHEN (status = 'booting' AND (boot_started_at IS NULL OR error_code = 'WAITING_FOR_WORKER_HEARTBEAT')) THEN NOW()
              WHEN (status = 'startup_failed' AND error_code = 'STARTUP_TIMEOUT') THEN NOW()
              ELSE boot_started_at
            END,
            error_code = CASE
              WHEN error_code IN ('WAITING_FOR_WORKER_HEARTBEAT', 'STARTUP_TIMEOUT') THEN NULL
              ELSE error_code
            END,
            error_message = CASE
              WHEN error_code IN ('WAITING_FOR_WORKER_HEARTBEAT', 'STARTUP_TIMEOUT') THEN NULL
              ELSE error_message
            END
        WHERE id = $1
        "#,
    )
    .bind(payload.instance_id)
    .bind(payload.model_id)
    .bind(payload.vllm_port)
    .bind(payload.health_port)
    .bind(payload.ip_address)
    .bind(payload.metadata)
    .execute(&state.db)
    .await;

    match res {
        Ok(r) if r.rows_affected() > 0 => {
            if let Some((token, token_prefix)) = issued_token {
                (
                    StatusCode::OK,
                    Json(json!({"status":"ok","bootstrap_token": token,"bootstrap_token_prefix": token_prefix})),
                )
                    .into_response()
            } else {
                (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
            }
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "instance_not_found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "message": e.to_string()})),
        )
            .into_response(),
    }
}

async fn worker_heartbeat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<WorkerHeartbeatRequest>,
) -> impl IntoResponse {
    if !verify_worker_auth(&state.db, &headers, payload.instance_id).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response();
    }

    let status = payload.status.to_ascii_lowercase();

    // Log agent info if present
    if let Some(agent_info) = &payload.agent_info {
        println!(
            "📦 [Worker] AGENT INFO: instance_id={} version={:?} build_date={:?} checksum={:?}",
            payload.instance_id,
            agent_info.version,
            agent_info.build_date,
            agent_info.checksum.as_ref().map(|s| &s[..16]) // Log first 16 chars of checksum
        );
    }

    println!(
        "💓 [Worker] HEARTBEAT: instance_id={} worker_id={:?} status={} model_id={:?} gpu_util={:?} queue_depth={:?} ip={:?}",
        payload.instance_id, payload.worker_id, status, payload.model_id, payload.gpu_utilization, payload.queue_depth, payload.ip_address
    );

    // Log essential payload fields for debugging
    let payload_summary = json!({
        "instance_id": payload.instance_id,
        "worker_id": payload.worker_id,
        "status": status,
        "model_id": payload.model_id,
        "gpu_utilization": payload.gpu_utilization,
        "gpu_mem_used_mb": payload.gpu_mem_used_mb,
        "queue_depth": payload.queue_depth,
        "ip_address": payload.ip_address,
        "agent_info": payload.agent_info,
        "has_metadata": payload.metadata.is_some()
    });
    println!(
        "🔵 [Worker] HEARTBEAT payload summary: {}",
        serde_json::to_string(&payload_summary).unwrap_or_default()
    );

    // We'll need metadata both for persistence on the instance row and for time-series sampling.
    let meta_clone = payload.metadata.clone();

    // Merge agent_info into metadata for storage
    let mut enriched_metadata = meta_clone.clone().unwrap_or(json!({}));
    if let Some(agent_info) = &payload.agent_info {
        enriched_metadata["agent_info"] = json!({
            "version": agent_info.version.clone(),
            "build_date": agent_info.build_date.clone(),
            "checksum": agent_info.checksum.clone(),
        });
    }

    let res = sqlx::query(
        r#"
        UPDATE instances
        SET worker_last_heartbeat = NOW(),
            worker_status = $2,
            worker_model_id = COALESCE($3, worker_model_id),
            worker_queue_depth = COALESCE($4, worker_queue_depth),
            worker_gpu_utilization = COALESCE($5, worker_gpu_utilization),
            ip_address = CASE
              WHEN ip_address IS NULL AND $6 IS NOT NULL AND btrim($6) <> '' THEN $6::inet
              ELSE ip_address
            END,
            worker_metadata = COALESCE($7, worker_metadata),
            -- Generic recovery: late heartbeats should be able to recover from startup timeouts.
            status = CASE
              WHEN status = 'startup_failed' AND error_code = 'STARTUP_TIMEOUT' THEN 'booting'
              ELSE status
            END,
            boot_started_at = CASE
              WHEN (status = 'booting' AND (boot_started_at IS NULL OR error_code = 'WAITING_FOR_WORKER_HEARTBEAT')) THEN NOW()
              WHEN (status = 'startup_failed' AND error_code = 'STARTUP_TIMEOUT') THEN NOW()
              ELSE boot_started_at
            END,
            error_code = CASE
              WHEN error_code IN ('WAITING_FOR_WORKER_HEARTBEAT', 'STARTUP_TIMEOUT') THEN NULL
              ELSE error_code
            END,
            error_message = CASE
              WHEN error_code IN ('WAITING_FOR_WORKER_HEARTBEAT', 'STARTUP_TIMEOUT') THEN NULL
              ELSE error_message
            END
        WHERE id = $1
        "#,
    )
    .bind(payload.instance_id)
    .bind(status)
    .bind(payload.model_id)
    .bind(payload.queue_depth)
    .bind(payload.gpu_utilization)
    .bind(payload.ip_address.clone())
    .bind(meta_clone.clone())
    .execute(&state.db)
    .await;

    // Insert time series GPU samples (nvtop-like dashboard).
    // Prefer per-GPU list in metadata.gpus, fallback to aggregate fields.
    // Best-effort only: do not fail heartbeat on metrics insert.
    // Helper function to validate and clamp GPU metrics.
    let validate_gpu_util = |v: Option<f64>| v.map(|x| x.clamp(0.0, 100.0));
    let validate_temp = |v: Option<f64>| {
        v.and_then(|x| {
            // Accept temperatures between -50°C and 150°C (reasonable range for GPUs)
            if x >= -50.0 && x <= 150.0 {
                Some(x)
            } else {
                eprintln!(
                    "⚠️ Invalid GPU temperature for instance {}: {}°C (clamping to valid range)",
                    payload.instance_id, x
                );
                Some(x.clamp(-50.0, 150.0))
            }
        })
    };
    let validate_power = |v: Option<f64>| v.map(|x| x.max(0.0)); // Power cannot be negative
    let validate_vram = |v: Option<f64>| v.map(|x| x.max(0.0)); // VRAM cannot be negative

    if let Some(meta) = meta_clone.as_ref() {
        if let Some(gpus) = meta.get("gpus").and_then(|v| v.as_array()) {
            for g in gpus {
                let idx = g.get("index").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let util = validate_gpu_util(g.get("gpu_utilization").and_then(|v| v.as_f64()));
                let used = validate_vram(g.get("gpu_mem_used_mb").and_then(|v| v.as_f64()));
                let total = validate_vram(g.get("gpu_mem_total_mb").and_then(|v| v.as_f64()));
                let temp = validate_temp(g.get("gpu_temp_c").and_then(|v| v.as_f64()));
                let power = validate_power(g.get("gpu_power_w").and_then(|v| v.as_f64()));
                let power_limit =
                    validate_power(g.get("gpu_power_limit_w").and_then(|v| v.as_f64()));

                // Validate VRAM used <= total if both are present
                let used_final = match (used, total) {
                    (Some(u), Some(t)) if u > t => {
                        eprintln!(
                            "⚠️ GPU {} VRAM used ({:.1}MB) > total ({:.1}MB) for instance {}, clamping",
                            idx, u, t, payload.instance_id
                        );
                        Some(t)
                    }
                    _ => used,
                };

                if let Err(e) = sqlx::query(
                    r#"
                    INSERT INTO gpu_samples (instance_id, gpu_index, gpu_utilization, vram_used_mb, vram_total_mb, temp_c, power_w, power_limit_w)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    "#,
                )
                .bind(payload.instance_id)
                .bind(idx)
                .bind(util)
                .bind(used_final)
                .bind(total)
                .bind(temp)
                .bind(power)
                .bind(power_limit)
                .execute(&state.db)
                .await
                {
                    eprintln!(
                        "⚠️ Failed to insert gpu_samples for instance {} GPU {}: {}",
                        payload.instance_id, idx, e
                    );
                }
            }
        } else {
            // Fallback aggregate
            let total = validate_vram(meta.get("gpu_mem_total_mb").and_then(|v| v.as_f64()));
            let temp = validate_temp(meta.get("gpu_temp_c").and_then(|v| v.as_f64()));
            let power = validate_power(meta.get("gpu_power_w").and_then(|v| v.as_f64()));
            let power_limit =
                validate_power(meta.get("gpu_power_limit_w").and_then(|v| v.as_f64()));
            let util = validate_gpu_util(payload.gpu_utilization);
            let used = validate_vram(payload.gpu_mem_used_mb);

            // Validate VRAM used <= total if both are present
            let used_final = match (used, total) {
                (Some(u), Some(t)) if u > t => {
                    eprintln!(
                        "⚠️ GPU aggregate VRAM used ({:.1}MB) > total ({:.1}MB) for instance {}, clamping",
                        u, t, payload.instance_id
                    );
                    Some(t)
                }
                _ => used,
            };

            if let Err(e) = sqlx::query(
                r#"
                INSERT INTO gpu_samples (instance_id, gpu_index, gpu_utilization, vram_used_mb, vram_total_mb, temp_c, power_w, power_limit_w)
                VALUES ($1, 0, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(payload.instance_id)
            .bind(util)
            .bind(used_final)
            .bind(total)
            .bind(temp)
            .bind(power)
            .bind(power_limit)
            .execute(&state.db)
            .await
            {
                eprintln!(
                    "⚠️ Failed to insert gpu_samples (aggregate) for instance {}: {}",
                    payload.instance_id, e
                );
            }
        }
    } else {
        // No metadata, use payload fields only
        let util = validate_gpu_util(payload.gpu_utilization);
        let used = validate_vram(payload.gpu_mem_used_mb);

        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO gpu_samples (instance_id, gpu_index, gpu_utilization, vram_used_mb, vram_total_mb, temp_c, power_w, power_limit_w)
            VALUES ($1, 0, $2, $3, NULL, NULL, NULL, NULL)
            "#,
        )
        .bind(payload.instance_id)
        .bind(util)
        .bind(used)
        .execute(&state.db)
        .await
        {
            eprintln!(
                "⚠️ Failed to insert gpu_samples (minimal) for instance {}: {}",
                payload.instance_id, e
            );
        }
    }

    // Insert time series system samples (CPU/Mem/Disk/Network) from metadata.system.
    // Best-effort only: do not fail heartbeat on metrics insert.
    // Helper functions to validate system metrics.
    let validate_cpu_pct = |v: Option<f64>| v.map(|x| x.clamp(0.0, 100.0));
    let validate_load = |v: Option<f64>| v.map(|x| x.max(0.0)); // Load cannot be negative
    let validate_bytes = |v: Option<i64>| v.map(|x| x.max(0)); // Bytes cannot be negative
    let validate_bps = |v: Option<f64>| v.map(|x| x.max(0.0)); // Network rates cannot be negative

    if let Some(meta) = meta_clone.as_ref() {
        if let Some(sys) = meta.get("system") {
            let cpu = validate_cpu_pct(sys.get("cpu_usage_pct").and_then(|v| v.as_f64()));
            let load1 = validate_load(sys.get("load1").and_then(|v| v.as_f64()));
            let mem_used = validate_bytes(sys.get("mem_used_bytes").and_then(|v| v.as_i64()));
            let mem_total = validate_bytes(sys.get("mem_total_bytes").and_then(|v| v.as_i64()));
            let disk_used = validate_bytes(sys.get("disk_used_bytes").and_then(|v| v.as_i64()));
            let disk_total = validate_bytes(sys.get("disk_total_bytes").and_then(|v| v.as_i64()));
            let rx_bps = validate_bps(sys.get("net_rx_bps").and_then(|v| v.as_f64()));
            let tx_bps = validate_bps(sys.get("net_tx_bps").and_then(|v| v.as_f64()));

            // Validate used <= total for memory and disk
            let mem_used_final = match (mem_used, mem_total) {
                (Some(u), Some(t)) if u > t => {
                    eprintln!(
                        "⚠️ Memory used ({}) > total ({}) for instance {}, clamping",
                        u, t, payload.instance_id
                    );
                    Some(t)
                }
                _ => mem_used,
            };
            let disk_used_final = match (disk_used, disk_total) {
                (Some(u), Some(t)) if u > t => {
                    eprintln!(
                        "⚠️ Disk used ({}) > total ({}) for instance {}, clamping",
                        u, t, payload.instance_id
                    );
                    Some(t)
                }
                _ => disk_used,
            };

            // Insert only when at least one meaningful field is present.
            if cpu.is_some()
                || load1.is_some()
                || mem_used_final.is_some()
                || mem_total.is_some()
                || disk_used_final.is_some()
                || disk_total.is_some()
                || rx_bps.is_some()
                || tx_bps.is_some()
            {
                if let Err(e) = sqlx::query(
                    r#"
                    INSERT INTO system_samples (
                      instance_id,
                      cpu_usage_pct,
                      load1,
                      mem_used_bytes,
                      mem_total_bytes,
                      disk_used_bytes,
                      disk_total_bytes,
                      net_rx_bps,
                      net_tx_bps
                    )
                    VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                    "#,
                )
                .bind(payload.instance_id)
                .bind(cpu)
                .bind(load1)
                .bind(mem_used_final)
                .bind(mem_total)
                .bind(disk_used_final)
                .bind(disk_total)
                .bind(rx_bps)
                .bind(tx_bps)
                .execute(&state.db)
                .await
                {
                    eprintln!(
                        "⚠️ Failed to insert system_samples for instance {}: {}",
                        payload.instance_id, e
                    );
                }
            }
        }
    }

    match res {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "instance_not_found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "db_error", "message": e.to_string()})),
        )
            .into_response(),
    }
}

async fn scaling_engine_loop(state: Arc<AppState>) {
    println!("Scaling Engine Started");
    loop {
        sleep(Duration::from_secs(60)).await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM instances")
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);
        println!("Scaler Heartbeat: {} total instances managed.", count);
    }
}
