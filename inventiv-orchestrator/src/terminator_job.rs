use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::logger;
use crate::provider::CloudProvider;
use crate::provider_manager::ProviderManager;
use crate::state_machine;

/// job-terminator: processes TERMINATING instances until provider deletion is confirmed.
/// Uses SKIP LOCKED claiming so multiple orchestrators can run safely.
pub async fn run(pool: Pool<Postgres>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    println!("🧹 job-terminator started (processing TERMINATING instances)");

    loop {
        interval.tick().await;

        if let Some(provider) = ProviderManager::get_provider("scaleway") {
            match terminator_terminating_instances(&pool, provider.as_ref()).await {
                Ok(count) if count > 0 => println!("🧹 job-terminator: progressed {} instance(s)", count),
                Ok(_) => {}
                Err(e) => eprintln!("❌ job-terminator error: {:?}", e),
            }
        }
    }
}

pub async fn terminator_terminating_instances(
    pool: &Pool<Postgres>,
    provider: &(impl CloudProvider + ?Sized),
) -> Result<usize, Box<dyn std::error::Error>> {
    let claimed: Vec<(Uuid, String, String)> = sqlx::query_as(
        "WITH cte AS (
            SELECT i.id,
                   i.provider_instance_id::text AS provider_instance_id,
                   COALESCE(z.code, z.name) AS zone
            FROM instances i
            JOIN zones z ON i.zone_id = z.id
            WHERE i.status = 'terminating'
              AND i.provider_instance_id IS NOT NULL
              AND (i.last_reconciliation IS NULL OR i.last_reconciliation < NOW() - INTERVAL '30 seconds')
            ORDER BY i.last_reconciliation NULLS FIRST
            LIMIT 50
            FOR UPDATE SKIP LOCKED
        )
        UPDATE instances i
        SET last_reconciliation = NOW()
        FROM cte
        WHERE i.id = cte.id
        RETURNING cte.id, cte.provider_instance_id, cte.zone",
    )
    .fetch_all(pool)
    .await?;

    if claimed.is_empty() {
        return Ok(0);
    }

    let mut progressed = 0usize;

    for (instance_id, provider_instance_id, zone) in claimed {
        match provider.check_instance_exists(&zone, &provider_instance_id).await {
            Ok(false) => {
                // Provider deletion confirmed → finalize
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

                let _ = state_machine::terminating_to_terminated(pool, instance_id).await?;

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

                match provider.terminate_instance(&zone, &provider_instance_id).await {
                    Ok(true) => {
                        if let Some(lid) = log_id {
                            let duration = start.elapsed().as_millis() as i32;
                            logger::log_event_complete(pool, lid, "success", duration, Some("Termination retried"))
                                .await
                                .ok();
                        }
                        progressed += 1;
                    }
                    Ok(false) => {
                        if let Some(lid) = log_id {
                            let duration = start.elapsed().as_millis() as i32;
                            logger::log_event_complete(pool, lid, "failed", duration, Some("Provider returned non-success"))
                                .await
                                .ok();
                        }
                        let _ = sqlx::query("UPDATE instances SET last_reconciliation = NULL WHERE id = $1")
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
                        let _ = sqlx::query("UPDATE instances SET last_reconciliation = NULL WHERE id = $1")
                            .bind(instance_id)
                            .execute(pool)
                            .await;
                    }
                }
            }
            Err(e) => {
                eprintln!("⚠️  [job-terminator] Error checking instance {}: {:?}", instance_id, e);
                let _ = sqlx::query("UPDATE instances SET last_reconciliation = NULL WHERE id = $1")
                    .bind(instance_id)
                    .execute(pool)
                    .await;
            }
        }
    }

    Ok(progressed)
}

