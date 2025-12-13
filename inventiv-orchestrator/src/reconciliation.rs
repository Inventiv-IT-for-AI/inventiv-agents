use sqlx::{Pool, Postgres};
use uuid::Uuid;
use crate::provider::CloudProvider; // Only trait
use crate::logger;

/// Smart reconciliation: Only checks instances not reconciled in last 60 seconds
/// This allows frequent loop (10s) without overloading provider API
pub async fn reconcile_instances(
    pool: &Pool<Postgres>,
    provider: &(impl CloudProvider + ?Sized), // Allow unsized (dyn)
) -> Result<usize, Box<dyn std::error::Error>> {
    println!("🔍 [Reconciliation] Starting smart reconciliation check...");

    // Fetch instances that need reconciliation:
    // - STABLE states only: 'ready', 'terminating' (NOT 'booting' - that's health_check's job!)
    // - Have provider_instance_id
    // - NOT reconciled in last 60 seconds (smart filtering)
    let instances = sqlx::query!(
        "SELECT id, provider_instance_id
         FROM instances
         WHERE status IN ('ready', 'terminating')
         AND provider_instance_id IS NOT NULL
         AND (last_reconciliation IS NULL OR last_reconciliation < NOW() - INTERVAL '60 seconds')"
    )
    .fetch_all(pool)
    .await?;

    if instances.is_empty() {
        println!("✅ [Reconciliation] No instances need checking (all recently reconciled)");
        return Ok(0);
    }

    println!("🔍 [Reconciliation] Checking {} instances...", instances.len());
    let mut orphaned_count = 0;

    for instance in instances {
        let instance_id = instance.id;
        let provider_id = instance.provider_instance_id.unwrap();

        println!("🔍 [Reconciliation] Checking instance {} (provider: {})", instance_id, provider_id);

        // Fetch current status and zone
        let instance_data = sqlx::query!(
            "SELECT i.status::text, z.code as zone 
             FROM instances i 
             JOIN zones z ON i.zone_id = z.id 
             WHERE i.id = $1",
            instance_id
        )
        .fetch_optional(pool)
        .await?;

        let (current_status, zone) = match instance_data {
            Some(data) => (data.status, data.zone),
            None => {
                eprintln!("⚠️  Instance {} not found in DB during reconciliation", instance_id);
                continue;
            }
        };

        // Check existence on provider
        let exists = provider.check_instance_exists(&zone, &provider_id).await;

        match exists {
            Ok(false) => {
                // Instance doesn't exist on provider
                if current_status.as_deref() == Some("terminating") {
                    // Watchdog: Instance was being terminated and is now gone → mark as terminated
                    println!("✅ [Reconciliation Watchdog] Instance {} was terminating and is now deleted on provider", instance_id);
                    
                    sqlx::query!(
                        "UPDATE instances 
                         SET status = 'terminated', 
                             terminated_at = COALESCE(terminated_at, NOW()),
                             last_reconciliation = NOW()
                         WHERE id = $1",
                        instance_id
                    )
                    .execute(pool)
                    .await?;
                    
                    println!("✅ [Reconciliation] Instance {} marked as terminated (termination completed)", instance_id);
                } else {
                    // Regular orphan detection (was 'ready' but provider deleted it)
                    println!("🔴 [Reconciliation] Instance {} not found on provider, marking as deleted", instance_id);
                    
                    mark_instance_as_provider_deleted(
                        pool,
                        instance_id,
                        &provider_id,
                    ).await?;
                }
                
                orphaned_count += 1;
            }
            Ok(true) => {
                // Instance exists on provider
                if current_status.as_deref() == Some("terminating") {
                    // Watchdog: Instance stuck in terminating → retry termination
                    println!("⚠️  [Reconciliation Watchdog] Instance {} stuck in 'terminating', retrying...", instance_id);
                    
                    // Retry termination via provider
                    match provider.terminate_instance(&zone, &provider_id).await {
                        Ok(_) => {
                            println!("✅ [Reconciliation] Retried termination for instance {}", instance_id);
                            // Update last_reconciliation so we don't retry immediately
                            sqlx::query!(
                                "UPDATE instances SET last_reconciliation = NOW() WHERE id = $1",
                                instance_id
                            )
                            .execute(pool)
                            .await?;
                        }
                        Err(e) => {
                            eprintln!("❌ [Reconciliation] Failed to retry termination for {}: {:?}", instance_id, e);
                            // Don't update last_reconciliation to retry sooner
                        }
                    }
                } else {
                    // Normal case: instance exists and is in expected state
                    println!("✅ [Reconciliation] Instance {} exists on provider", instance_id);
                    
                    let update_result = sqlx::query!(
                        "UPDATE instances SET last_reconciliation = NOW() WHERE id = $1",
                        instance_id
                    )
                    .execute(pool)
                    .await;
                    
                    if let Err(e) = update_result {
                        eprintln!("⚠️  Failed to update last_reconciliation for {}: {:?}", instance_id, e);
                    }
                }
            }
            Err(e) => {
                // API error, log and skip (don't update last_reconciliation to retry soon)
                eprintln!("⚠️  [Reconciliation] Error checking instance {}: {:?}", instance_id, e);
            }
        }
    }

    println!("✅ [Reconciliation] Complete: {} orphaned instances detected", orphaned_count);
    Ok(orphaned_count)
}

async fn mark_instance_as_provider_deleted(
    pool: &Pool<Postgres>,
    instance_id: Uuid,
    provider_instance_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    // LOG: PROVIDER_DELETED_DETECTED
    let error_msg = format!(
        "Provider instance {} not found on provider infrastructure", 
        provider_instance_id
    );
    
    let metadata = serde_json::json!({
        "provider_instance_id": provider_instance_id,
        "detection_method": "reconciliation"
    });

    let log_id = logger::log_event_with_metadata(
        pool,
        "PROVIDER_DELETED_DETECTED",
        "in_progress",
        instance_id,
        Some(&error_msg),
        Some(metadata),
    ).await.ok();

    // Update instance status
    sqlx::query!(
        "UPDATE instances
         SET status = 'terminated',
             deletion_reason = 'provider_deleted',
             deleted_by_provider = TRUE,
             last_reconciliation = NOW()
         WHERE id = $1",
        instance_id
    )
    .execute(pool)
    .await?;

    println!("✅ [Reconciliation] Instance {} marked as terminated (provider_deleted)", instance_id);

    // Complete log
    if let Some(log_id) = log_id {
        let duration = start.elapsed().as_millis() as i32;
        logger::log_event_complete(pool, log_id, "success", duration, None).await.ok();
    }

    Ok(())
}
