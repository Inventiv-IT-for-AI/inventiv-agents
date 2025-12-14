use axum::{
    extract::{State},
    response::{IntoResponse, Json},
    routing::{get},
    Router,
};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
mod provider;
mod provider_manager; // NEW
mod providers; // NEW
mod models;
mod logger;
mod health_check_job;
mod terminator_job;
mod watch_dog_job;
mod services; // NEW
mod finops_events;
use tokio::time::{sleep, Duration};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};


struct AppState {
    db: Pool<Postgres>,
    redis_client: redis::Client,
}

#[derive(serde::Deserialize, Debug)]
struct CommandProvision {
    instance_id: String,
    zone: String,
    instance_type: String,
    correlation_id: Option<String>,
}

mod migrations; // NEW
mod state_machine;
mod health_check_flow;

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
                        if let Ok(cmd) = serde_json::from_value::<CommandProvision>(event_json.clone()) {
                            let pool = state_redis.db.clone();
                            let redis_client = state_redis.redis_client.clone();
                            tokio::spawn(async move {
                                services::process_provisioning(pool, redis_client, cmd.instance_id, cmd.zone, cmd.instance_type, cmd.correlation_id).await;
                            });
                        }
                    }
                    "CMD:TERMINATE" => {
                        if let Ok(cmd) = serde_json::from_value::<CommandTerminate>(event_json.clone()) {
                            let pool = state_redis.db.clone();
                            let redis_client = state_redis.redis_client.clone();
                            tokio::spawn(async move {
                                services::process_termination(pool, redis_client, cmd.instance_id, cmd.correlation_id).await;
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
    tokio::spawn(async move { watch_dog_job::run(pool_watchdog, redis_watchdog).await; });

    // job-terminator (TERMINATING)
    let pool_terminator = state.db.clone();
    let redis_terminator = state.redis_client.clone();
    tokio::spawn(async move { terminator_job::run(pool_terminator, redis_terminator).await; });

    // job-health-check (BOOTING)
    let db_health = state.db.clone();
    tokio::spawn(async move { health_check_job::run(db_health).await; });

    // 5. Start HTTP Server (Admin API - Simplified for internal health/debug only)
    let app = Router::new()
        .route("/", get(root))
        .route("/admin/status", get(get_status))
        // NO MORE PUBLIC API FOR INSTANCES
        // .route("/instances", get(list_instances))
        // .route("/instances/:id", axum::routing::delete(delete_instance_handler))
        .with_state(state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    println!("Orchestrator listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Inventiv Orchestrator Online (Postgres Backed)"
}

#[derive(serde::Deserialize, Debug)]
struct CommandTerminate {
    instance_id: String,
    correlation_id: Option<String>,
}

// DELETED HANDLERS (Moved to services.rs)

async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM instances WHERE status != 'terminated'")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    Json(json!({
        "cloud_instances_count": count,
        "message": "Full details available via GET /instances"
    })).into_response()
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


