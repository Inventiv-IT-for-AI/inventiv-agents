use axum::{
    extract::{Json, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    http_client: Client,
    orchestrator_url: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let client = Client::new();
    let orchestrator_url = std::env::var("ORCHESTRATOR_URL").unwrap_or("http://localhost:8001".to_string());

    let state = Arc::new(AppState {
        http_client: client,
        orchestrator_url,
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/v1/chat/completions", post(proxy_chat_completions))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8002));
    println!("Router listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}

// Simple MVP Proxy: Ask Orchestrator for an instance, then forward.
// In Real World: Cache instances in memory/Redis.
async fn proxy_chat_completions(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // 1. Get Model ID
    let model_id = payload.get("model").and_then(|v| v.as_str());

    if model_id.is_none() {
        return (StatusCode::BAD_REQUEST, "Missing model field").into_response();
    }

    // 2. Find Instance (Mock call to orchestrator admin API or just assume local worker for MVP)
    // For this Rust MVP step, let's assume a worker is running at http://localhost:8000 (vLLM default)
    // or try to fetch from orchestrator.
    // Ideally we implemented /admin/status in orchestrator, we could call it.
    
    // Hardcoded logic for MVP to demonstrate Rust Proxying:
    // Try to forward to localhost:8000 (the worker container mapped port)
    // NOTE: In docker-compose, worker is distinct. We should use standard service discovery or orchestrator lookup.
    
    // Let's implement robust proxying to a target URL
    // target_url should come from load balancing logic.
    let target_url = "http://localhost:8000/v1/chat/completions"; // Local test
    // If running in docker, might be http://worker:8000/v1/chat/completions

    // 3. Proxy Request (Streaming supported)
    // We send the request exactly as received
    let resp = state
        .http_client
        .post(target_url)
        .json(&payload)
        .send()
        .await;

    match resp {
        Ok(response) => {
            let status = response.status();
            // We want to stream the body back
            let body = axum::body::Body::from_stream(response.bytes_stream());
            
            Response::builder()
                .status(status)
                .body(body)
                .unwrap()
        }
        Err(e) => {
            println!("Proxy error: {}", e);
            (StatusCode::BAD_GATEWAY, "Worker unavailable").into_response()
        }
    }
}
