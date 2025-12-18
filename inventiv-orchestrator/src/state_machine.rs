use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::logger;

/// Record a state transition in instance_state_history.
async fn log_state_transition(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    from_status: &str,
    to_status: &str,
    reason: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO instance_state_history (instance_id, from_status, to_status, reason)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(instance_id)
    .bind(from_status)
    .bind(to_status)
    .bind(reason)
    .execute(db)
    .await;
}

/// Transition BOOTING -> READY (idempotent).
pub async fn booting_to_ready(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE instances
         SET status = 'ready',
             ready_at = NOW(),
             last_health_check = NOW()
         WHERE id = $1 AND status = 'booting'",
    )
    .bind(instance_id)
    .execute(db)
    .await?;

    if res.rows_affected() > 0 {
        let log_id = logger::log_event_with_metadata(
            db,
            "INSTANCE_READY",
            "in_progress",
            instance_id,
            None,
            Some(serde_json::json!({"reason": reason})),
        )
        .await
        .ok();
        if let Some(lid) = log_id {
            logger::log_event_complete(db, lid, "success", 0, None)
                .await
                .ok();
        }
        log_state_transition(db, instance_id, "booting", "ready", reason).await;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Transition BOOTING -> STARTUP_FAILED (idempotent) + logs in action_logs.
pub async fn booting_to_startup_failed(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    error_code: &str,
    error_message: &str,
) -> Result<bool, sqlx::Error> {
    let metadata = serde_json::json!({
        "error_code": error_code,
        "error_message": error_message,
    });

    let log_id = logger::log_event_with_metadata(
        db,
        "INSTANCE_STARTUP_FAILED",
        "failed",
        instance_id,
        Some(error_message),
        Some(metadata),
    )
    .await
    .ok();

    let res = sqlx::query(
        "UPDATE instances
         SET status = 'startup_failed',
             error_code = $2,
             error_message = $3,
             failed_at = COALESCE(failed_at, NOW())
         WHERE id = $1 AND status = 'booting'",
    )
    .bind(instance_id)
    .bind(error_code)
    .bind(error_message)
    .execute(db)
    .await?;

    if let Some(lid) = log_id {
        logger::log_event_complete(db, lid, "failed", 0, None)
            .await
            .ok();
    }

    if res.rows_affected() > 0 {
        log_state_transition(db, instance_id, "booting", "startup_failed", error_message).await;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Update health check failures for BOOTING instances (idempotent).
pub async fn update_booting_health_failures(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    new_failures: i32,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE instances
         SET health_check_failures = $2,
             last_health_check = NOW()
         WHERE id = $1 AND status = 'booting'",
    )
    .bind(instance_id)
    .bind(new_failures)
    .execute(db)
    .await?;

    Ok(res.rows_affected() > 0)
}

/// Mark instance as terminated because provider deleted it (READY -> TERMINATED).
pub async fn mark_provider_deleted(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    provider_instance_id: &str,
    detection_method: &str,
) -> Result<bool, sqlx::Error> {
    let start = std::time::Instant::now();
    let error_msg = format!(
        "Provider instance {} not found on provider infrastructure",
        provider_instance_id
    );

    let metadata = serde_json::json!({
        "provider_instance_id": provider_instance_id,
        "detection_method": detection_method,
    });

    let log_id = logger::log_event_with_metadata(
        db,
        "PROVIDER_DELETED_DETECTED",
        "in_progress",
        instance_id,
        Some(&error_msg),
        Some(metadata),
    )
    .await
    .ok();

    let res = sqlx::query(
        "UPDATE instances
         SET status = 'terminated',
             terminated_at = COALESCE(terminated_at, NOW()),
             deletion_reason = 'provider_deleted',
             deleted_by_provider = TRUE
         WHERE id = $1 AND status = 'ready'",
    )
    .bind(instance_id)
    .execute(db)
    .await?;

    if let Some(lid) = log_id {
        let duration = start.elapsed().as_millis() as i32;
        logger::log_event_complete(db, lid, "success", duration, None)
            .await
            .ok();
    }

    if res.rows_affected() > 0 {
        log_state_transition(db, instance_id, "ready", "terminated", "provider_deleted").await;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Transition TERMINATING -> TERMINATED when deletion is confirmed (idempotent).
pub async fn terminating_to_terminated(
    db: &Pool<Postgres>,
    instance_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE instances
         SET status = 'terminated',
             terminated_at = COALESCE(terminated_at, NOW())
         WHERE id = $1 AND status = 'terminating'",
    )
    .bind(instance_id)
    .execute(db)
    .await?;

    if res.rows_affected() > 0 {
        let log_id = logger::log_event(db, "INSTANCE_TERMINATED", "in_progress", instance_id, None)
            .await
            .ok();
        if let Some(lid) = log_id {
            logger::log_event_complete(db, lid, "success", 0, None)
                .await
                .ok();
        }
        log_state_transition(
            db,
            instance_id,
            "terminating",
            "terminated",
            "termination_confirmed",
        )
        .await;
        Ok(true)
    } else {
        Ok(false)
    }
}
