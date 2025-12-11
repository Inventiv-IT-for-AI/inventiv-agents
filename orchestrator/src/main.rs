use axum::{
    extract::{State},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use inventiv_common::{CloudInstance, InstanceStatus, GPUProfile};
use std::collections::HashMap;
use tokio::sync::Mutex;

// -- Mock Database / State --
struct AppState {
    // In real app: Use SQLx Pool
    // db: PgPool,
    
    // In-memory state for MVP Rust
    instances: Mutex<HashMap<String, CloudInstance>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = Arc::new(AppState {
        instances: Mutex::new(HashMap::new()),
    });

    // Start Scaling Engine Loop
    let scaling_state = state.clone();
    tokio::spawn(async move {
        scaling_engine_loop(scaling_state).await;
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/admin/status", get(get_status))
        .route("/models", post(register_model)) // Placeholder
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8001));
    println!("Orchestrator listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Orchestrator Online (Rust)"
}

async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let instances = state.instances.lock().await;
    // Return status generic structure compatible with what Router expects
    Json(json!({
        "cloud_instances_count": instances.len(),
        "details": instances.values().cloned().collect::<Vec<_>>()
    }))
}

async fn register_model() -> impl IntoResponse {
    // TODO: Implement Model Registry in DB
    Json(json!({"status": "registered"}))
}

// -- Scaling Engine --

async fn scaling_engine_loop(state: Arc<AppState>) {
    println!("Scaling Engine Started");
    loop {
        sleep(Duration::from_secs(10)).await;
        // Logic: Check DB/Redis queue -> Call Provider -> Update State
        // For MVP: Do nothing or simulate auto-healing
        // check_queue_and_scale(&state).await;
    }
}
