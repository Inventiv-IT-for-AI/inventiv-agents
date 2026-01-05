use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::logger;
use crate::services;

/// job-provisioning: re-queues stuck PROVISIONING instances.
///
/// Why: Redis Pub/Sub is not durable. If orchestrator is down during publish, the event is lost,
/// and the instance can remain in `provisioning` forever with no provider_instance_id.
///
/// Strategy:
/// - Claim stale `provisioning` rows (provider_instance_id IS NULL) with SKIP LOCKED
/// - Bump retry_count and set last_reconciliation as a lease timestamp
/// - Call process_provisioning again
pub async fn run(pool: Pool<Postgres>, redis_client: redis::Client) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    println!("🔁 job-provisioning started (re-queue stale PROVISIONING instances)");

    loop {
        interval.tick().await;

        match requeue_stale_provisioning(&pool, &redis_client).await {
            Ok(count) if count > 0 => {
                eprintln!("🔁 [job-provisioning] Re-queued {} stale instance(s)", count)
            }
            Ok(_) => {
                // Silent - no stale instances found (this is normal)
            }
            Err(e) => eprintln!("❌ [job-provisioning] Error: {:?}", e),
        }
    }
}

async fn requeue_stale_provisioning(
    pool: &Pool<Postgres>,
    redis_client: &redis::Client,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Claim rows and lease via last_reconciliation (used as a generic job lease timestamp).
    let claimed: Vec<(Uuid, String, String, i32)> = sqlx::query_as(
        r#"
        WITH cte AS (
            SELECT
                i.id,
                COALESCE(z.code, z.name) AS zone,
                it.code AS instance_type,
                COALESCE(i.retry_count, 0) AS retry_count
            FROM instances i
            JOIN zones z ON z.id = i.zone_id
            JOIN instance_types it ON it.id = i.instance_type_id
            WHERE i.status = 'provisioning'
              AND i.provider_instance_id IS NULL
              AND i.failed_at IS NULL
              AND i.created_at < NOW() - INTERVAL '30 seconds'
              AND (i.last_reconciliation IS NULL OR i.last_reconciliation < NOW() - INTERVAL '30 seconds')
              AND COALESCE(i.retry_count, 0) < 5
            ORDER BY i.created_at ASC
            LIMIT 25
            FOR UPDATE SKIP LOCKED
        )
        UPDATE instances i
        SET last_reconciliation = NOW(),
            retry_count = cte.retry_count + 1
        FROM cte
        WHERE i.id = cte.id
        RETURNING cte.id, cte.zone, cte.instance_type, cte.retry_count + 1 AS retry_count
        "#,
    )
    .fetch_all(pool)
    .await?;

    if claimed.is_empty() {
        return Ok(0);
    }

    let claimed_len = claimed.len();
    eprintln!("🔁 [job-provisioning] Found {} stale instance(s) to re-queue", claimed_len);

    for (instance_id, zone, instance_type, retry_count) in claimed {
        let db = pool.clone();
        let redis = redis_client.clone();
        tokio::spawn(async move {
            let correlation_id = Some(format!("requeue-{}-{}", instance_id, retry_count));
            eprintln!("🔁 [job-provisioning] Re-queuing instance {} (zone={}, type={}, retry={})", 
                instance_id, zone, instance_type, retry_count);
            let start = std::time::Instant::now();
            let log_id = logger::log_event_with_metadata(
                &db,
                "REQUEUE_PROVISION",
                "in_progress",
                instance_id,
                None,
                Some(serde_json::json!({
                    "zone": zone,
                    "instance_type": instance_type,
                    "retry_count": retry_count,
                    "correlation_id": correlation_id,
                })),
            )
            .await
            .ok();

            eprintln!("🔵 [job-provisioning] Calling process_provisioning for instance {}", instance_id);
            services::process_provisioning(
                db.clone(),
                redis,
                instance_id.to_string(),
                zone,
                instance_type,
                correlation_id,
            )
            .await;
            eprintln!("🔵 [job-provisioning] process_provisioning completed for instance {}", instance_id);

            if let Some(lid) = log_id {
                let dur = start.elapsed().as_millis() as i32;
                logger::log_event_complete(
                    &db,
                    lid,
                    "success",
                    dur,
                    Some("Re-queued provisioning"),
                )
                .await
                .ok();
            }
        });
    }

    Ok(claimed_len)
}
