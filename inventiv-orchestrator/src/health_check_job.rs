use sqlx::{Pool, Postgres};

use crate::health_check_flow::check_and_transition_instance;
use crate::provider_manager::ProviderManager;
use crate::logger;

/// job-health-check: processes BOOTING instances and transitions them to READY/STARTUP_FAILED.
/// Uses SKIP LOCKED claiming so multiple orchestrators can run safely.
pub async fn run(pool: Pool<Postgres>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
    println!("🏥 job-health-check started (checking BOOTING instances)");

    loop {
        interval.tick().await;

        // Claim BOOTING instances even if IP is missing. If IP is missing, we try to fetch it from provider.
        let booting_instances: Result<
            Vec<(
                uuid::Uuid,
                Option<String>, // provider_instance_id
                String,         // zone
                Option<String>, // ip
                Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
                Option<i32>,
            )>,
            _,
        > = sqlx::query_as(
            "WITH cte AS (
                SELECT i.id,
                       i.provider_instance_id::text AS provider_instance_id,
                       COALESCE(z.code, z.name) AS zone,
                       i.ip_address::text as ip,
                       i.created_at,
                       i.health_check_failures
                FROM instances i
                JOIN zones z ON i.zone_id = z.id
                WHERE i.status = 'booting'
                  AND i.provider_instance_id IS NOT NULL
                  AND (i.last_health_check IS NULL OR i.last_health_check < NOW() - INTERVAL '10 seconds')
                ORDER BY i.last_health_check NULLS FIRST
                LIMIT 50
                FOR UPDATE SKIP LOCKED
            )
            UPDATE instances i
            SET last_health_check = NOW()
            FROM cte
            WHERE i.id = cte.id
            RETURNING cte.id, cte.provider_instance_id, cte.zone, cte.ip, cte.created_at, cte.health_check_failures",
        )
        .fetch_all(&pool)
        .await;

        match booting_instances {
            Ok(instances) if !instances.is_empty() => {
                println!("🏥 job-health-check: checking {} booting instance(s)...", instances.len());

                for (id, provider_instance_id, zone, ip, created_at, health_check_failures) in instances {
                    let db_clone = pool.clone();
                    tokio::spawn(async move {
                        // If IP is missing, try to fetch it from provider first (bounded by reqwest timeout).
                        if ip.is_none() {
                            if let Some(pid) = provider_instance_id.as_deref() {
                                let provider = ProviderManager::get_provider("scaleway");
                                let Some(provider) = provider else {
                                    return;
                                };
                                let start = std::time::Instant::now();
                                let log_id = logger::log_event_with_metadata(
                                    &db_clone,
                                    "PROVIDER_GET_IP",
                                    "in_progress",
                                    id,
                                    None,
                                    Some(serde_json::json!({"zone": zone, "server_id": pid, "source": "job-health-check"})),
                                ).await.ok();

                                match provider.get_instance_ip(&zone, pid).await {
                                    Ok(Some(found_ip)) => {
                                        let _ = sqlx::query(
                                            "UPDATE instances SET ip_address = $1::inet WHERE id = $2 AND ip_address IS NULL"
                                        )
                                        .bind(&found_ip)
                                        .bind(id)
                                        .execute(&db_clone)
                                        .await;

                                        if let Some(lid) = log_id {
                                            let dur = start.elapsed().as_millis() as i32;
                                            let meta = serde_json::json!({"ip_address": found_ip, "zone": zone, "server_id": pid});
                                            logger::log_event_complete_with_metadata(&db_clone, lid, "success", dur, None, Some(meta)).await.ok();
                                        }
                                        return;
                                    }
                                    Ok(None) => {
                                        if let Some(lid) = log_id {
                                            let dur = start.elapsed().as_millis() as i32;
                                            logger::log_event_complete(&db_clone, lid, "failed", dur, Some("IP not available yet")).await.ok();
                                        }
                                        return;
                                    }
                                    Err(e) => {
                                        if let Some(lid) = log_id {
                                            let dur = start.elapsed().as_millis() as i32;
                                            logger::log_event_complete(&db_clone, lid, "failed", dur, Some(&e.to_string())).await.ok();
                                        }
                                        return;
                                    }
                                }
                            }
                        }

                        check_and_transition_instance(
                            id,
                            ip,
                            created_at.unwrap_or_else(|| sqlx::types::chrono::Utc::now()),
                            health_check_failures.unwrap_or(0),
                            db_clone,
                        )
                        .await;
                    });
                }
            }
            Ok(_) => {}
            Err(e) => {
                println!("⚠️  job-health-check query error: {:?}", e);
            }
        }
    }
}

