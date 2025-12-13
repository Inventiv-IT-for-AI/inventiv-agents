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
use crate::provider::CloudProvider;
use crate::provider_manager::ProviderManager; // NEW
mod providers; // NEW
mod models;
mod logger;
mod reconciliation;
mod services; // NEW
use tokio::time::{sleep, Duration};
use inventiv_common::{Instance, InstanceStatus}; // Keep imports
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

mod health_check;
use health_check::{check_and_transition_instance};

struct AppState {
    db: Pool<Postgres>,
}

#[derive(serde::Deserialize, Debug)]
struct CommandProvision {
    instance_id: String,
    zone: String,
    instance_type: String,
}

mod migrations; // NEW

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    
    // Connect to Postgres
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    // Check connection
    sqlx::query("SELECT 1").execute(&pool).await.unwrap();
    println!("✅ Connected to Database");

    // Run Migrations
    migrations::run_inline_migrations(&pool).await;

    let state = Arc::new(AppState {
        db: pool,
    });



    // 3. Start Scaling Engine Loop (Background Task)
    let state_clone = state.clone();
    tokio::spawn(async move {
        scaling_engine_loop(state_clone).await;
    });

    // 4. Start Event Listener (Redis Subscriber)
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let client = redis::Client::open(redis_url.clone()).unwrap();
    
    // Use dedicated PubSub connection
    let mut pubsub = client.get_async_pubsub().await.unwrap();
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
                            tokio::spawn(async move {
                                services::process_provisioning(pool, cmd.instance_id, cmd.zone, cmd.instance_type).await;
                            });
                        }
                    }
                    "CMD:TERMINATE" => {
                        if let Ok(cmd) = serde_json::from_value::<CommandTerminate>(event_json.clone()) {
                            let pool = state_redis.db.clone();
                            tokio::spawn(async move {
                                services::process_termination(pool, cmd.instance_id).await;
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

    // Background reconciliation task - runs every 10 seconds
    // but only checks instances not reconciled in last 60 seconds (smart filtering)
    let pool_reconcile = state.db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));

        loop {
            interval.tick().await;
            
            // Check Scaleway Provider
            if let Some(provider) = ProviderManager::get_provider("scaleway") {
                match crate::reconciliation::reconcile_instances(&pool_reconcile, provider.as_ref()).await {
                    Ok(count) if count > 0 => {
                        println!("🔴 [Auto-Reconciliation] {} orphaned instances detected", count);
                    }
                    Ok(_) => {}, // Normal
                    Err(e) => eprintln!("❌ [Auto-Reconciliation] {:?}", e),
                }
            } else {
                // Provider not configured, skipping
            }
        }
    });

    // 4. Spawn Health Check Monitor (Background Task)
    let db_health = state.db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        println!("🏥 Health Check Monitor Started (checking every 10s)");
        
        loop {
            interval.tick().await;
            
            // Find all instances in 'booting' state with IP addresses
            let booting_instances: Result<Vec<_>, _> = sqlx::query!(
                "SELECT id, ip_address::text as ip, created_at, health_check_failures 
                 FROM instances 
                 WHERE status = 'booting' AND ip_address IS NOT NULL"
            )
            .fetch_all(&db_health)
            .await;
            
            match booting_instances {
                Ok(instances) if !instances.is_empty() => {
                    println!("🔍 Checking {} booting instance(s)...", instances.len());
                    
                    for instance in instances {
                        let db_clone = db_health.clone();
                        tokio::spawn(async move {
                            check_and_transition_instance(
                    instance.id,
                    instance.ip,
                    instance.created_at.unwrap_or_else(|| sqlx::types::chrono::Utc::now()),
                    instance.health_check_failures.unwrap_or(0),
                    db_clone
                ).await;
                        });
                    }
                }
                Ok(_) => {
                    // No booting instances, silent
                }
                Err(e) => {
                    println!("⚠️  Health check query error: {:?}", e);
                }
            }
        }
    });

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


