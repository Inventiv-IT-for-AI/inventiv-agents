use axum::{
    extract::{State, Path},
    routing::{get, post, delete},
    Router, Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use redis::AsyncCommands;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

// Swagger
use utoipa::{OpenApi, IntoParams};
use utoipa_swagger_ui::SwaggerUi;
mod settings; // Module
mod api_docs;

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
    // Run Migrations (Manual for now due to macro path issues in Docker)
    /*
    let res: Result<(), sqlx::Error> = sqlx::migrate!("../migrations")
        .run(&pool)
        .await;
    
    if let Err(e) = res {
        panic!("Failed to run migrations: {}", e);
    }
    */

    let state = Arc::new(AppState {
        redis_client: client,
        db: pool,
    });

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_docs::ApiDoc::openapi()))
        .route("/", get(root))
        .route("/deployments", post(create_deployment))
        // NEW READ ENDPOINTS
        .route("/instances", get(list_instances))
        .route("/instances/:id/archive", axum::routing::put(archive_instance))
        .route("/instances/:id", delete(terminate_instance))
        // SETTINGS ENDPOINTS
        .route("/regions", get(settings::list_regions))
        .route("/regions/:id", axum::routing::put(settings::update_region))
        .route("/zones", get(settings::list_zones))
        .route("/zones/:id", axum::routing::put(settings::update_zone))
        .route("/instance_types", get(settings::list_instance_types))
        .route("/instance_types/:id", axum::routing::put(settings::update_instance_type))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8003));
    println!("Backend listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct InstanceResponse {
    pub id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub zone_id: uuid::Uuid,
    pub instance_type_id: uuid::Uuid,
    pub status: String,
    pub ip_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    // Joined Fields
    pub provider_name: String,
    pub region: String,
    pub zone: String,
    pub instance_type: String,
    pub gpu_vram: Option<i32>,
    pub cost_per_hour: Option<f64>, // NEW
    pub total_cost: Option<f64>,    // NEW
    pub is_archived: bool,          // NEW
}

#[derive(Deserialize, IntoParams)]
pub struct ListInstanceParams {
    pub archived: Option<bool>,
}

async fn root() -> &'static str {
    "Inventiv Backend API (Product Plane) - CQRS Enabled"
}

#[derive(Deserialize, utoipa::ToSchema)]
struct DeploymentRequest {
    zone: String,
    instance_type: String,
}

#[derive(Serialize, utoipa::ToSchema)]
struct DeploymentResponse {
    status: String,
    deployment_id: String,
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
) -> Json<DeploymentResponse> {
    let deployment_id = uuid::Uuid::new_v4().to_string();
    println!("🚀 New Deployment Request: {}", deployment_id);

    // Publish Event to Redis
    let event_payload = serde_json::json!({
        "type": "CMD:PROVISION",
        "deployment_id": deployment_id,
        "zone": payload.zone,
        "instance_type": payload.instance_type
    }).to_string();

    let mut conn = state.redis_client.get_multiplexed_async_connection().await.unwrap();
    let _: () = conn.publish("orchestrator_events", event_payload).await.unwrap();

    Json(DeploymentResponse {
        status: "accepted".to_string(),
        deployment_id,
    })
}

// QUERY : LIST INSTANCES
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
            cast(i.status as text) as status, 
            cast(i.ip_address as text) as ip_address, 
            i.created_at,
            i.is_archived,
            p.name as provider_name,
            z.name as zone,
            r.name as region,
            it.name as instance_type,
            it.vram_per_gpu_gb as gpu_vram,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            (EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8) as total_cost
        FROM instances i
        JOIN providers p ON i.provider_id = p.id
        JOIN zones z ON i.zone_id = z.id
        JOIN regions r ON z.region_id = r.id
        JOIN instance_types it ON i.instance_type_id = it.id
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

// COMMAND : ARCHIVE INSTANCE
#[utoipa::path(
    put,
    path = "/instances/{id}/archive",
    params(
        ("id" = Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 200, description = "Instance Archived")
    )
)]
async fn archive_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "UPDATE instances SET is_archived = true WHERE id = $1 AND status = 'terminated'"
    )
    .bind(id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => (StatusCode::OK, "Instance Archived"),
        Ok(_) => (StatusCode::BAD_REQUEST, "Instance not found or not terminated"),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database Error"),
    }
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
    println!("🗑️ Termination Request: {}", id);

    // Publish CMD:TERMINATE
    let event_payload = serde_json::json!({
        "type": "CMD:TERMINATE",
        "instance_id": id
    }).to_string();

    let mut conn = state.redis_client.get_multiplexed_async_connection().await.unwrap();
    let _: () = conn.publish("orchestrator_events", event_payload).await.unwrap();

    (StatusCode::ACCEPTED, "Termination Signal Sent")
}
