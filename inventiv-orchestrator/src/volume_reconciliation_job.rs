use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::logger;
use crate::provider_manager::ProviderManager;

/// job-volume-reconciliation: reconciles volumes between DB and provider.
///
/// This job provides resilience by:
/// 1. Detecting volumes in DB marked as deleted but still existing at provider (retry deletion)
/// 2. Retrying failed volume deletions with exponential backoff
/// 3. Marking volumes as reconciled (reconciled_at) when confirmed deleted at provider
///
/// IMPORTANT: All data is preserved for audit, traceability, FinOps calculations, and debugging.
/// Volumes are never deleted from DB - only marked with reconciled_at timestamp when reconciliation is complete.
/// This allows precise cost calculations and recalculation based on detailed usage per second.
pub async fn run(pool: Pool<Postgres>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    println!("🔍 job-volume-reconciliation started (reconciling volumes between DB and provider)");

    loop {
        interval.tick().await;

        match reconcile_volumes(&pool).await {
            Ok(count) if count > 0 => {
                println!(
                    "🔍 job-volume-reconciliation: reconciled {} volume(s)",
                    count
                )
            }
            Ok(_) => {
                // Silent - no volumes to reconcile (this is normal)
            }
            Err(e) => eprintln!("❌ job-volume-reconciliation error: {:?}", e),
        }
    }
}

async fn reconcile_volumes(pool: &Pool<Postgres>) -> Result<usize, Box<dyn std::error::Error>> {
    let mut reconciled = 0usize;

    // 1. Find volumes marked as deleted in DB but not yet reconciled
    // These are volumes where deletion was reported but we need to verify at provider
    // Only process volumes that haven't been reconciled yet (reconciled_at IS NULL)
    let deleted_but_existing: Vec<(Uuid, String, String, String, Option<Uuid>)> = sqlx::query_as(
        r#"
        SELECT 
            iv.id,
            iv.provider_volume_id,
            iv.zone_code,
            iv.instance_id::text,
            i.provider_id
        FROM instance_volumes iv
        JOIN instances i ON i.id = iv.instance_id
        WHERE iv.deleted_at IS NOT NULL
          AND iv.status = 'deleted'
          AND iv.delete_on_terminate = true
          AND iv.reconciled_at IS NULL
          AND (iv.last_reconciliation IS NULL OR iv.last_reconciliation < NOW() - INTERVAL '5 minutes')
          AND i.provider_instance_id IS NOT NULL
        ORDER BY iv.deleted_at ASC
        LIMIT 50
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (row_id, provider_volume_id, zone_code, instance_id_str, provider_id_opt) in
        deleted_but_existing
    {
        if let Some(provider_id) = provider_id_opt {
            let provider_code: String =
                sqlx::query_scalar("SELECT code FROM providers WHERE id = $1")
                    .bind(provider_id)
                    .fetch_optional(pool)
                    .await?
                    .unwrap_or_else(|| ProviderManager::current_provider_name());

            if let Ok(provider) = ProviderManager::get_provider(&provider_code, pool.clone()).await
            {
                // Check if volume still exists at provider
                match provider
                    .check_volume_exists(&zone_code, &provider_volume_id)
                    .await
                {
                    Ok(true) => {
                        // Volume still exists - retry deletion
                        eprintln!(
                            "🔄 [job-volume-reconciliation] Volume {} still exists at provider (instance {}), retrying deletion",
                            provider_volume_id, instance_id_str
                        );

                        let instance_id = Uuid::parse_str(&instance_id_str).ok();
                        let log_id = logger::log_event_with_metadata(
                            pool,
                            "VOLUME_RECONCILIATION_RETRY_DELETE",
                            "in_progress",
                            instance_id.unwrap_or(Uuid::nil()),
                            None,
                            Some(serde_json::json!({
                                "zone": zone_code,
                                "volume_id": provider_volume_id,
                                "instance_id": instance_id_str,
                                "reason": "volume_marked_deleted_but_still_exists"
                            })),
                        )
                        .await
                        .ok();

                        let start = std::time::Instant::now();
                        let res = provider
                            .delete_volume(&zone_code, &provider_volume_id)
                            .await;

                        if let Some(lid) = log_id {
                            let dur = start.elapsed().as_millis() as i32;
                            match &res {
                                Ok(true) => {
                                    logger::log_event_complete(
                                        pool,
                                        lid,
                                        "success",
                                        dur,
                                        Some("Volume deleted successfully"),
                                    )
                                    .await
                                    .ok();
                                    // Update DB to reflect successful deletion
                                    let _ = sqlx::query(
                                        "UPDATE instance_volumes SET last_reconciliation = NOW() WHERE id = $1"
                                    )
                                    .bind(row_id)
                                    .execute(pool)
                                    .await;
                                    reconciled += 1;
                                }
                                Ok(false) => {
                                    logger::log_event_complete(
                                        pool,
                                        lid,
                                        "failed",
                                        dur,
                                        Some("Provider returned non-success"),
                                    )
                                    .await
                                    .ok();
                                    // Update last_reconciliation to retry later
                                    let _ = sqlx::query(
                                        "UPDATE instance_volumes SET last_reconciliation = NOW() WHERE id = $1"
                                    )
                                    .bind(row_id)
                                    .execute(pool)
                                    .await;
                                }
                                Err(e) => {
                                    logger::log_event_complete(
                                        pool,
                                        lid,
                                        "failed",
                                        dur,
                                        Some(&e.to_string()),
                                    )
                                    .await
                                    .ok();
                                    // Update last_reconciliation to retry later
                                    let _ = sqlx::query(
                                        "UPDATE instance_volumes SET last_reconciliation = NOW() WHERE id = $1"
                                    )
                                    .bind(row_id)
                                    .execute(pool)
                                    .await;
                                }
                            };
                        }
                    }
                    Ok(false) => {
                        // Volume doesn't exist - mark as reconciled (preserve all data for audit/FinOps)
                        eprintln!(
                            "✅ [job-volume-reconciliation] Volume {} confirmed deleted at provider (instance {}), marking as reconciled",
                            provider_volume_id, instance_id_str
                        );
                        let _ = sqlx::query(
                            r#"
                            UPDATE instance_volumes 
                            SET reconciled_at = NOW(), 
                                last_reconciliation = NOW()
                            WHERE id = $1
                            "#,
                        )
                        .bind(row_id)
                        .execute(pool)
                        .await;
                        reconciled += 1;
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️ [job-volume-reconciliation] Error checking volume {}: {:?}",
                            provider_volume_id, e
                        );
                        // Update last_reconciliation to retry later
                        let _ = sqlx::query(
                            "UPDATE instance_volumes SET last_reconciliation = NOW() WHERE id = $1",
                        )
                        .bind(row_id)
                        .execute(pool)
                        .await;
                    }
                }
            }
        }
    }

    // 2. Retry failed volume deletions (volumes with delete_on_terminate=true but deletion failed)
    // Only process volumes that haven't been reconciled yet
    let failed_deletions: Vec<(Uuid, String, String, String, Option<Uuid>)> = sqlx::query_as(
        r#"
        SELECT 
            iv.id,
            iv.provider_volume_id,
            iv.zone_code,
            iv.instance_id::text,
            i.provider_id
        FROM instance_volumes iv
        JOIN instances i ON i.id = iv.instance_id
        WHERE iv.delete_on_terminate = true
          AND iv.deleted_at IS NULL
          AND iv.status != 'deleted'
          AND iv.reconciled_at IS NULL
          AND i.status IN ('terminated', 'provider_deleted', 'terminating')
          AND (iv.last_reconciliation IS NULL OR iv.last_reconciliation < NOW() - INTERVAL '5 minutes')
        ORDER BY iv.last_reconciliation NULLS FIRST
        LIMIT 50
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (row_id, provider_volume_id, zone_code, instance_id_str, provider_id_opt) in
        failed_deletions
    {
        if let Some(provider_id) = provider_id_opt {
            let provider_code: String =
                sqlx::query_scalar("SELECT code FROM providers WHERE id = $1")
                    .bind(provider_id)
                    .fetch_optional(pool)
                    .await?
                    .unwrap_or_else(|| ProviderManager::current_provider_name());

            if let Ok(provider) = ProviderManager::get_provider(&provider_code, pool.clone()).await
            {
                // Check if volume still exists
                match provider
                    .check_volume_exists(&zone_code, &provider_volume_id)
                    .await
                {
                    Ok(true) => {
                        // Volume exists - try to delete it
                        eprintln!(
                            "🔄 [job-volume-reconciliation] Retrying deletion of volume {} (instance {})",
                            provider_volume_id, instance_id_str
                        );

                        let instance_id = Uuid::parse_str(&instance_id_str)
                            .ok()
                            .unwrap_or(Uuid::nil());
                        let log_id = logger::log_event_with_metadata(
                            pool,
                            "VOLUME_RECONCILIATION_RETRY_DELETE",
                            "in_progress",
                            instance_id,
                            None,
                            Some(serde_json::json!({
                                "zone": zone_code,
                                "volume_id": provider_volume_id,
                                "instance_id": instance_id_str,
                                "reason": "retry_failed_deletion"
                            })),
                        )
                        .await
                        .ok();

                        let start = std::time::Instant::now();
                        let res = provider
                            .delete_volume(&zone_code, &provider_volume_id)
                            .await;

                        if let Some(lid) = log_id {
                            let dur = start.elapsed().as_millis() as i32;
                            match &res {
                                Ok(true) => {
                                    logger::log_event_complete(
                                        pool,
                                        lid,
                                        "success",
                                        dur,
                                        Some("Volume deleted successfully"),
                                    )
                                    .await
                                    .ok();
                                    // Mark as deleted but not yet reconciled (will be reconciled in next cycle after verification)
                                    let _ = sqlx::query(
                                        r#"
                                        UPDATE instance_volumes 
                                        SET status='deleted', 
                                            deleted_at=NOW(), 
                                            last_reconciliation=NOW() 
                                        WHERE id = $1
                                        "#,
                                    )
                                    .bind(row_id)
                                    .execute(pool)
                                    .await;
                                    reconciled += 1;
                                }
                                Ok(false) => {
                                    logger::log_event_complete(
                                        pool,
                                        lid,
                                        "failed",
                                        dur,
                                        Some("Provider returned non-success"),
                                    )
                                    .await
                                    .ok();
                                    let _ = sqlx::query(
                                        "UPDATE instance_volumes SET last_reconciliation = NOW() WHERE id = $1"
                                    )
                                    .bind(row_id)
                                    .execute(pool)
                                    .await;
                                }
                                Err(e) => {
                                    logger::log_event_complete(
                                        pool,
                                        lid,
                                        "failed",
                                        dur,
                                        Some(&e.to_string()),
                                    )
                                    .await
                                    .ok();
                                    let _ = sqlx::query(
                                        "UPDATE instance_volumes SET last_reconciliation = NOW() WHERE id = $1"
                                    )
                                    .bind(row_id)
                                    .execute(pool)
                                    .await;
                                }
                            };
                        }
                    }
                    Ok(false) => {
                        // Volume doesn't exist - mark as deleted and reconciled (preserve all data for audit/FinOps)
                        eprintln!(
                            "✅ [job-volume-reconciliation] Volume {} confirmed deleted (instance {}), marking as deleted and reconciled",
                            provider_volume_id, instance_id_str
                        );
                        let _ = sqlx::query(
                            r#"
                            UPDATE instance_volumes 
                            SET status='deleted', 
                                deleted_at=NOW(), 
                                reconciled_at=NOW(),
                                last_reconciliation=NOW() 
                            WHERE id = $1
                            "#,
                        )
                        .bind(row_id)
                        .execute(pool)
                        .await;
                        reconciled += 1;
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️ [job-volume-reconciliation] Error checking volume {}: {:?}",
                            provider_volume_id, e
                        );
                        let _ = sqlx::query(
                            "UPDATE instance_volumes SET last_reconciliation = NOW() WHERE id = $1",
                        )
                        .bind(row_id)
                        .execute(pool)
                        .await;
                    }
                }
            }
        }
    }

    Ok(reconciled)
}
