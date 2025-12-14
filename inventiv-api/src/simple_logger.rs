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

    // Capture instance status at action start (if instance exists)
    let before_status: Option<String> = if let Some(iid) = instance_id {
        sqlx::query_scalar("SELECT status::text FROM instances WHERE id = $1")
            .bind(iid)
            .fetch_optional(db)
            .await
            .unwrap_or(None)
    } else {
        None
    };
    
    sqlx::query(
        "INSERT INTO action_logs 
         (id, action_type, component, status, error_message, instance_id, metadata, instance_status_before, created_at) 
         VALUES ($1, $2, 'api', $3, $4, $5, $6, $7, NOW())"
    )
    .bind(log_id)
    .bind(action_type)
    .bind(status)
    .bind(error_message)
    .bind(instance_id)
    .bind(metadata)
    .bind(before_status)
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
    // Capture instance status at completion (if the log has an instance_id)
    let instance_id: Option<Uuid> = sqlx::query_scalar("SELECT instance_id FROM action_logs WHERE id = $1")
        .bind(log_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

    let after_status: Option<String> = if let Some(iid) = instance_id {
        sqlx::query_scalar("SELECT status::text FROM instances WHERE id = $1")
            .bind(iid)
            .fetch_optional(db)
            .await
            .unwrap_or(None)
    } else {
        None
    };

    sqlx::query(
        "UPDATE action_logs 
         SET status = $2, duration_ms = $3, error_message = $4, metadata = COALESCE($5, metadata),
             instance_status_after = COALESCE($6, instance_status_after),
             completed_at = NOW()
         WHERE id = $1"
    )
    .bind(log_id)
    .bind(status)
    .bind(duration_ms)
    .bind(error_message)
    .bind(metadata)
    .bind(after_status)
    .execute(db)
    .await?;
    
    Ok(())
}
