use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::provider_manager::ProviderManager;
use crate::state_machine;
use crate::finops_events;
use crate::health_check_flow;

/// job-watch-dog: checks READY instances still exist on provider.
/// Uses SKIP LOCKED claiming so multiple orchestrators can run safely.
pub async fn run(pool: Pool<Postgres>, redis_client: redis::Client) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    println!("🐶 job-watch-dog started (checking READY instances)");

    loop {
        interval.tick().await;

        match watchdog_ready_instances(&pool, &redis_client).await {
            Ok(count) if count > 0 => println!("🐶 job-watch-dog: {} orphan(s) marked", count),
            Ok(_) => {}
            Err(e) => eprintln!("❌ job-watch-dog error: {:?}", e),
        }
    }
}

pub async fn watchdog_ready_instances(
    pool: &Pool<Postgres>,
    redis_client: &redis::Client,
) -> Result<usize, Box<dyn std::error::Error>> {
    let claimed: Vec<(Uuid, Uuid, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "WITH cte AS (
            SELECT i.id,
                   i.provider_id,
                   i.provider_instance_id::text AS provider_instance_id,
                   COALESCE(z.code, z.name) AS zone,
                   i.ip_address::text as ip,
                   NULLIF(btrim(COALESCE(i.worker_model_id, '')), '') as worker_model_id
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
        RETURNING cte.id, cte.provider_id, cte.provider_instance_id, cte.zone, cte.ip, cte.worker_model_id",
    )
    .fetch_all(pool)
    .await?;

    if claimed.is_empty() {
        return Ok(0);
    }

    let mut orphaned_count = 0;
    let vllm_port: u16 = std::env::var("WORKER_VLLM_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8000);

    for (instance_id, provider_id, provider_instance_id, zone, ip, worker_model_id) in claimed {
        let provider_code: String = sqlx::query_scalar("SELECT code FROM providers WHERE id = $1")
            .bind(provider_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| ProviderManager::current_provider_name());

        let Some(provider) = ProviderManager::get_provider(&provider_code, pool.clone()) else {
            let _ = sqlx::query("UPDATE instances SET last_reconciliation = NULL WHERE id = $1")
                .bind(instance_id)
                .execute(pool)
                .await;
            continue;
        };

        match provider.check_instance_exists(&zone, &provider_instance_id).await {
            Ok(false) => {
                let changed = state_machine::mark_provider_deleted(
                    pool,
                    instance_id,
                    &provider_instance_id,
                    "watch_dog",
                )
                .await?;

                if changed {
                    let _ = finops_events::emit_instance_cost_stop(
                        pool,
                        redis_client,
                        instance_id,
                        "inventiv-orchestrator/watch_dog_job",
                        "provider_deleted",
                    )
                    .await;
                }
                orphaned_count += 1;
            }
            Ok(true) => {
                // If the instance is READY but `worker_model_id` is missing, infer it from vLLM `/v1/models`
                // and persist worker runtime fields so the runtime Models module can show serving models.
                if worker_model_id.is_none() {
                    if let Some(ip) = ip.as_deref().filter(|s| !s.trim().is_empty()) {
                        let (ok, ids, _ms, _err) = health_check_flow::check_vllm_http_models(ip, vllm_port).await;
                        if ok {
                            if let Some(mid) = ids.get(0).cloned() {
                                let _ = sqlx::query(
                                    r#"
                                    UPDATE instances
                                    SET
                                      worker_model_id = $2,
                                      worker_status = 'ready',
                                      worker_last_heartbeat = NOW(),
                                      worker_vllm_port = $3
                                    WHERE id = $1 AND (worker_model_id IS NULL OR btrim(worker_model_id) = '')
                                    "#,
                                )
                                .bind(instance_id)
                                .bind(mid)
                                .bind(vllm_port as i32)
                                .execute(pool)
                                .await;
                            }
                        }
                    }
                }
            }
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
