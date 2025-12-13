use sqlx::{Pool, Postgres};
use uuid::Uuid;

/// Simple action logger using query() instead of query!() to avoid DATABASE_URL at build time
pub async fn log_action(
    db: &Pool<Postgres>,
    action_type: &str,
    status: &str,
    instance_id: Option<Uuid>,
    error_message: Option<&str>,
) -> Result<Uuid, sqlx::Error> {
    log_action_with_metadata(db, action_type, status, instance_id, error_message, None).await
}

/// Log action with metadata (context info)
pub async fn log_action_with_metadata(
    db: &Pool<Postgres>,
    action_type: &str,
    status: &str,
    instance_id: Option<Uuid>,
    error_message: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<Uuid, sqlx::Error> {
    let log_id = Uuid::new_v4();
    
    sqlx::query(
        "INSERT INTO action_logs 
         (id, action_type, component, status, error_message, instance_id, metadata, created_at) 
         VALUES ($1, $2, 'api', $3, $4, $5, $6, NOW())"
    )
    .bind(log_id)
    .bind(action_type)
    .bind(status)
    .bind(error_message)
    .bind(instance_id)
    .bind(metadata)
    .execute(db)
    .await?;
    
    println!("📝 [API] Logged: {} - {} ({})", action_type, status, log_id);
    Ok(log_id)
}

/// Log action completion with duration
pub async fn log_action_complete(
    db: &Pool<Postgres>,
    log_id: Uuid,
    status: &str,
    duration_ms: i32,
    error_message: Option<&str>,
) -> Result<(), sqlx::Error> {
    log_action_complete_with_metadata(db, log_id, status, duration_ms, error_message, None).await
}

/// Log action completion with metadata
pub async fn log_action_complete_with_metadata(
    db: &Pool<Postgres>,
    log_id: Uuid,
    status: &str,
    duration_ms: i32,
    error_message: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE action_logs 
         SET status = $2, duration_ms = $3, error_message = $4, metadata = COALESCE($5, metadata), completed_at = NOW()
         WHERE id = $1"
    )
    .bind(log_id)
    .bind(status)
    .bind(duration_ms)
    .bind(error_message)
    .bind(metadata)
    .execute(db)
    .await?;
    
    Ok(())
}
