use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::finops_events;
use crate::logger;
use crate::provider::CloudProvider;
use crate::provider_manager::ProviderManager;
use crate::state_machine;

async fn delete_instance_volumes_best_effort(
    pool: &Pool<Postgres>,
    provider: &dyn CloudProvider,
    instance_id: Uuid,
) -> bool {
    // Returns true if there are no remaining deletable volumes (all deleted or none configured).
    // We only delete volumes marked delete_on_terminate=true.
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
                        if let Some(lid) = log_id {
                            let duration = start.elapsed().as_millis() as i32;
                            logger::log_event_complete(
                                pool,
                                lid,
                                "success",
                                duration,
                                Some("Termination retried"),
                            )
                            .await
                            .ok();
                        }
                        progressed += 1;
                    }
                    Ok(false) => {
                        if let Some(lid) = log_id {
                            let duration = start.elapsed().as_millis() as i32;
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
                    Err(e) => {
                        let msg = e.to_string();
                        if let Some(lid) = log_id {
                            let duration = start.elapsed().as_millis() as i32;
                            logger::log_event_complete(pool, lid, "failed", duration, Some(&msg))
                                .await
                                .ok();
                        }
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
                        Ok(true) => logger::log_event_complete(
                            pool,
                            lid,
                            "success",
                            duration,
                            Some("Termination retried (after check failure)"),
                        )
                        .await
                        .ok(),
                        Ok(false) => logger::log_event_complete(
                            pool,
                            lid,
                            "failed",
                            duration,
                            Some("Provider returned non-success"),
                        )
                        .await
                        .ok(),
                        Err(e2) => logger::log_event_complete(
                            pool,
                            lid,
                            "failed",
                            duration,
                            Some(&e2.to_string()),
                        )
                        .await
                        .ok(),
                    };
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
