// ============================================================================
// HEALTH CHECK & STATE TRANSITION LOGIC
// ============================================================================

use std::net::TcpStream;
use std::time::Duration as StdDuration;
use sqlx::{Pool, Postgres};

/// Check instance health by testing SSH port connectivity
async fn check_instance_health(ip: &str) -> bool {
    // Try to connect to SSH port (22) with 3 second timeout
    // Strip CIDR suffix if present (e.g. "1.2.3.4/32" -> "1.2.3.4")
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let addr = format!("{}:22", clean_ip);
    
    tokio::task::spawn_blocking(move || {
        let socket_addr = match addr.parse() {
            Ok(a) => a,
            Err(_) => return false,
        };
        TcpStream::connect_timeout(
            &socket_addr,
            StdDuration::from_secs(3)
        ).is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Check and transition a booting instance based on health status
pub async fn check_and_transition_instance(
    instance_id: uuid::Uuid,
    ip: Option<String>,
    created_at: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
    failures: i32,
    db: Pool<Postgres>
) {
    let ip = match ip {
        Some(ip) => ip,
        None => {
            println!("⚠️  Instance {} has no IP, skipping health check", instance_id);
            return;
        }
    };
    
    // Check age (timeout after 5 minutes = 300 seconds)
    let age = sqlx::types::chrono::Utc::now() - created_at;
    if age.num_seconds() > 300 {
        println!("⏱️  Instance {} timeout exceeded ({}s), marking as startup_failed", instance_id, age.num_seconds());
        transition_to_failed(instance_id, "STARTUP_TIMEOUT", "Instance failed to become healthy within 5 minutes", &db).await;
        return;
    }
    
    // Perform health check
    let is_healthy = check_instance_health(&ip).await;
    
    if is_healthy {
        // SUCCESS: Transition to ready
        println!("✅ Instance {} health check PASSED! Transitioning to ready", instance_id);
        
        let result = sqlx::query!(
            "UPDATE instances 
             SET status = 'ready', ready_at = NOW(), last_health_check = NOW() 
             WHERE id = $1 AND status = 'booting'",
            instance_id
        )
        .execute(&db)
        .await;
        
        match result {
            Ok(_) => {
                println!("✅ Instance {} is now READY", instance_id);
                log_state_transition(instance_id, "booting", "ready", "Health check passed", &db).await;
            }
            Err(e) => println!("❌ Failed to update instance status: {:?}", e),
        }
    } else {
        // FAILED: Increment failure counter
        let new_failures = failures + 1;
        println!("❌ Instance {} health check FAILED (attempt {}/30)", instance_id, new_failures);
        
        let result = sqlx::query!(
            "UPDATE instances 
             SET health_check_failures = $1, last_health_check = NOW() 
             WHERE id = $2",
            new_failures,
            instance_id
        )
        .execute(&db)
        .await;
        
        match result {
            Ok(_) => {
                // Check if max retries exceeded (30 attempts = 5 minutes)
                if new_failures >= 30 {
                    println!("❌ Instance {} exceeded max health check retries, marking as startup_failed", instance_id);
                    transition_to_failed(instance_id, "HEALTH_CHECK_FAILED", "Instance failed health checks after 30 attempts", &db).await;
                }
            }
            Err(e) => println!("❌ Failed to update failure count: {:?}", e),
        }
    }
}

/// Transition instance to failed state and log error
async fn transition_to_failed(
    instance_id: uuid::Uuid,
    error_code: &str,
    error_message: &str,
    db: &Pool<Postgres>
) {
    // Log the failure via action_logs system
    let metadata = serde_json::json!({
        "error_code": error_code,
        "error_message": error_message,
    });
    
    let log_id = crate::logger::log_event_with_metadata(
        db,
        "INSTANCE_STARTUP_FAILED",
        "failed",
        instance_id,
        Some(error_message),
        Some(metadata),
    ).await.ok();
    
    // Update instance status to startup_failed
    let result = sqlx::query!(
        "UPDATE instances 
         SET status = 'startup_failed'
         WHERE id = $1",
        instance_id
    )
    .execute(db)
    .await;
    
    match result {
        Ok(_) => {
            println!("✅ Instance {} marked as startup_failed", instance_id);
            if let Some(log_id) = log_id {
                crate::logger::log_event_complete(db, log_id, "failed", 0, None).await.ok();
            }
            log_state_transition(instance_id, "booting", "startup_failed", error_message, db).await;
        }
        Err(e) => println!("❌ Failed to mark instance as failed: {:?}", e),
    }
}

/// Log state transition to history table
async fn log_state_transition(
    instance_id: uuid::Uuid,
    from_status: &str,
    to_status: &str,
    reason: &str,
    db: &Pool<Postgres>
) {
    let result = sqlx::query!(
        "INSERT INTO instance_state_history (instance_id, from_status, to_status, reason)
         VALUES ($1, $2, $3, $4)",
        instance_id,
        from_status,
        to_status,
        reason
    )
    .execute(db)
    .await;
    
    match result {
        Ok(_) => println!("📊 State transition logged: {} -> {}", from_status, to_status),
        Err(e) => println!("⚠️  Failed to log state transition: {:?}", e),
    }
}
