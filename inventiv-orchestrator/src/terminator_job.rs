use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::finops_events;
use crate::logger;
use inventiv_providers::CloudProvider;
use crate::provider_manager::ProviderManager;
use crate::state_machine;

async fn delete_instance_volumes_best_effort(
    pool: &Pool<Postgres>,
    provider: &dyn CloudProvider,
    instance_id: Uuid,
) -> bool {
    // Returns true if there are no remaining deletable volumes (all deleted or none configured).
    // We only delete volumes marked delete_on_terminate=true.
    
    // First, get instance info to discover volumes that might not be in instance_volumes table
    let instance_info: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT provider_instance_id::text, 
               (SELECT COALESCE(z.code, z.name) FROM zones z WHERE z.id = i.zone_id) AS zone
        FROM instances i
        WHERE i.id = $1
          AND i.provider_instance_id IS NOT NULL
        "#,
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    
    // Discover volumes attached to the instance (even if not in instance_volumes table)
    // This handles cases where Scaleway creates volumes automatically (e.g., local boot volumes)
    if let Some((provider_instance_id, zone)) = instance_info {
        if let Ok(attached_volumes) = provider.list_attached_volumes(&zone, &provider_instance_id).await {
            for av in attached_volumes {
                // Check if this volume is already tracked in instance_volumes
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM instance_volumes WHERE instance_id=$1 AND provider_volume_id=$2 AND deleted_at IS NULL)",
                )
                .bind(instance_id)
                .bind(&av.provider_volume_id)
                .fetch_one(pool)
                .await
                .unwrap_or(false);
                
                if !exists {
                    // Volume exists at provider but not in our DB - track it so we can delete it
                    eprintln!(
                        "🔍 [job-terminator] Discovered untracked volume {} for instance {} - adding to deletion queue",
                        av.provider_volume_id, instance_id
                    );
                    
                    let row_id = Uuid::new_v4();
                    let provider_id: Option<Uuid> = sqlx::query_scalar(
                        "SELECT provider_id FROM instances WHERE id = $1"
                    )
                    .bind(instance_id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();
                    
                    if let Some(pid) = provider_id {
                        let _ = sqlx::query(
                            r#"
                            INSERT INTO instance_volumes 
                            (id, instance_id, provider_id, zone_code, provider_volume_id, provider_volume_name, volume_type, size_bytes, delete_on_terminate, status, attached_at, is_boot)
                            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE, 'attached', NOW(), $9)
                            ON CONFLICT (instance_id, provider_volume_id) DO NOTHING
                            "#,
                        )
                        .bind(row_id)
                        .bind(instance_id)
                        .bind(pid)
                        .bind(&zone)
                        .bind(&av.provider_volume_id)
                        .bind(av.provider_volume_name.as_deref())
                        .bind(&av.volume_type)
                        .bind(av.size_bytes.unwrap_or(0))
                        .bind(av.boot)
                        .execute(pool)
                        .await;
                    }
                }
            }
        }
    }
    
    // Now delete all volumes marked delete_on_terminate=true
    let vols: Vec<(Uuid, String, String, bool)> = sqlx::query_as(
        r#"
        SELECT id, provider_volume_id, zone_code, delete_on_terminate
        FROM instance_volumes
        WHERE instance_id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut remaining = 0usize;

    for (row_id, provider_volume_id, zone_code, delete_on_terminate) in vols {
        if !delete_on_terminate {
            continue;
        }
        remaining += 1;

        let log_id = logger::log_event_with_metadata(
            pool,
            "PROVIDER_DELETE_VOLUME",
            "in_progress",
            instance_id,
            None,
            Some(serde_json::json!({"zone": zone_code, "volume_id": provider_volume_id})),
        )
        .await
        .ok();

        let start = std::time::Instant::now();
        let res = provider
            .delete_volume(&zone_code, &provider_volume_id)
            .await;
        let ok = matches!(res, Ok(true));

        if let Some(lid) = log_id {
            let dur = start.elapsed().as_millis() as i32;
            match &res {
                Ok(true) => logger::log_event_complete(pool, lid, "success", dur, None)
                    .await
                    .ok(),
                Ok(false) => logger::log_event_complete(
                    pool,
                    lid,
                    "failed",
                    dur,
                    Some("Provider returned non-success"),
                )
                .await
                .ok(),
                Err(e) => {
                    logger::log_event_complete(pool, lid, "failed", dur, Some(&e.to_string()))
                        .await
                        .ok()
                }
            };
        }

        if ok {
            let _ = sqlx::query(
                "UPDATE instance_volumes SET status='deleted', deleted_at=NOW() WHERE id=$1",
            )
            .bind(row_id)
            .execute(pool)
            .await;
            remaining -= 1;
        }
    }

    remaining == 0
}

/// job-terminator: processes TERMINATING instances until provider deletion is confirmed.
/// Uses SKIP LOCKED claiming so multiple orchestrators can run safely.
pub async fn run(pool: Pool<Postgres>, redis_client: redis::Client) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    println!("🧹 job-terminator started (processing TERMINATING instances)");

    loop {
        interval.tick().await;

        match terminator_terminating_instances(&pool, &redis_client).await {
            Ok(count) if count > 0 => {
                println!("🧹 job-terminator: progressed {} instance(s)", count)
            }
            Ok(_) => {}
            Err(e) => eprintln!("❌ job-terminator error: {:?}", e),
        }
    }
}

pub async fn terminator_terminating_instances(
    pool: &Pool<Postgres>,
    redis_client: &redis::Client,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Claim instances that are terminating and haven't been reconciled recently
    // This handles cases where:
    // 1. Redis event was lost (non-durable pub/sub)
    // 2. process_termination failed silently
    // 3. Instance is stuck waiting for provider deletion confirmation
    let claimed: Vec<(Uuid, Uuid, Option<String>, Option<String>)> = sqlx::query_as(
        "WITH cte AS (
            SELECT i.id,
                   i.provider_id,
                   i.provider_instance_id::text AS provider_instance_id,
                   (SELECT COALESCE(z.code, z.name) FROM zones z WHERE z.id = i.zone_id) AS zone
            FROM instances i
            WHERE i.status = 'terminating'
              AND (i.last_reconciliation IS NULL OR i.last_reconciliation < NOW() - INTERVAL '30 seconds')
            ORDER BY i.last_reconciliation NULLS FIRST
            LIMIT 50
            FOR UPDATE SKIP LOCKED
        )
        UPDATE instances i
        SET last_reconciliation = NOW()
        FROM cte
        WHERE i.id = cte.id
        RETURNING cte.id, cte.provider_id, cte.provider_instance_id, cte.zone",
    )
    .fetch_all(pool)
    .await?;
    
    if !claimed.is_empty() {
        eprintln!(
            "🔵 [job-terminator] Claimed {} instance(s) for termination processing",
            claimed.len()
        );
    }

    if claimed.is_empty() {
        return Ok(0);
    }

    let mut progressed = 0usize;

    for (instance_id, provider_id, provider_instance_id_opt, zone_opt) in claimed {
        // If we don't even have a provider instance id, we can safely finalize termination in DB.
        // This happens for invalid/failed provisioning requests that never created a provider resource.
        if provider_instance_id_opt.as_deref().unwrap_or("").is_empty() {
            let start = std::time::Instant::now();
            let log_id = logger::log_event_with_metadata(
                pool,
                "TERMINATION_CONFIRMED",
                "in_progress",
                instance_id,
                None,
                Some(serde_json::json!({"reason": "no_provider_instance_id"})),
            )
            .await
            .ok();

            let _ = sqlx::query(
                "UPDATE instances
                 SET deletion_reason = COALESCE(deletion_reason, 'no_provider_resource')
                 WHERE id = $1",
            )
            .bind(instance_id)
            .execute(pool)
            .await;

            // Even if there was no provider instance, we may have created volumes during provisioning.
            let provider_code: String =
                sqlx::query_scalar("SELECT code FROM providers WHERE id = $1")
                    .bind(provider_id)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None)
                    .unwrap_or_else(|| ProviderManager::current_provider_name());
            if let Ok(provider) = ProviderManager::get_provider(&provider_code, pool.clone()).await {
                let _ =
                    delete_instance_volumes_best_effort(pool, provider.as_ref(), instance_id).await;
            }

            let changed = state_machine::terminating_to_terminated(pool, instance_id).await?;
            if changed {
                let _ = finops_events::emit_instance_cost_stop(
                    pool,
                    redis_client,
                    instance_id,
                    "inventiv-orchestrator/terminator_job",
                    "no_provider_resource",
                )
                .await;
            }

            if let Some(lid) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                logger::log_event_complete(pool, lid, "success", duration, None)
                    .await
                    .ok();
            }

            progressed += 1;
            continue;
        }

        // If we have a provider id but no zone, we cannot call the provider API safely.
        let Some(zone) = zone_opt else {
            eprintln!(
                "⚠️  [job-terminator] Missing zone for instance {} (provider_instance_id present).",
                instance_id
            );
            let _ = sqlx::query("UPDATE instances SET last_reconciliation = NULL, error_code = 'MISSING_ZONE', error_message = 'Missing zone for termination' WHERE id = $1")
                .bind(instance_id)
                .execute(pool)
                .await;
            continue;
        };

        let provider_instance_id = provider_instance_id_opt.unwrap_or_default();
        let provider_code: String = sqlx::query_scalar("SELECT code FROM providers WHERE id = $1")
            .bind(provider_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| ProviderManager::current_provider_name());

        let Ok(provider) = ProviderManager::get_provider(&provider_code, pool.clone()).await else {
            let _ = sqlx::query("UPDATE instances SET last_reconciliation = NULL WHERE id = $1")
                .bind(instance_id)
                .execute(pool)
                .await;
            continue;
        };

        match provider
            .check_instance_exists(&zone, &provider_instance_id)
            .await
        {
            Ok(false) => {
                // Provider deletion confirmed → delete volumes first (Scaleway can reject delete while attached).
                let start = std::time::Instant::now();
                let log_id = logger::log_event(
                    pool,
                    "TERMINATION_CONFIRMED",
                    "in_progress",
                    instance_id,
                    None,
                )
                .await
                .ok();

                let volumes_deleted =
                    delete_instance_volumes_best_effort(pool, provider.as_ref(), instance_id).await;
                if !volumes_deleted {
                    // Keep terminating and retry later until volumes are gone.
                    let _ = sqlx::query(
                        "UPDATE instances
                         SET last_reconciliation = NULL,
                             error_code = COALESCE(error_code, 'VOLUMES_DELETE_PENDING'),
                             error_message = COALESCE(error_message, 'Waiting for provider volumes deletion')
                         WHERE id = $1",
                    )
                    .bind(instance_id)
                    .execute(pool)
                    .await;

                    if let Some(lid) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(
                            pool,
                            lid,
                            "success",
                            duration,
                            Some("Provider deleted, volumes still pending"),
                        )
                        .await
                        .ok();
                    }
                    progressed += 1;
                    continue;
                }

                let changed = state_machine::terminating_to_terminated(pool, instance_id).await?;
                if changed {
                    let _ = finops_events::emit_instance_cost_stop(
                        pool,
                        redis_client,
                        instance_id,
                        "inventiv-orchestrator/terminator_job",
                        "termination_confirmed",
                    )
                    .await;
                }

                if let Some(lid) = log_id {
                    let duration = start.elapsed().as_millis() as i32;
                    logger::log_event_complete(pool, lid, "success", duration, None)
                        .await
                        .ok();
                }

                progressed += 1;
            }
            Ok(true) => {
                // Still exists → retry termination request
                let start = std::time::Instant::now();
                let log_id = logger::log_event_with_metadata(
                    pool,
                    "TERMINATOR_RETRY",
                    "in_progress",
                    instance_id,
                    None,
                    Some(serde_json::json!({"zone": zone, "provider_instance_id": provider_instance_id})),
                )
                .await
                .ok();

                match provider
                    .terminate_instance(&zone, &provider_instance_id)
                    .await
                {
                    Ok(true) => {
                        // Even if terminate_instance returns Ok(true), the instance may still be stopping
                        // Check instance state to determine if this is a retry or truly completed
                        let instance_state = provider.get_server_state(&zone, &provider_instance_id).await.ok().flatten();
                        let is_still_stopping = instance_state.as_deref()
                            .map(|s| matches!(s, "stopping" | "stopped" | "stopped_in_place"))
                            .unwrap_or(false);
                        
                        if let Some(lid) = log_id {
                            let duration = start.elapsed().as_millis() as i32;
                            if is_still_stopping {
                                // Instance is still stopping - this is a normal retry
                                let state_str = instance_state.as_deref().unwrap_or("stopping");
                                logger::log_event_complete(
                                    pool,
                                    lid,
                                    "retry",
                                    duration,
                                    Some(&format!("Instance is {} - retrying termination", state_str)),
                                )
                                .await
                                .ok();
                            } else {
                                // Termination request accepted, but instance may still exist
                                // This is still a retry until instance is fully deleted
                                logger::log_event_complete(
                                    pool,
                                    lid,
                                    "retry",
                                    duration,
                                    Some("Termination retried - waiting for instance deletion"),
                                )
                                .await
                                .ok();
                            }
                        }
                        progressed += 1;
                    }
                    Ok(false) => {
                        // Provider returned non-success - this might be normal if instance is still stopping
                        // Check instance state to determine if this is a retry or a real failure
                        let instance_state = provider.get_server_state(&zone, &provider_instance_id).await.ok().flatten();
                        let is_stopping = instance_state.as_deref()
                            .map(|s| matches!(s, "stopping" | "stopped" | "stopped_in_place"))
                            .unwrap_or(false);
                        
                        if let Some(lid) = log_id {
                            let duration = start.elapsed().as_millis() as i32;
                            if is_stopping {
                                // Instance is stopping - this is a normal retry, not a failure
                                let state_str = instance_state.as_deref().unwrap_or("stopping");
                                logger::log_event_complete(
                                    pool,
                                    lid,
                                    "retry",
                                    duration,
                                    Some(&format!("Instance is {} - retrying termination", state_str)),
                                )
                                .await
                                .ok();
                            } else {
                                // Real failure - instance is not stopping
                                logger::log_event_complete(
                                    pool,
                                    lid,
                                    "failed",
                                    duration,
                                    Some("Provider returned non-success"),
                                )
                                .await
                                .ok();
                                let _ = sqlx::query(
                                    "UPDATE instances
                                     SET last_reconciliation = NULL,
                                         error_code = COALESCE(error_code, 'TERMINATOR_RETRY_FAILED'),
                                         error_message = COALESCE(error_message, 'Provider returned non-success on terminate')
                                     WHERE id = $1"
                                )
                                    .bind(instance_id)
                                    .execute(pool)
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        // Check if error indicates instance is stopping (normal retry scenario)
                        let is_stopping_retry = msg.contains("current state: stopping") 
                            || msg.contains("current state: stopped")
                            || msg.contains("failed to stop");
                        
                        if let Some(lid) = log_id {
                            let duration = start.elapsed().as_millis() as i32;
                            if is_stopping_retry {
                                // Extract state from error message if available
                                let state_msg = if msg.contains("current state:") {
                                    msg.split("current state: ").nth(1)
                                        .and_then(|s| s.split_whitespace().next())
                                        .map(|s| format!("Instance is {} - retrying termination", s))
                                        .unwrap_or_else(|| "Instance is stopping - retrying termination".to_string())
                                } else {
                                    "Instance is stopping - retrying termination".to_string()
                                };
                                
                                // This is a normal retry, not a failure
                                logger::log_event_complete(pool, lid, "retry", duration, Some(&state_msg))
                                    .await
                                    .ok();
                            } else {
                                // Real error - log as failed
                                logger::log_event_complete(pool, lid, "failed", duration, Some(&msg))
                                    .await
                                    .ok();
                                let _ = sqlx::query(
                                    "UPDATE instances
                                     SET last_reconciliation = NULL,
                                         error_code = COALESCE(error_code, 'TERMINATOR_RETRY_FAILED'),
                                         error_message = COALESCE(error_message, $2)
                                     WHERE id = $1",
                                )
                                .bind(instance_id)
                                .bind(&msg)
                                .execute(pool)
                                .await;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let msg = e.to_string();
                eprintln!(
                    "⚠️  [job-terminator] Error checking instance {}: {:?}",
                    instance_id, e
                );

                // Log an action for visibility in UI (otherwise it looks like termination does nothing).
                let start = std::time::Instant::now();
                let log_id = logger::log_event_with_metadata(
                    pool,
                    "TERMINATOR_RETRY",
                    "in_progress",
                    instance_id,
                    Some("check_instance_exists failed; attempting terminate anyway"),
                    Some(serde_json::json!({"zone": zone, "provider_instance_id": provider_instance_id, "error": msg})),
                )
                .await
                .ok();

                // Best-effort: still try to send termination request (providers can be flaky on GET).
                let terminate_res = provider
                    .terminate_instance(&zone, &provider_instance_id)
                    .await;
                if let Some(lid) = log_id {
                    let duration = start.elapsed().as_millis() as i32;
                    match &terminate_res {
                        Ok(true) => {
                            logger::log_event_complete(
                                pool,
                                lid,
                                "success",
                                duration,
                                Some("Termination retried (after check failure)"),
                            )
                            .await
                            .ok();
                        }
                        Ok(false) => {
                            // Check if instance is stopping (normal retry scenario)
                            let instance_state = provider.get_server_state(&zone, &provider_instance_id).await.ok().flatten();
                            let is_stopping = instance_state.as_deref()
                                .map(|s| matches!(s, "stopping" | "stopped" | "stopped_in_place"))
                                .unwrap_or(false);
                            
                            if is_stopping {
                                let state_str = instance_state.as_deref().unwrap_or("stopping");
                                logger::log_event_complete(
                                    pool,
                                    lid,
                                    "retry",
                                    duration,
                                    Some(&format!("Instance is {} - retrying termination (after check failure)", state_str)),
                                )
                                .await
                                .ok();
                            } else {
                                logger::log_event_complete(
                                    pool,
                                    lid,
                                    "failed",
                                    duration,
                                    Some("Provider returned non-success"),
                                )
                                .await
                                .ok();
                            }
                        }
                        Err(e2) => {
                            let err_msg = e2.to_string();
                            // Check if error indicates instance is stopping (normal retry scenario)
                            let is_stopping_retry = err_msg.contains("current state: stopping") 
                                || err_msg.contains("current state: stopped")
                                || err_msg.contains("failed to stop");
                            
                            if is_stopping_retry {
                                let state_msg = if err_msg.contains("current state:") {
                                    err_msg.split("current state: ").nth(1)
                                        .and_then(|s| s.split_whitespace().next())
                                        .map(|s| format!("Instance is {} - retrying termination (after check failure)", s))
                                        .unwrap_or_else(|| "Instance is stopping - retrying termination (after check failure)".to_string())
                                } else {
                                    "Instance is stopping - retrying termination (after check failure)".to_string()
                                };
                                
                                logger::log_event_complete(
                                    pool,
                                    lid,
                                    "retry",
                                    duration,
                                    Some(&state_msg),
                                )
                                .await
                                .ok();
                            } else {
                                logger::log_event_complete(
                                    pool,
                                    lid,
                                    "failed",
                                    duration,
                                    Some(&err_msg),
                                )
                                .await
                                .ok();
                            }
                        }
                    }
                }

                let _ = sqlx::query(
                    "UPDATE instances
                     SET last_reconciliation = NULL,
                         error_code = COALESCE(error_code, 'TERMINATOR_CHECK_FAILED'),
                         error_message = COALESCE(error_message, $2)
                     WHERE id = $1",
                )
                .bind(instance_id)
                .bind(&msg)
                .execute(pool)
                .await;
                progressed += 1;
            }
        }
    }

    Ok(progressed)
}
