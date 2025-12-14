use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::provider::CloudProvider;
use crate::provider_manager::ProviderManager;
use crate::state_machine;

/// job-watch-dog: checks READY instances still exist on provider.
/// Uses SKIP LOCKED claiming so multiple orchestrators can run safely.
pub async fn run(pool: Pool<Postgres>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    println!("🐶 job-watch-dog started (checking READY instances)");

    loop {
        interval.tick().await;

        if let Some(provider) = ProviderManager::get_provider("scaleway") {
            match watchdog_ready_instances(&pool, provider.as_ref()).await {
                Ok(count) if count > 0 => println!("🐶 job-watch-dog: {} orphan(s) marked", count),
                Ok(_) => {}
                Err(e) => eprintln!("❌ job-watch-dog error: {:?}", e),
            }
        }
    }
}

pub async fn watchdog_ready_instances(
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
            WHERE i.status = 'ready'
              AND i.provider_instance_id IS NOT NULL
              AND (i.last_reconciliation IS NULL OR i.last_reconciliation < NOW() - INTERVAL '60 seconds')
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

    let mut orphaned_count = 0;
    for (instance_id, provider_instance_id, zone) in claimed {
        match provider.check_instance_exists(&zone, &provider_instance_id).await {
            Ok(false) => {
                let _ = state_machine::mark_provider_deleted(
                    pool,
                    instance_id,
                    &provider_instance_id,
                    "watch_dog",
                )
                .await?;
                orphaned_count += 1;
            }
            Ok(true) => {}
            Err(e) => {
                eprintln!("⚠️  [job-watch-dog] Error checking instance {}: {:?}", instance_id, e);
                let _ = sqlx::query("UPDATE instances SET last_reconciliation = NULL WHERE id = $1")
                    .bind(instance_id)
                    .execute(pool)
                    .await;
            }
        }
    }

    Ok(orphaned_count)
}
