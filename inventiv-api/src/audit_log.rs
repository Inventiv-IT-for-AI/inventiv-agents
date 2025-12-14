use sqlx::{Pool, Postgres};
use serde_json::Value;
use uuid::Uuid;
use std::time::Instant;

/// Audit logger for tracking all backend actions
pub struct AuditLogger {
    db: Pool<Postgres>,
}

impl AuditLogger {
    pub fn new(db: Pool<Postgres>) -> Self {
        Self { db }
    }
    
    /// Log the start of an action
    pub async fn log_action(
        &self,
        action_type: &str,
        instance_id: Option<Uuid>,
        status: &str,
        error_code: Option<&str>,
        error_message: Option<&str>,
        request_payload: Option<Value>,
        response_payload: Option<Value>,
    ) -> Result<Uuid, sqlx::Error> {
        let log_id = Uuid::new_v4();
        
        sqlx::query!(
            "INSERT INTO action_logs 
             (id, action_type, component, status, error_code, error_message, 
              instance_id, request_payload, response_payload, created_at)
             VALUES ($1, $2, 'api', $3, $4, $5, $6, $7, $8, NOW())",
            log_id,
            action_type,
            status,
            error_code,
            error_message,
            instance_id,
            request_payload,
            response_payload
        )
        .execute(&self.db)
        .await?;
        
        println!("📝 Logged action: {} ({})", action_type, status);
        Ok(log_id)
    }
    
    /// Update log when action completes
    pub async fn update_log_completion(
        &self,
        log_id: Uuid,
        status: &str,
        duration_ms: i32,
        response_payload: Option<Value>,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE action_logs 
             SET status = $2, duration_ms = $3, response_payload = $4, 
                 error_code = $5, error_message = $6, completed_at = NOW()
             WHERE id = $1",
            log_id,
            status,
            duration_ms,
            response_payload,
            error_code,
            error_message
        )
        .execute(&self.db)
        .await?;
        
        println!("✅ Action completed: {} in {}ms", status, duration_ms);
        Ok(())
    }
    
    /// Helper to create a scoped logger that tracks duration automatically
    pub fn start_action(&self, action_type: &str, instance_id: Option<Uuid>, request_payload: Option<Value>) -> ScopedLogger {
        ScopedLogger {
            logger: self.clone(),
            action_type: action_type.to_string(),
            instance_id,
            request_payload,
            start_time: Instant::now(),
            log_id: None,
        }
    }
}

// Clone implementation for convenience
impl Clone for AuditLogger {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
        }
    }
}

/// Scoped logger that automatically logs completion when dropped
pub struct ScopedLogger {
    logger: AuditLogger,
    action_type: String,
    instance_id: Option<Uuid>,
    request_payload: Option<Value>,
    start_time: Instant,
    log_id: Option<Uuid>,
}

impl ScopedLogger {
    /// Initialize the log entry (call this at the start of the operation)
    pub async fn init(&mut self) -> Result<(), sqlx::Error> {
        let log_id = self.logger.log_action(
            &self.action_type,
            self.instance_id,
            "in_progress",
            None,
            None,
            self.request_payload.clone(),
            None,
        ).await?;
        
        self.log_id = Some(log_id);
        Ok(())
    }
    
    /// Mark the operation as successful
    pub async fn success(&self, response_payload: Option<Value>) -> Result<(), sqlx::Error> {
        if let Some(log_id) = self.log_id {
            let duration_ms = self.start_time.elapsed().as_millis() as i32;
            self.logger.update_log_completion(
                log_id,
                "success",
                duration_ms,
                response_payload,
                None,
                None,
            ).await?;
        }
        Ok(())
    }
    
    /// Mark the operation as failed
    pub async fn failed(&self, error_code: &str, error_message: &str) -> Result<(), sqlx::Error> {
        if let Some(log_id) = self.log_id {
            let duration_ms = self.start_time.elapsed().as_millis() as i32;
            self.logger.update_log_completion(
                log_id,
                "failed",
                duration_ms,
                None,
                Some(error_code),
                Some(error_message),
            ).await?;
        }
        Ok(())
    }
}
