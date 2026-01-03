use sqlx::{Pool, Postgres};
use uuid::Uuid;
use crate::InstanceResponse;

/// Calculate and attach progress percentage to instances
pub async fn enrich_instances_with_progress(
    db: &Pool<Postgres>,
    instances: &mut [InstanceResponse],
) {
    for instance in instances.iter_mut() {
        match calculate_instance_progress(db, instance.id, &instance.status).await {
            Ok(progress) => {
                instance.progress_percent = Some(progress);
            }
            Err(_) => {
                instance.progress_percent = Some(0);
            }
        }
    }
}

/// Calculate progress percentage (0-100) for an instance based on its status and completed actions.
/// 
/// Progress stages (granular breakdown):
/// 
/// **provisioning (0-20%)**:
///   - 5%: Request created
///   - 20%: PROVIDER_CREATE completed (instance created at provider)
/// 
/// **booting (20-100%)**:
///   - 20%: PROVIDER_CREATE completed
///   - 30%: PROVIDER_START completed (instance powered on)
///   - 40%: PROVIDER_GET_IP completed (IP address assigned)
///   - 50%: WORKER_SSH_INSTALL completed (Docker, dependencies, agent installed)
///   - 60%: WORKER_VLLM_HTTP_OK completed (vLLM HTTP endpoint responding)
///   - 75%: WORKER_MODEL_LOADED completed (LLM model loaded in vLLM)
///   - 90%: WORKER_VLLM_WARMUP completed (model warmed up, ready for inference)
///   - 95%: HEALTH_CHECK success (worker health endpoint confirms readiness)
///   - 100%: ready (VM fully operational)
/// 
/// **Terminal states**:
///   - ready: 100%
///   - terminated/terminating/archived: 0%
///   - failed states: 0%
/// 
/// For Mock providers, progress is simulated based on time elapsed since creation.
pub async fn calculate_instance_progress(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    status: &str,
) -> Result<u8, sqlx::Error> {
    let status_lower = status.to_ascii_lowercase();
    
    // Check if this is a Mock provider instance (simulate progress)
    let provider_name: Option<String> = sqlx::query_scalar(
        r#"
        SELECT p.name
        FROM instances i
        JOIN providers p ON i.provider_id = p.id
        WHERE i.id = $1
        "#,
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await?
    .map(|s: String| s.to_ascii_lowercase());
    
    let is_mock = provider_name.as_deref() == Some("mock");
    
    // Terminal states: no progress or 100%
    match status_lower.as_str() {
        "ready" => return Ok(100),
        "terminated" | "terminating" | "archived" => return Ok(0),
        "provisioning_failed" | "startup_failed" | "failed" => return Ok(0),
        _ => {}
    }
    
    // For Mock providers, simulate progress based on time elapsed
    if is_mock {
        return calculate_mock_progress(db, instance_id, &status_lower).await;
    }
    
    // For real providers (Scaleway, etc.), check actual actions
    if status_lower == "provisioning" {
        let has_provider_create = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'PROVIDER_CREATE'
                  AND status = 'success'
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if has_provider_create {
            return Ok(20); // Moved to booting phase
        }
        return Ok(5); // Just created, minimal progress
    }
    
    if status_lower == "booting" {
        // Step 1: PROVIDER_CREATE (20%)
        let has_provider_create = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'PROVIDER_CREATE'
                  AND status = 'success'
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if !has_provider_create {
            return Ok(10);
        }
        
        // Step 2: PROVIDER_START (30%)
        let has_provider_start = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'PROVIDER_START'
                  AND status = 'success'
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if !has_provider_start {
            return Ok(20);
        }
        
        // Step 3: PROVIDER_GET_IP (40%)
        let has_provider_ip = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'PROVIDER_GET_IP'
                  AND status = 'success'
                  AND metadata->>'ip_address' IS NOT NULL
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if !has_provider_ip {
            return Ok(30);
        }
        
        // Step 4: WORKER_SSH_INSTALL (50%)
        let has_ssh_install = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'WORKER_SSH_INSTALL'
                  AND status = 'success'
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if !has_ssh_install {
            // Check if SSH install is in progress
            let ssh_in_progress = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM action_logs
                    WHERE instance_id = $1
                      AND action_type = 'WORKER_SSH_INSTALL'
                      AND status = 'in_progress'
                )
                "#,
            )
            .bind(instance_id)
            .fetch_one(db)
            .await?;
            
            if ssh_in_progress {
                return Ok(45); // SSH install in progress
            }
            return Ok(40);
        }
        
        // Step 5: WORKER_VLLM_HTTP_OK (60%) - vLLM HTTP endpoint responding
        let has_vllm_http = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'WORKER_VLLM_HTTP_OK'
                  AND status = 'success'
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if !has_vllm_http {
            return Ok(50);
        }
        
        // Step 6: WORKER_MODEL_LOADED (75%) - LLM model loaded in vLLM
        let has_model_loaded = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'WORKER_MODEL_LOADED'
                  AND status = 'success'
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if !has_model_loaded {
            return Ok(60);
        }
        
        // Step 7: WORKER_VLLM_WARMUP (90%) - Model warmed up
        let has_warmup = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'WORKER_VLLM_WARMUP'
                  AND status = 'success'
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if !has_warmup {
            return Ok(75);
        }
        
        // Step 8: HEALTH_CHECK success (95%) - Worker health endpoint confirms readiness
        let has_health_check_success = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'HEALTH_CHECK'
                  AND status = 'success'
            )
            "#,
        )
        .bind(instance_id)
        .fetch_one(db)
        .await?;
        
        if has_health_check_success {
            return Ok(95); // Almost ready, waiting for final transition to 'ready'
        }
        
        return Ok(90); // Warmup completed, waiting for health checks
    }
    
    // Default: minimal progress
    Ok(0)
}

/// Calculate progress for Mock provider instances (simulated based on time elapsed)
async fn calculate_mock_progress(
    db: &Pool<Postgres>,
    instance_id: Uuid,
    status: &str,
) -> Result<u8, sqlx::Error> {
    // Get instance creation time
    let created_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>> = sqlx::query_scalar(
        "SELECT created_at FROM instances WHERE id = $1",
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await?;
    
    let Some(created_at) = created_at else {
        return Ok(0);
    };
    
    let elapsed_secs = (sqlx::types::chrono::Utc::now() - created_at).num_seconds();
    
    match status {
        "provisioning" => {
            // Mock provisioning: simulate quick progression
            if elapsed_secs < 2 {
                return Ok(5);
            }
            return Ok(20); // Quickly move to booting
        }
        "booting" => {
            // Mock booting: simulate progression over time
            // Mock instances progress faster than real ones
            if elapsed_secs < 3 {
                return Ok(30); // PROVIDER_START
            }
            if elapsed_secs < 5 {
                return Ok(40); // PROVIDER_GET_IP
            }
            if elapsed_secs < 8 {
                return Ok(50); // WORKER_SSH_INSTALL
            }
            if elapsed_secs < 10 {
                return Ok(60); // WORKER_VLLM_HTTP_OK
            }
            if elapsed_secs < 12 {
                return Ok(75); // WORKER_MODEL_LOADED
            }
            if elapsed_secs < 15 {
                return Ok(90); // WORKER_VLLM_WARMUP
            }
            return Ok(95); // HEALTH_CHECK success (almost ready)
        }
        _ => Ok(0),
    }
}

