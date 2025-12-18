use sqlx::{Pool, Postgres};

use crate::health_check_flow::check_and_transition_instance;
use crate::logger;
use crate::provider_manager::ProviderManager;

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
                uuid::Uuid,      // provider_id
                Option<String>, // provider_instance_id
                String,         // zone
                Option<String>, // ip
                Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
                Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
                Option<i32>,
            )>,
            _,
        > = sqlx::query_as(
            "WITH cte AS (
                SELECT i.id,
                       i.provider_id,
                       i.provider_instance_id::text AS provider_instance_id,
                       COALESCE(z.code, z.name) AS zone,
                       i.ip_address::text as ip,
                       i.created_at,
                       i.boot_started_at,
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
            RETURNING cte.id, cte.provider_id, cte.provider_instance_id, cte.zone, cte.ip, cte.created_at, cte.boot_started_at, cte.health_check_failures",
        )
        .fetch_all(&pool)
        .await;

        match booting_instances {
            Ok(instances) if !instances.is_empty() => {
                println!(
                    "🏥 job-health-check: checking {} booting instance(s)...",
                    instances.len()
                );

                for (
                    id,
                    provider_id,
                    provider_instance_id,
                    zone,
                    ip,
                    created_at,
                    boot_started_at,
                    health_check_failures,
                ) in instances
                {
                    let db_clone = pool.clone();
                    tokio::spawn(async move {
                        let created_at =
                            created_at.unwrap_or_else(|| sqlx::types::chrono::Utc::now());
                        let boot_started_at = boot_started_at.unwrap_or(created_at);

                        // If IP is missing, try to fetch it from provider first (bounded by reqwest timeout).
                        if ip.is_none() {
                            if let Some(pid) = provider_instance_id.as_deref() {
                                let provider_code: String =
                                    sqlx::query_scalar("SELECT code FROM providers WHERE id = $1")
                                        .bind(provider_id)
                                        .fetch_optional(&db_clone)
                                        .await
                                        .unwrap_or(None)
                                        .unwrap_or_else(|| {
                                            ProviderManager::current_provider_name()
                                        });
                                let provider =
                                    ProviderManager::get_provider(&provider_code, db_clone.clone());
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
                                            logger::log_event_complete_with_metadata(
                                                &db_clone,
                                                lid,
                                                "success",
                                                dur,
                                                None,
                                                Some(meta),
                                            )
                                            .await
                                            .ok();
                                        }
                                        return;
                                    }
                                    Ok(None) => {
                                        if let Some(lid) = log_id {
                                            let dur = start.elapsed().as_millis() as i32;
                                            logger::log_event_complete(
                                                &db_clone,
                                                lid,
                                                "failed",
                                                dur,
                                                Some("IP not available yet"),
                                            )
                                            .await
                                            .ok();
                                        }

                                        // If we've been booting for a long time with no IP, try a bounded recovery.
                                        // Common root cause on GPU instances: provider can't allocate capacity (out_of_stock),
                                        // leaving the server in `stopped` with no public IP.
                                        let age_secs = (sqlx::types::chrono::Utc::now()
                                            - boot_started_at)
                                            .num_seconds();
                                        if age_secs >= 300 {
                                            let retry_log = logger::log_event_with_metadata(
                                                &db_clone,
                                                "PROVIDER_START_RETRY",
                                                "in_progress",
                                                id,
                                                None,
                                                Some(serde_json::json!({"zone": zone, "server_id": pid, "age_secs": age_secs})),
                                            )
                                            .await
                                            .ok();
                                            let retry_start = std::time::Instant::now();
                                            let retry_res =
                                                provider.start_instance(&zone, pid).await;

                                            if let Some(lid) = retry_log {
                                                let dur = retry_start.elapsed().as_millis() as i32;
                                                match &retry_res {
                                                    Ok(true) => logger::log_event_complete(
                                                        &db_clone,
                                                        lid,
                                                        "success",
                                                        dur,
                                                        Some("Poweron retried"),
                                                    )
                                                    .await
                                                    .ok(),
                                                    Ok(false) => logger::log_event_complete(
                                                        &db_clone,
                                                        lid,
                                                        "failed",
                                                        dur,
                                                        Some("Provider returned false"),
                                                    )
                                                    .await
                                                    .ok(),
                                                    Err(e) => logger::log_event_complete(
                                                        &db_clone,
                                                        lid,
                                                        "failed",
                                                        dur,
                                                        Some(&e.to_string()),
                                                    )
                                                    .await
                                                    .ok(),
                                                };
                                            }

                                            // If retry indicates out-of-stock, fail fast and cleanup to avoid infinite booting.
                                            if let Err(e) = retry_res {
                                                let msg = e.to_string();
                                                if msg.contains("out_of_stock")
                                                    || msg.contains("Out of stock")
                                                {
                                                    let _ = sqlx::query(
                                                        "UPDATE instances
                                                         SET status = 'terminating',
                                                             error_code = COALESCE(error_code, 'PROVIDER_OUT_OF_STOCK'),
                                                             error_message = COALESCE($2, error_message),
                                                             failed_at = COALESCE(failed_at, NOW()),
                                                             deletion_reason = COALESCE(deletion_reason, 'provider_out_of_stock')
                                                         WHERE id = $1"
                                                    )
                                                    .bind(id)
                                                    .bind(&msg)
                                                    .execute(&db_clone)
                                                    .await;

                                                    // Best-effort terminate to avoid leaking a stopped server.
                                                    let term_log = logger::log_event_with_metadata(
                                                        &db_clone,
                                                        "PROVIDER_TERMINATE",
                                                        "in_progress",
                                                        id,
                                                        None,
                                                        Some(serde_json::json!({"zone": zone, "server_id": pid, "reason": "out_of_stock_cleanup"})),
                                                    )
                                                    .await
                                                    .ok();
                                                    let t0 = std::time::Instant::now();
                                                    let term_res = provider
                                                        .terminate_instance(&zone, pid)
                                                        .await;
                                                    if let Some(lid) = term_log {
                                                        let dur = t0.elapsed().as_millis() as i32;
                                                        match &term_res {
                                                            Ok(true) => logger::log_event_complete(
                                                                &db_clone, lid, "success", dur,
                                                                None,
                                                            )
                                                            .await
                                                            .ok(),
                                                            Ok(false) => {
                                                                logger::log_event_complete(
                                                                    &db_clone,
                                                                    lid,
                                                                    "failed",
                                                                    dur,
                                                                    Some("Provider returned false"),
                                                                )
                                                                .await
                                                                .ok()
                                                            }
                                                            Err(e) => logger::log_event_complete(
                                                                &db_clone,
                                                                lid,
                                                                "failed",
                                                                dur,
                                                                Some(&e.to_string()),
                                                            )
                                                            .await
                                                            .ok(),
                                                        };
                                                    }
                                                }
                                            }
                                        }
                                        return;
                                    }
                                    Err(e) => {
                                        if let Some(lid) = log_id {
                                            let dur = start.elapsed().as_millis() as i32;
                                            logger::log_event_complete(
                                                &db_clone,
                                                lid,
                                                "failed",
                                                dur,
                                                Some(&e.to_string()),
                                            )
                                            .await
                                            .ok();
                                        }
                                        return;
                                    }
                                }
                            }
                        }

                        check_and_transition_instance(
                            id,
                            ip,
                            boot_started_at,
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
