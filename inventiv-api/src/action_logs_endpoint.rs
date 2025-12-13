
// ============================================================================
// ACTION LOGS API
// ============================================================================

use axum::extract::Query;

#[derive(Deserialize, IntoParams)]
struct ActionLogQuery {
    instance_id: Option<uuid::Uuid>,
    component: Option<String>,
    status: Option<String>,
    limit: Option<i32>,
}

#[derive(Serialize, sqlx::FromRow)]
struct ActionLogResponse {
    id: uuid::Uuid,
    action_type: String,
    component: String,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
    instance_id: Option<uuid::Uuid>,
    duration_ms: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    // Instance details (joined)
    instance_provider_instance_id: Option<String>,
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
    
    let logs = sqlx::query_as!(
        ActionLogResponse,
        r#"SELECT 
            al.id, al.action_type, al.component, al.status, 
            al.error_code, al.error_message, al.instance_id, al.duration_ms,
            al.created_at, al.completed_at,
            i.provider_instance_id as instance_provider_instance_id
         FROM action_logs al
         LEFT JOIN instances i ON al.instance_id = i.id
         WHERE ($1::uuid IS NULL OR al.instance_id = $1)
           AND ($2::text IS NULL OR al.component = $2)
           AND ($3::text IS NULL OR al.status = $3)
         ORDER BY al.created_at DESC
         LIMIT $4"#,
        params.instance_id,
        params.component,
        params.status,
        limit as i64
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    
    Json(logs)
}
