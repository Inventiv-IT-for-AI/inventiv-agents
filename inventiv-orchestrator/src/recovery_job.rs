use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::logger;
use crate::services;
use crate::state_machine;

/// job-recovery: handles stuck instances in various states.
/// 
/// This job provides resilience by:
/// 1. Recovering TERMINATING instances that didn't get processed (similar to provisioning_job)
/// 2. Detecting and recovering from state machine deadlocks
/// 3. Providing a safety net for instances stuck in intermediate states
pub async fn run(pool: Pool<Postgres>, redis_client: redis::Client) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
    println!("🔄 job-recovery started (recovering stuck instances)");

    loop {
        interval.tick().await;

        match recover_stuck_instances(&pool, &redis_client).await {
            Ok(count) if count > 0 => {
                println!("🔄 job-recovery: recovered {} instance(s)", count)
            }
            Ok(_) => {}
            Err(e) => eprintln!("❌ job-recovery error: {:?}", e),
        }
    }
}

async fn recover_stuck_instances(
    pool: &Pool<Postgres>,
    redis_client: &redis::Client,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut recovered = 0usize;

    // 1. Recover TERMINATING instances that didn't get processed via Redis event
    // (Similar to provisioning_job but for termination)
    let stuck_terminating: Vec<(Uuid, Uuid, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        WITH cte AS (
            SELECT i.id,
                   i.provider_id,
                   i.provider_instance_id::text AS provider_instance_id,
                   (SELECT COALESCE(z.code, z.name) FROM zones z WHERE z.id = i.zone_id) AS zone
            FROM instances i
            WHERE i.status = 'terminating'
              AND (i.last_reconciliation IS NULL OR i.last_reconciliation < NOW() - INTERVAL '30 seconds')
              AND i.created_at < NOW() - INTERVAL '2 minutes'
            ORDER BY i.last_reconciliation NULLS FIRST
            LIMIT 25
            FOR UPDATE SKIP LOCKED
        )
        UPDATE instances i
        SET last_reconciliation = NOW()
        FROM cte
        WHERE i.id = cte.id
        RETURNING cte.id, cte.provider_id, cte.provider_instance_id, cte.zone
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (instance_id, _provider_id, provider_instance_id_opt, zone_opt) in stuck_terminating {
        if let Some(zone) = zone_opt {
            if let Some(provider_instance_id) = provider_instance_id_opt {
                eprintln!(
                    "🔄 [job-recovery] Recovering stuck TERMINATING instance {} (provider_instance_id={}, zone={})",
                    instance_id, provider_instance_id, zone
                );
                
                let db_for_log = pool.clone();
                let log_id = logger::log_event_with_metadata(
                    &db_for_log,
                    "RECOVERY_TERMINATE",
                    "in_progress",
                    instance_id,
                    None,
                    Some(serde_json::json!({
                        "zone": zone,
                        "provider_instance_id": provider_instance_id,
                        "reason": "stuck_terminating"
                    })),
                )
                .await
                .ok();

                // Re-process termination
                let db = pool.clone();
                let redis = redis_client.clone();
                let db_for_complete = pool.clone();
                tokio::spawn(async move {
                    services::process_termination(
                        db,
                        redis,
                        instance_id.to_string(),
                        Some(format!("recovery-{}", instance_id)),
                    )
                    .await;

                    if let Some(lid) = log_id {
                        logger::log_event_complete(&db_for_complete, lid, "success", 0, Some("Recovery triggered"))
                            .await
                            .ok();
                    }
                });
                recovered += 1;
            }
        }
    }

    // 2. Detect instances stuck in BOOTING for too long (potential deadlock)
    let stuck_booting: Vec<(Uuid, i64)> = sqlx::query_as(
        r#"
        SELECT id, EXTRACT(EPOCH FROM (NOW() - created_at))::bigint AS age_seconds
        FROM instances
        WHERE status = 'booting'
          AND created_at < NOW() - INTERVAL '2 hours'
          AND (last_reconciliation IS NULL OR last_reconciliation < NOW() - INTERVAL '5 minutes')
        ORDER BY created_at ASC
        LIMIT 10
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (instance_id, age_seconds) in stuck_booting {
        eprintln!(
            "⚠️ [job-recovery] Instance {} stuck in BOOTING for {}s - marking as startup_failed",
            instance_id, age_seconds
        );
        
        let _ = logger::log_event_with_metadata(
            pool,
            "RECOVERY_STARTUP_FAILED",
            "failed",
            instance_id,
            Some("Instance stuck in booting state for too long"),
            Some(serde_json::json!({
                "age_seconds": age_seconds,
                "reason": "stuck_booting_timeout"
            })),
        )
        .await;

        let _ = state_machine::booting_to_startup_failed(
            pool,
            instance_id,
            "RECOVERY_TIMEOUT",
            "Instance stuck in booting state for too long (recovery job)",
        )
        .await;
        recovered += 1;
    }

    Ok(recovered)
}

