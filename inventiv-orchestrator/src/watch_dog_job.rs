use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::finops_events;
use crate::health_check_flow;
use crate::provider_manager::ProviderManager;
use crate::state_machine;

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

        match provider
            .check_instance_exists(&zone, &provider_instance_id)
            .await
        {
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
                // If we don't have volume metadata for this instance yet, introspect provider-attached volumes
                // and persist them into instance_volumes so UI can display Storage (and terminator can cleanup).
                let has_any_volumes: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM instance_volumes WHERE instance_id = $1)",
                )
                .bind(instance_id)
                .fetch_one(pool)
                .await
                .unwrap_or(false);
                if !has_any_volumes {
                    if let Ok(attached) = provider
                        .list_attached_volumes(&zone, &provider_instance_id)
                        .await
                    {
                        for av in attached {
                            if av.volume_type != "sbs_volume" {
                                continue;
                            }
                            let exists: bool = sqlx::query_scalar(
                                "SELECT EXISTS(SELECT 1 FROM instance_volumes WHERE instance_id=$1 AND provider_volume_id=$2)",
                            )
                            .bind(instance_id)
                            .bind(&av.provider_volume_id)
                            .fetch_one(pool)
                            .await
                            .unwrap_or(false);
                            if exists {
                                // Update missing metadata (boot volume size/name can be absent on first insert).
                                if av.size_bytes.unwrap_or(0) > 0
                                    || av.provider_volume_name.is_some()
                                {
                                    let _ = sqlx::query(
                                        r#"
                                        UPDATE instance_volumes
                                        SET
                                          provider_volume_name = COALESCE(provider_volume_name, $3),
                                          size_bytes = CASE
                                            WHEN (size_bytes IS NULL OR size_bytes = 0) AND $4 > 0 THEN $4
                                            ELSE size_bytes
                                          END,
                                          is_boot = $5
                                        WHERE instance_id = $1
                                          AND provider_volume_id = $2
                                        "#,
                                    )
                                    .bind(instance_id)
                                    .bind(&av.provider_volume_id)
                                    .bind(av.provider_volume_name.as_deref())
                                    .bind(av.size_bytes.unwrap_or(0))
                                    .bind(av.boot)
                                    .execute(pool)
                                    .await;
                                }
                                continue;
                            }
                            let row_id = Uuid::new_v4();
                            let _ = sqlx::query(
                                "INSERT INTO instance_volumes (id, instance_id, provider_id, zone_code, provider_volume_id, provider_volume_name, volume_type, size_bytes, perf_iops, delete_on_terminate, status, attached_at, is_boot)
                                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NULL,TRUE,'attached',NOW(),$9)",
                            )
                            .bind(row_id)
                            .bind(instance_id)
                            .bind(provider_id)
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

                // If the instance is READY but `worker_model_id` is missing, infer it from vLLM `/v1/models`
                // and persist worker runtime fields so the runtime Models module can show serving models.
                if worker_model_id.is_none() {
                    if let Some(ip) = ip.as_deref().filter(|s| !s.trim().is_empty()) {
                        let (ok, ids, _ms, _err) =
                            health_check_flow::check_vllm_http_models(ip, vllm_port).await;
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
                eprintln!(
                    "⚠️  [job-watch-dog] Error checking instance {}: {:?}",
                    instance_id, e
                );
                let _ =
                    sqlx::query("UPDATE instances SET last_reconciliation = NULL WHERE id = $1")
                        .bind(instance_id)
                        .execute(pool)
                        .await;
            }
        }
    }

    Ok(orphaned_count)
}
