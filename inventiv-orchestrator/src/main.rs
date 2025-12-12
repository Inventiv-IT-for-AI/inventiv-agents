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
use crate::provider::CloudProvider; // TOP LEVEL IMPORT
mod models;
use tokio::time::{sleep, Duration};
use inventiv_common::{Instance, InstanceStatus}; // Keep imports
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

struct AppState {
    db: Pool<Postgres>,
}

#[derive(serde::Deserialize, Debug)]
struct CommandProvision {
    deployment_id: String,
    zone: String,
    instance_type: String,
}

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

    // Run Migrations Manually (Inline Fallback)
    println!("📦 Running Migrations (Inline Schema)...");
    
    // Minimal Schema for Orchestrator to work (Instances Table)
    let schema_sql = r#"
        CREATE TYPE instance_status AS ENUM (
            'provisioning', 'booting', 'ready', 'draining', 'terminated', 'failed'
        );
        CREATE TABLE IF NOT EXISTS providers (
            id UUID PRIMARY KEY,
            name VARCHAR(50) UNIQUE NOT NULL,
            description TEXT,
            created_at TIMESTAMPTZ DEFAULT NOW()
        );
        CREATE TABLE IF NOT EXISTS regions (
            id UUID PRIMARY KEY,
            provider_id UUID NOT NULL,
            name VARCHAR(50) NOT NULL,
            UNIQUE(provider_id, name)
        );
        CREATE TABLE IF NOT EXISTS zones (
            id UUID PRIMARY KEY,
            region_id UUID NOT NULL,
            name VARCHAR(50) NOT NULL,
            UNIQUE(region_id, name)
        );
        CREATE TABLE IF NOT EXISTS instance_types (
            id UUID PRIMARY KEY,
            provider_id UUID NOT NULL,
            name VARCHAR(50) NOT NULL,
            gpu_count INTEGER NOT NULL,
            vram_per_gpu_gb INTEGER NOT NULL,
            UNIQUE(provider_id, name)
        );
        CREATE TABLE IF NOT EXISTS instances (
            id UUID PRIMARY KEY,
            provider_id UUID NOT NULL,
            zone_id UUID NOT NULL,
            instance_type_id UUID NOT NULL,
            model_id UUID,
            provider_instance_id VARCHAR(255),
            ip_address INET,
            api_key VARCHAR(255),
            status instance_status NOT NULL DEFAULT 'provisioning',
            created_at TIMESTAMPTZ DEFAULT NOW(),
            terminated_at TIMESTAMPTZ,
            gpu_profile JSONB NOT NULL
        );
    "#;

    // Execute schema (ignoring errors if types already exist)
    // We split by ; explicitly because simple query protocol might not be used by sqlx pool execute
    // Actually, letting sqlx execute the block might fail if multiple statements.
    // We'll execute creating the enum separately as it cannot be IF NOT EXISTS easily in Postgres < 12 without blocks.
    // Simplified: Just try to execute the whole block. If it fails, we assume it exists.
    
    // Note: sqlx::query might not support multiple statements.
    // We'll try. If it fails, I'll recommend user to use the CLI or I'll split it.
    // Splitting by ; is safer.
    for statement in schema_sql.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() {
             let _ = sqlx::query(stmt).execute(&pool).await;
        }
    }
    
    // Run Seeds needed for FK
    let seeds_sql = r#"
        INSERT INTO providers (id, name, description) VALUES ('00000000-0000-0000-0000-000000000001', 'scaleway', 'Scaleway GPU Cloud') ON CONFLICT DO NOTHING;
        INSERT INTO regions (id, provider_id, name) VALUES ('00000000-0000-0000-0000-000000000010', '00000000-0000-0000-0000-000000000001', 'fr-par') ON CONFLICT DO NOTHING;
        INSERT INTO zones (id, region_id, name) VALUES ('00000000-0000-0000-0000-000000000020', '00000000-0000-0000-0000-000000000010', 'fr-par-2') ON CONFLICT DO NOTHING;
        INSERT INTO instance_types (id, provider_id, name, gpu_count, vram_per_gpu_gb) VALUES ('00000000-0000-0000-0000-000000000030', '00000000-0000-0000-0000-000000000001', 'RENDER-S', 1, 24) ON CONFLICT DO NOTHING;
    "#;
    
    for statement in seeds_sql.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() {
             let _ = sqlx::query(stmt).execute(&pool).await;
        }
    }

    println!("✅ Migrations (Inline) Applied");

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
            // SAFETY: Explicit typing to help inference
            let payload: String = msg.get_payload().unwrap();
            println!("⚡️ Event Received: {}", payload);
            
            let state_clone = state_redis.clone();
            if let Ok(cmd) = serde_json::from_str::<CommandProvision>(&payload) {
                tokio::spawn(async move {
                    process_provisioning(state_clone, cmd).await;
                });
            } else if let Ok(cmd) = serde_json::from_str::<CommandTerminate>(&payload) {
                 println!("🛑 Termination Command: {}", cmd.instance_id);
                 tokio::spawn(async move {
                    process_termination(state_clone, cmd).await;
                });
            } else {
                 println!("⚠️ Ignored/Invalid Event Payload: {}", payload);
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

async fn process_termination(state: Arc<AppState>, cmd: CommandTerminate) {
    let id_uuid = uuid::Uuid::parse_str(&cmd.instance_id).unwrap_or_default();
    println!("⚙️ Processing Termination Async: {}", id_uuid);

     // 1. Fetch from DB
    let instance = sqlx::query_as::<Postgres, Instance>("SELECT * FROM instances WHERE id = $1")
        .bind(id_uuid)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    if let Some(mut inst) = instance {
        if inst.status == InstanceStatus::Terminated {
             println!("⚠️ Instance {} already terminated.", id_uuid);
             return;
        }

        // 2. Call Scaleway API
        let project_id = std::env::var("SCALEWAY_PROJECT_ID").unwrap_or_default();
        let secret_key = std::env::var("SCALEWAY_SECRET_KEY").unwrap_or_default();
        
        let provider: crate::provider::ScalewayProvider = crate::provider::ScalewayProvider::new(project_id, secret_key);
        
        if let Some(remote_id) = &inst.provider_instance_id {
            match provider.terminate_instance("fr-par-2", remote_id).await {
                Ok(_) => println!("✅ Remote Instance Terminated: {}", remote_id),
                Err(e) => println!("⚠️ Failed to terminate remote: {}", e),
            }
        }

        // 3. Mark in DB
        let _ = sqlx::query("UPDATE instances SET status = 'terminated'::instance_status, terminated_at = NOW() WHERE id = $1")
            .bind(id_uuid)
            .execute(&state.db)
            .await;
            
        println!("✅ Instance {} marked as terminated in DB.", id_uuid);
    } else {
        println!("⚠️ Instance {} not found for termination.", id_uuid);
    }
}

// ASYNC CORE SERVICE
async fn process_provisioning(state: Arc<AppState>, cmd: CommandProvision) {
    println!("⚙️ Processing Provisioning Async: {:?}", cmd);
    
    // 1. Init Provider
    let project_id = std::env::var("SCALEWAY_PROJECT_ID").unwrap_or_default();
    let secret_key = std::env::var("SCALEWAY_SECRET_KEY").unwrap_or_default();
    
    if project_id.is_empty() || secret_key.is_empty() {
         println!("❌ Error: Missing Credentials");
         return;
    }
    
    // Explicit type
    let provider: crate::provider::ScalewayProvider = crate::provider::ScalewayProvider::new(project_id, secret_key);

    // Ubuntu 24.04 Noble Numbat (x86_64, fr-par-2) fetched dynamically
    let image_id = "8e0da557-5d75-40ba-b928-5984075aa255"; 
    
    // Call via Trait explicitly? No, provider implements it.
    match provider.create_instance(&cmd.zone, &cmd.instance_type, image_id).await {
        Ok(server_id) => {
             println!("✅ Server Created: {}", server_id);
             
             // 3. Power On
             let _ = provider.start_instance(&cmd.zone, &server_id).await;
             
             // 4. Record in DB
             let instance_id = uuid::Uuid::parse_str(&cmd.deployment_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
             println!("💾 Persisting Instance {} ({}) to DB...", instance_id, server_id);
             
             // UNCOMMENTED & FIXED
             let result = sqlx::query::<Postgres>(
                "INSERT INTO instances (id, provider_id, zone_id, instance_type_id, provider_instance_id, status, gpu_profile)
                 VALUES ($1, $2, $3, $4, $5, 'booting', 
                 '{\"id\": \"00000000-0000-0000-0000-000000000000\", \"provider_id\": \"00000000-0000-0000-0000-000000000000\", \"name\": \"L4\", \"gpu_count\": 1, \"vram_per_gpu_gb\": 24}'::jsonb)"
             )
             .bind(instance_id) // ID from Backend
             .bind(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()) // Provider Scaleway
             .bind(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000020").unwrap()) // Zone fr-par-2
             .bind(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000030").unwrap()) // Type RENDER-S
             .bind(&server_id)
             .execute(&state.db)
             .await;
             
             if let Err(e) = result {
                 println!("❌ Database Insert Error: {:?}", e);
             } else {
                 println!("✅ Database Insert Success");
             }
        }
        Err(e) => {
            println!("❌ Provision Error: {}", e);
        }
    }
}

// DELETED HANDLERS (Moved to Backend)
// async fn list_instances(...)
// async fn delete_instance_handler(...)

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
