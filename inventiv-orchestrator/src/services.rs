use crate::finops_events;
use crate::logger;
use crate::provider_manager::ProviderManager;
use crate::worker_storage;
use bigdecimal::FromPrimitive;
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::fs;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

fn gb_to_bytes(gb: i64) -> i64 {
    // Scaleway APIs use bytes; use decimal GB.
    gb.saturating_mul(1_000_000_000)
}

fn worker_control_plane_url() -> String {
    // Priority:
    // 1) WORKER_CONTROL_PLANE_URL (direct)
    // 2) WORKER_CONTROL_PLANE_URL_FILE (read file contents)
    // 3) empty
    let direct = std::env::var("WORKER_CONTROL_PLANE_URL").unwrap_or_default();
    if !direct.trim().is_empty() {
        return direct.trim().to_string();
    }
    if let Ok(path) = std::env::var("WORKER_CONTROL_PLANE_URL_FILE") {
        let p = path.trim();
        if !p.is_empty() {
            if let Ok(contents) = std::fs::read_to_string(p) {
                let v = contents.trim();
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    String::new()
}

fn worker_hf_token() -> String {
    // Priority:
    // 1) WORKER_HF_TOKEN (direct; or common HF token env variants)
    // 2) WORKER_HF_TOKEN_FILE (read file contents)
    // 3) empty
    let direct = std::env::var("WORKER_HF_TOKEN")
        .or_else(|_| std::env::var("HUGGINGFACE_TOKEN"))
        .or_else(|_| std::env::var("HUGGING_FACE_HUB_TOKEN"))
        .or_else(|_| std::env::var("HUGGINGFACE_HUB_TOKEN"))
        .or_else(|_| std::env::var("HF_TOKEN"))
        .unwrap_or_default();
    let direct = direct.trim().to_string();
    if !direct.is_empty() {
        return direct;
    }
    if let Ok(path) = std::env::var("WORKER_HF_TOKEN_FILE") {
        let p = path.trim();
        if !p.is_empty() {
            if let Ok(contents) = std::fs::read_to_string(p) {
                let v = contents.trim();
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    String::new()
}

async fn resolve_instance_model_and_volume(
    db: &Pool<Postgres>,
    instance_id: Uuid,
) -> (Option<String>, Option<i64>) {
    let row: Option<(Option<String>, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT m.model_id, m.data_volume_gb
        FROM instances i
        LEFT JOIN models m ON m.id = i.model_id
        WHERE i.id = $1
        "#,
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);
    row.unwrap_or((None, None))
}

pub async fn process_termination(
    pool: Pool<Postgres>,
    _redis_client: redis::Client,
    instance_id: String,
    correlation_id: Option<String>,
) {
    let start = Instant::now();
    let id_uuid = match Uuid::parse_str(&instance_id) {
        Ok(v) => v,
        Err(e) => {
            println!(
                "❌ Invalid instance_id for termination '{}': {:?}",
                instance_id, e
            );
            return;
        }
    };
    let correlation_id_meta = correlation_id.clone();
    println!("⚙️ Processing Termination Async: {}", id_uuid);

    // LOG 1: EXECUTE_TERMINATE (orchestrator starts processing)
    let log_id_execute = logger::log_event_with_metadata(
        &pool,
        "EXECUTE_TERMINATE",
        "in_progress",
        id_uuid,
        None,
        Some(json!({
            "correlation_id": correlation_id_meta,
        })),
    )
    .await
    .ok();

    // 1. Get instance details from DB
    let row_result = sqlx::query_as::<_, (Option<String>, Option<String>, String)>(
        "SELECT i.provider_instance_id, z.code as zone, i.status::text
         FROM instances i
         LEFT JOIN zones z ON i.zone_id = z.id
         WHERE i.id = $1",
    )
    .bind(id_uuid)
    .fetch_optional(&pool)
    .await;

    match row_result {
        Ok(Some((provider_id_opt, zone_opt, current_status))) => {
            let zone = zone_opt.unwrap_or_default();
            println!(
                "🔍 Found instance {} (Zone: {}) Status: {}",
                provider_id_opt.as_deref().unwrap_or("None"),
                zone,
                current_status
            );

            // 2. Try to terminate on Provider
            if let Some(provider_instance_id) = provider_id_opt {
                // Select provider based on instance.provider_id (supports multiple providers)
                let provider_code: String = sqlx::query_scalar(
                    "SELECT p.code FROM providers p JOIN instances i ON i.provider_id = p.id WHERE i.id = $1",
                )
                .bind(id_uuid)
                .fetch_optional(&pool)
                .await
                .unwrap_or(None)
                .unwrap_or_else(|| ProviderManager::current_provider_name());

                if let Some(provider) = ProviderManager::get_provider(&provider_code, pool.clone())
                {
                    // LOG 2: PROVIDER_TERMINATE (API call to provider)
                    let api_start = Instant::now();
                    let log_id_provider = logger::log_event_with_metadata(
                        &pool,
                        "PROVIDER_TERMINATE",
                        "in_progress",
                        id_uuid,
                        None,
                        Some(json!({"zone": zone, "provider_instance_id": provider_instance_id, "correlation_id": correlation_id_meta})),
                    )
                    .await
                    .ok();

                    let result = provider
                        .terminate_instance(&zone, &provider_instance_id)
                        .await;

                    let termination_ok: bool = match &result {
                        Ok(true) => {
                            println!("✅ Successfully terminated instance on Provider");
                            if let Some(log_id) = log_id_provider {
                                let api_duration = api_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    log_id,
                                    "success",
                                    api_duration,
                                    None,
                                )
                                .await
                                .ok();
                            }
                            true
                        }
                        Ok(false) => {
                            let err_msg = "Provider termination call returned non-success status";
                            println!("⚠️ {}", err_msg);

                            if let Some(log_id) = log_id_provider {
                                let api_duration = api_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    log_id,
                                    "failed",
                                    api_duration,
                                    Some(err_msg),
                                )
                                .await
                                .ok();
                            }
                            if let Some(log_id) = log_id_execute {
                                let duration = start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    log_id,
                                    "failed",
                                    duration,
                                    Some(err_msg),
                                )
                                .await
                                .ok();
                            }
                            return;
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            if err_msg.contains("404") || err_msg.contains("not found") {
                                println!("⚠️ Instance not found on Provider (already deleted)");
                                // Still log as success since the end result is the same
                                if let Some(log_id) = log_id_provider {
                                    let api_duration = api_start.elapsed().as_millis() as i32;
                                    logger::log_event_complete(
                                        &pool,
                                        log_id,
                                        "success",
                                        api_duration,
                                        Some("Instance already deleted"),
                                    )
                                    .await
                                    .ok();
                                }
                                true
                            } else {
                                println!("⚠️ Error terminating on Provider: {:?}", e);
                                if let Some(log_id) = log_id_provider {
                                    let api_duration = api_start.elapsed().as_millis() as i32;
                                    logger::log_event_complete(
                                        &pool,
                                        log_id,
                                        "failed",
                                        api_duration,
                                        Some(&err_msg),
                                    )
                                    .await
                                    .ok();
                                }
                                // Don't proceed to mark as terminated if provider call failed
                                if let Some(log_id) = log_id_execute {
                                    let duration = start.elapsed().as_millis() as i32;
                                    logger::log_event_complete(
                                        &pool,
                                        log_id,
                                        "failed",
                                        duration,
                                        Some(&err_msg),
                                    )
                                    .await
                                    .ok();
                                }
                                return;
                            }
                        }
                    };

                    // Optional: delete attached volumes (if configured) after termination is accepted or already done.
                    if termination_ok {
                        let volumes: Vec<(Uuid, String, bool)> = sqlx::query_as(
                            r#"
                            SELECT id, provider_volume_id, delete_on_terminate
                            FROM instance_volumes
                            WHERE instance_id = $1
                              AND deleted_at IS NULL
                            "#,
                        )
                        .bind(id_uuid)
                        .fetch_all(&pool)
                        .await
                        .unwrap_or_default();

                        for (vol_row_id, provider_volume_id, delete_on_terminate) in volumes {
                            if !delete_on_terminate {
                                continue;
                            }
                            let log_id_vol = logger::log_event_with_metadata(
                                &pool,
                                "PROVIDER_DELETE_VOLUME",
                                "in_progress",
                                id_uuid,
                                None,
                                Some(json!({"zone": zone, "volume_id": provider_volume_id, "correlation_id": correlation_id_meta})),
                            )
                            .await
                            .ok();
                            let vol_start = Instant::now();
                            let del_res = provider.delete_volume(&zone, &provider_volume_id).await;
                            if let Some(lid) = log_id_vol {
                                let dur = vol_start.elapsed().as_millis() as i32;
                                match &del_res {
                                    Ok(true) => {
                                        logger::log_event_complete(&pool, lid, "success", dur, None)
                                            .await
                                            .ok()
                                    }
                                    Ok(false) => logger::log_event_complete(
                                        &pool,
                                        lid,
                                        "failed",
                                        dur,
                                        Some("Provider returned false"),
                                    )
                                    .await
                                    .ok(),
                                    Err(e) => logger::log_event_complete(
                                        &pool,
                                        lid,
                                        "failed",
                                        dur,
                                        Some(&e.to_string()),
                                    )
                                    .await
                                    .ok(),
                                };
                            }
                            if del_res.unwrap_or(false) {
                                let _ = sqlx::query(
                                    "UPDATE instance_volumes SET status='deleted', deleted_at=NOW() WHERE id=$1"
                                )
                                .bind(vol_row_id)
                                .execute(&pool)
                                .await;
                            }
                        }
                    }

                    // 2.5 Verify deletion (avoid marking terminated while still running)
                    // Scaleway termination is async; we poll for a short, bounded period.
                    let verify_start = Instant::now();
                    let mut deleted = false;
                    while verify_start.elapsed() < Duration::from_secs(60) {
                        match provider
                            .check_instance_exists(&zone, &provider_instance_id)
                            .await
                        {
                            Ok(false) => {
                                deleted = true;
                                break;
                            }
                            Ok(true) => {
                                sleep(Duration::from_secs(5)).await;
                            }
                            Err(e) => {
                                eprintln!("⚠️ Error checking deletion status on provider: {:?}", e);
                                // Keep waiting a bit; reconciliation watchdog will retry later if needed.
                                sleep(Duration::from_secs(5)).await;
                            }
                        }
                    }

                    if !deleted {
                        // Don't mark terminated in DB yet; keep 'terminating' until reconciliation confirms deletion.
                        let log_id_pending = logger::log_event_with_metadata(
                            &pool,
                            "TERMINATION_PENDING",
                            "in_progress",
                            id_uuid,
                            Some("Termination requested on provider; instance still exists (deletion in progress)"),
                        Some(json!({"zone": zone, "provider_instance_id": provider_instance_id, "waited_ms": verify_start.elapsed().as_millis(), "correlation_id": correlation_id_meta})),
                        ).await.ok();

                        if let Some(log_id) = log_id_pending {
                            let duration = verify_start.elapsed().as_millis() as i32;
                            logger::log_event_complete(&pool, log_id, "success", duration, None)
                                .await
                                .ok();
                        }

                        if let Some(log_id) = log_id_execute {
                            let duration = start.elapsed().as_millis() as i32;
                            logger::log_event_complete(
                                &pool,
                                log_id,
                                "success",
                                duration,
                                Some("Termination in progress (not yet deleted on provider)"),
                            )
                            .await
                            .ok();
                        }
                        return;
                    }
                } else {
                    println!("⚠️ Provider configuration missing or provider not found");
                    if let Some(log_id) = log_id_execute {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(
                            &pool,
                            log_id,
                            "failed",
                            duration,
                            Some("Provider not configured"),
                        )
                        .await
                        .ok();
                    }
                    return;
                }
            } else {
                println!("ℹ️ No provider_instance_id found, skipping Provider API call (just updating DB)");
            }

            // LOG 3: INSTANCE_TERMINATED (update DB)
            let db_start = Instant::now();
            let log_id_terminated =
                logger::log_event(&pool, "INSTANCE_TERMINATED", "in_progress", id_uuid, None)
                    .await
                    .ok();

            // 3. Update DB status to terminated
            let update_result = sqlx::query(
                "UPDATE instances SET status = 'terminated', terminated_at = NOW() WHERE id = $1",
            )
            .bind(id_uuid)
            .execute(&pool)
            .await;

            match update_result {
                Ok(_) => {
                    println!("✅ Instance {} marked as terminated in DB", id_uuid);

                    if let Some(log_id) = log_id_terminated {
                        let duration = db_start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "success", duration, None)
                            .await
                            .ok();
                    }

                    if let Some(log_id) = log_id_execute {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "success", duration, None)
                            .await
                            .ok();
                    }
                }
                Err(e) => {
                    println!("❌ Failed to update instance status in DB: {:?}", e);
                    let msg = format!("DB update failed: {:?}", e);

                    if let Some(log_id) = log_id_terminated {
                        let duration = db_start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                            .await
                            .ok();
                    }

                    if let Some(log_id) = log_id_execute {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                            .await
                            .ok();
                    }
                }
            }
        }
        Ok(None) => {
            println!("⚠️ Instance {} not found in DB for termination.", id_uuid);
            if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                logger::log_event_complete(
                    &pool,
                    log_id,
                    "failed",
                    duration,
                    Some("Instance not found"),
                )
                .await
                .ok();
            }
        }
        Err(e) => {
            println!("❌ Database Error during termination fetch: {:?}", e);
            if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                let msg = format!("DB error: {:?}", e);
                logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                    .await
                    .ok();
            }
        }
    }
}

pub async fn process_provisioning(
    pool: Pool<Postgres>,
    redis_client: redis::Client,
    instance_id: String,
    zone: String,
    instance_type: String,
    correlation_id: Option<String>,
) {
    let start = Instant::now();
    let instance_uuid = match Uuid::parse_str(&instance_id) {
        Ok(v) => v,
        Err(e) => {
            println!(
                "❌ Invalid instance_id for provisioning '{}': {:?}",
                instance_id, e
            );
            return;
        }
    };
    let correlation_id_meta = correlation_id.clone();
    println!(
        "🔨 [Orchestrator] Processing Provision for instance: {}",
        instance_uuid
    );

    // 0. Resolve provider from the instance row (supports multiple providers)
    // No hardcoded UUID fallbacks: the DB catalog must contain the provider referenced by the instance.
    let provider_id: Uuid =
        match sqlx::query_scalar::<_, Uuid>("SELECT provider_id FROM instances WHERE id = $1")
            .bind(instance_uuid)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None)
        {
            Some(v) => v,
            None => {
                println!(
                    "❌ Error: instance {} not found (cannot resolve provider_id).",
                    instance_uuid
                );
                return;
            }
        };

    let provider_name: String = sqlx::query_scalar("SELECT code FROM providers WHERE id = $1")
        .bind(provider_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| ProviderManager::current_provider_name());

    let zone_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT z.id
        FROM zones z
        JOIN regions r ON r.id = z.region_id
        WHERE z.code = $1
          AND z.is_active = true
          AND r.provider_id = $2
        LIMIT 1
        "#,
    )
    .bind(&zone)
    .bind(provider_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let type_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM instance_types WHERE code = $1 AND provider_id = $2 AND is_active = true",
    )
    .bind(&instance_type)
    .bind(provider_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if zone_id.is_none() || type_id.is_none() {
        println!(
            "❌ Error: Zone '{}' or Type '{}' not found in catalog.",
            zone, instance_type
        );
        let _msg = format!(
            "Catalog lookup failed: Zone={} Type={}",
            zone, instance_type
        );
        sqlx::query("UPDATE instances SET status = 'failed' WHERE id = $1")
            .bind(instance_uuid)
            .execute(&pool)
            .await
            .ok();
        // TODO: Log failure
        return;
    }
    let zone_id = zone_id.unwrap();
    let type_id = type_id.unwrap();

    // Guardrails for worker auto-install (Scaleway): prevent provisioning unsupported/unavailable types.
    let auto_install_guard = std::env::var("WORKER_AUTO_INSTALL")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    if auto_install_guard && provider_name.to_ascii_lowercase() == "scaleway" {
        let is_available: bool = sqlx::query_scalar(
            r#"
            SELECT COALESCE(itz.is_available, false)
            FROM instance_type_zones itz
            WHERE itz.instance_type_id = $1
              AND itz.zone_id = $2
            "#,
        )
        .bind(type_id)
        .bind(zone_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None)
        .unwrap_or(false);

        if !is_available {
            let msg = "Instance type is not available in this zone (catalog)".to_string();
            let _ = sqlx::query(
                "UPDATE instances
                 SET status='failed',
                     error_code=COALESCE(error_code,'INSTANCE_TYPE_NOT_AVAILABLE_IN_ZONE'),
                     error_message=COALESCE($2,error_message),
                     failed_at=COALESCE(failed_at,NOW())
                 WHERE id=$1",
            )
            .bind(instance_uuid)
            .bind(&msg)
            .execute(&pool)
            .await;
            eprintln!("❌ {}", msg);
            return;
        }

        let patterns = inventiv_common::worker_target::parse_instance_type_patterns(
            std::env::var("WORKER_AUTO_INSTALL_INSTANCE_PATTERNS")
                .ok()
                .as_deref(),
        );
        let is_supported = inventiv_common::worker_target::instance_type_matches_patterns(
            &instance_type,
            &patterns,
        );
        if !is_supported {
            let msg = format!(
                "Instance type '{}' not supported for worker auto-install (patterns={:?})",
                instance_type, patterns
            );
            let _ = sqlx::query(
                "UPDATE instances
                 SET status='failed',
                     error_code=COALESCE(error_code,'INSTANCE_TYPE_NOT_SUPPORTED'),
                     error_message=COALESCE($2,error_message),
                     failed_at=COALESCE(failed_at,NOW())
                 WHERE id=$1",
            )
            .bind(instance_uuid)
            .bind(&msg)
            .execute(&pool)
            .await;
            eprintln!("❌ {}", msg);
            return;
        }
    }

    // 0.5. Ensure row exists (idempotent; do NOT regress status on retries)
    let insert_result = sqlx::query(
         "INSERT INTO instances (id, provider_id, zone_id, instance_type_id, status, created_at, gpu_profile)
          VALUES ($1, $2, $3, $4, 'provisioning', NOW(), '{}')
          ON CONFLICT (id) DO NOTHING"
    )
    .bind(instance_uuid)
    .bind(provider_id)
    .bind(zone_id)
    .bind(type_id)
    .execute(&pool)
    .await;

    if let Err(e) = insert_result {
        println!("❌ Initial DB Insert Error: {:?}", e);
        return;
    }

    // LOG 2: EXECUTE_CREATE
    let log_id_execute = logger::log_event_with_metadata(
        &pool,
        "EXECUTE_CREATE",
        "in_progress",
        instance_uuid,
        None,
        Some(json!({
            "zone": zone,
            "instance_type": instance_type,
            "correlation_id": correlation_id_meta,
        })),
    )
    .await
    .ok();

    // Model is mandatory (request must define which model to install).
    // Safety net: even if API validation is bypassed, provisioning should not proceed without it.
    let (model_from_db, _vol_from_db) =
        resolve_instance_model_and_volume(&pool, instance_uuid).await;
    if model_from_db.is_none() {
        let msg = "Missing model for instance (instances.model_id is NULL)";
        eprintln!("❌ {}", msg);
        let _ = sqlx::query(
            "UPDATE instances
             SET status='failed',
                 error_code=COALESCE(error_code,'MISSING_MODEL'),
                 error_message=COALESCE($2,error_message),
                 failed_at=COALESCE(failed_at,NOW())
             WHERE id=$1",
        )
        .bind(instance_uuid)
        .bind(msg)
        .execute(&pool)
        .await;
        if let Some(log_id) = log_id_execute {
            let duration = start.elapsed().as_millis() as i32;
            logger::log_event_complete(&pool, log_id, "failed", duration, Some(msg))
                .await
                .ok();
        }
        return;
    }

    // 1. Init Provider
    let provider_opt = ProviderManager::get_provider(&provider_name, pool.clone());

    if provider_opt.is_none() {
        let msg = "Missing Provider Credentials";
        println!("❌ Error: {}", msg);
        if let Some(log_id) = log_id_execute {
            let duration = start.elapsed().as_millis() as i32;
            logger::log_event_complete(&pool, log_id, "failed", duration, Some(msg))
                .await
                .ok();
        }
        sqlx::query("UPDATE instances SET status = 'failed' WHERE id = $1")
            .bind(instance_uuid)
            .execute(&pool)
            .await
            .ok();
        return;
    }
    let provider = provider_opt.unwrap();

    // 1.5 Idempotence guard: if provider_instance_id already exists, don't create a second server
    let existing: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT provider_instance_id, status::text FROM instances WHERE id = $1")
            .bind(instance_uuid)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten();

    if let Some((Some(existing_server_id), status_opt)) = existing {
        let status = status_opt.unwrap_or_default();
        println!(
            "♻️ [Orchestrator] Provision idempotence: instance already has provider_instance_id={} (status={}), skipping create",
            existing_server_id, status
        );

        // Best-effort: refresh IP and ensure it's in booting (unless already beyond)
        let mut ip_address: Option<String> = None;
        for attempt in 1..=5 {
            match provider.get_instance_ip(&zone, &existing_server_id).await {
                Ok(Some(ip)) => {
                    ip_address = Some(ip);
                    break;
                }
                Ok(None) => {
                    if attempt < 5 {
                        sleep(Duration::from_secs(2)).await;
                    }
                }
                Err(_) => break,
            }
        }

        let _ = sqlx::query(
            "UPDATE instances
             SET ip_address = COALESCE($1::inet, ip_address),
                 status = CASE
                   WHEN status IN ('provisioning', 'booting') THEN 'booting'
                   ELSE status
                 END
             WHERE id = $2",
        )
        .bind(ip_address)
        .bind(instance_uuid)
        .execute(&pool)
        .await;

        if let Some(log_id) = log_id_execute {
            let duration = start.elapsed().as_millis() as i32;
            logger::log_event_complete(
                &pool,
                log_id,
                "success",
                duration,
                Some("Idempotent retry: provider server already exists"),
            )
            .await
            .ok();
        }
        return;
    }

    // 2. Create Server
    //
    // NOTE: Some provider + instance type combos require extra allocation parameters
    // (disk profile, boot image, security group, etc.).
    //
    // Scaleway: some GPU instance families (e.g. L4, L40S) require *no local volumes* (0GB).
    // In practice, this means the boot image must be compatible ("diskless") otherwise the
    // server can be created but will fail at poweron with:
    //   precondition_failed: resource_not_usable / local-volume(s) must be equal to 0GB
    fn scaleway_requires_diskless_boot_image(instance_type: &str) -> bool {
        let t = instance_type.trim().to_ascii_uppercase();
        t.starts_with("L4-") || t.starts_with("L40S-")
    }
    let mut image_id = "8e0da557-5d75-40ba-b928-5984075aa255".to_string();

    // Provider-specific image override (e.g. GPU-optimized images).
    // Expected: instance_types.allocation_params = {"scaleway":{"image_id":"<uuid>"}}.
    if provider_name.to_ascii_lowercase() == "scaleway" {
        let override_image: Option<String> = sqlx::query_scalar(
            r#"
            SELECT NULLIF(TRIM(it.allocation_params->'scaleway'->>'image_id'), '')
            FROM instance_types it
            WHERE it.id = $1
            "#,
        )
        .bind(type_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);
        if let Some(img) = override_image {
            image_id = img;
        }
    }

    if provider_name.to_ascii_lowercase() == "scaleway"
        && scaleway_requires_diskless_boot_image(&instance_type)
    {
        // Prefer a provider-specific diskless boot image configured on the instance type.
        // Expected shape: instance_types.allocation_params = {"scaleway": {"boot_image_id": "<uuid>" }}
        let configured: Option<String> = sqlx::query_scalar(
            r#"
            SELECT NULLIF(TRIM(it.allocation_params->'scaleway'->>'boot_image_id'), '')
            FROM instance_types it
            WHERE it.id = $1
            "#,
        )
        .bind(type_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

        if let Some(img) = configured {
            // Diskless override has priority.
            image_id = img;
        } else {
            // Try provider-side auto-discovery.
            if let Some(provider) = ProviderManager::get_provider(&provider_name, pool.clone()) {
                match provider.resolve_boot_image(&zone, &instance_type).await {
                    Ok(Some(img)) => {
                        println!(
                            "ℹ️ Scaleway diskless: auto-resolved boot image '{}' for zone '{}' (type '{}')",
                            img, zone, instance_type
                        );
                        image_id = img;

                        // Make subsequent provisions deterministic: persist the resolved boot image
                        // onto the instance type allocation_params if it isn't set already.
                        let _ = sqlx::query(
                            r#"
                            UPDATE instance_types
                            SET allocation_params =
                                jsonb_set(
                                    allocation_params,
                                    '{scaleway}',
                                    COALESCE(allocation_params->'scaleway', '{}'::jsonb)
                                      || jsonb_build_object('boot_image_id', to_jsonb($2::text)),
                                    true
                                )
                            WHERE id = $1
                              AND NULLIF(TRIM(allocation_params->'scaleway'->>'boot_image_id'), '') IS NULL
                            "#,
                        )
                        .bind(type_id)
                        .bind(&image_id)
                        .execute(&pool)
                        .await;
                    }
                    Ok(None) => {
                        let msg = "Scaleway requires a diskless/compatible boot image for this instance type. Auto-discovery did not find a suitable image. Configure instance_types.allocation_params.scaleway.boot_image_id for this type.".to_string();
                        eprintln!("❌ {}", msg);
                        if let Some(log_id) = log_id_execute {
                            let duration = start.elapsed().as_millis() as i32;
                            logger::log_event_complete(
                                &pool,
                                log_id,
                                "failed",
                                duration,
                                Some(&msg),
                            )
                            .await
                            .ok();
                        }
                        let _ = sqlx::query(
                            "UPDATE instances
                             SET status = 'failed',
                                 error_code = COALESCE(error_code, 'SCW_DISKLESS_BOOT_IMAGE_REQUIRED'),
                                 error_message = COALESCE($2, error_message),
                                 failed_at = COALESCE(failed_at, NOW())
                             WHERE id = $1"
                        )
                        .bind(instance_uuid)
                        .bind(&msg)
                        .execute(&pool)
                        .await;
                        return;
                    }
                    Err(e) => {
                        let msg =
                            format!("Scaleway diskless boot image auto-discovery failed: {}", e);
                        eprintln!("❌ {}", msg);
                        if let Some(log_id) = log_id_execute {
                            let duration = start.elapsed().as_millis() as i32;
                            logger::log_event_complete(
                                &pool,
                                log_id,
                                "failed",
                                duration,
                                Some(&msg),
                            )
                            .await
                            .ok();
                        }
                        let _ = sqlx::query(
                            "UPDATE instances
                             SET status = 'failed',
                                 error_code = COALESCE(error_code, 'SCW_DISKLESS_BOOT_IMAGE_RESOLVE_FAILED'),
                                 error_message = COALESCE($2, error_message),
                                 failed_at = COALESCE(failed_at, NOW())
                             WHERE id = $1"
                        )
                        .bind(instance_uuid)
                        .bind(&msg)
                        .execute(&pool)
                        .await;
                        return;
                    }
                }
            } else {
                let msg = "Missing Provider Credentials".to_string();
                eprintln!("❌ {}", msg);
                if let Some(log_id) = log_id_execute {
                    let duration = start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                        .await
                        .ok();
                }
                let _ = sqlx::query("UPDATE instances SET status = 'failed', error_message = COALESCE($2, error_message), failed_at = COALESCE(failed_at, NOW()) WHERE id = $1")
                    .bind(instance_uuid)
                    .bind(&msg)
                    .execute(&pool)
                    .await;
                return;
            }
        }
    }

    // LOG 3: PROVIDER_CREATE (API call)
    let api_start = Instant::now();
    let log_id_provider = logger::log_event_with_metadata(
        &pool, "PROVIDER_CREATE", "in_progress", instance_uuid, None,
        Some(json!({"zone": zone, "instance_type": instance_type, "image_id": image_id, "correlation_id": correlation_id_meta})),
    ).await.ok();

    // Optional: configure worker auto-install at boot (cloud-init) for Scaleway.
    //
    // Controlled by orchestrator env vars:
    // - WORKER_AUTO_INSTALL=1
    // - WORKER_CONTROL_PLANE_URL=https://api.<domain> (or tunnel URL for DEV local-to-cloud)
    // - WORKER_AUTO_INSTALL_INSTANCE_PATTERNS=L4-*,L40S-*,RENDER-S (optional; defaults to L4-*,L40S-*,RENDER-S)
    // - WORKER_MODEL_ID=Qwen/Qwen2.5-0.5B-Instruct (default)
    // - WORKER_VLLM_IMAGE=vllm/vllm-openai:latest (default)
    // - WORKER_AGENT_SOURCE_URL=<raw github url> (default to main)
    let auto_install = std::env::var("WORKER_AUTO_INSTALL")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    let is_scaleway = provider_name.to_ascii_lowercase() == "scaleway";

    let patterns = inventiv_common::worker_target::parse_instance_type_patterns(
        std::env::var("WORKER_AUTO_INSTALL_INSTANCE_PATTERNS")
            .ok()
            .as_deref(),
    );
    let is_worker_target =
        inventiv_common::worker_target::instance_type_matches_patterns(&instance_type, &patterns);

    let cp_url = worker_control_plane_url();
    let cp_url = cp_url.trim().trim_end_matches('/').to_string();

    // Include SSH key for debugging (same one used by provisioning).
    let ssh_pub_path = std::env::var("SCALEWAY_SSH_PUBLIC_KEY_FILE")
        .unwrap_or_else(|_| "/app/.ssh/llm-studio-key.pub".to_string());
    let ssh_pub = fs::read_to_string(&ssh_pub_path)
        .ok()
        .map(|s| s.trim().replace('\n', " "))
        .unwrap_or_default();

    let cloud_init_for_create: Option<String> = if is_scaleway {
        if auto_install && is_worker_target {
            if cp_url.is_empty() {
                eprintln!("⚠️ WORKER_AUTO_INSTALL=1 but WORKER_CONTROL_PLANE_URL is empty; creating server without worker bootstrap");
                if ssh_pub.trim().is_empty() {
                    None
                } else {
                    Some(build_ssh_key_cloud_init(&ssh_pub))
                }
            } else {
                let (model_from_db, _vol_from_db) =
                    resolve_instance_model_and_volume(&pool, instance_uuid).await;
                // model is mandatory; do not fallback silently here
                let worker_model =
                    model_from_db.expect("model is mandatory (validated before provisioning)");

                let provider_id: Option<Uuid> =
                    sqlx::query_scalar("SELECT provider_id FROM instances WHERE id = $1")
                        .bind(instance_uuid)
                        .fetch_optional(&pool)
                        .await
                        .unwrap_or(None);

                let vllm_image = if let Some(pid) = provider_id {
                    sqlx::query_scalar("SELECT value_text FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_IMAGE'")
                        .bind(pid)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten()
                        .filter(|s: &String| !s.trim().is_empty())
                        .or_else(|| std::env::var("WORKER_VLLM_IMAGE").ok().filter(|s| !s.trim().is_empty()))
                        .unwrap_or_else(|| "vllm/vllm-openai:latest".to_string())
                } else {
                    std::env::var("WORKER_VLLM_IMAGE")
                        .ok()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| "vllm/vllm-openai:latest".to_string())
                };

                let worker_health_port: u16 = if let Some(pid) = provider_id {
                    sqlx::query_scalar("SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_HEALTH_PORT'")
                        .bind(pid)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|v| u16::try_from(v).ok())
                        .or_else(|| std::env::var("WORKER_HEALTH_PORT").ok().and_then(|s| s.parse::<u16>().ok()))
                        .unwrap_or(8080)
                } else {
                    std::env::var("WORKER_HEALTH_PORT")
                        .ok()
                        .and_then(|s| s.parse::<u16>().ok())
                        .unwrap_or(8080)
                };
                let worker_vllm_port: u16 = if let Some(pid) = provider_id {
                    sqlx::query_scalar("SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_PORT'")
                        .bind(pid)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|v| u16::try_from(v).ok())
                        .or_else(|| std::env::var("WORKER_VLLM_PORT").ok().and_then(|s| s.parse::<u16>().ok()))
                        .unwrap_or(8000)
                } else {
                    std::env::var("WORKER_VLLM_PORT")
                        .ok()
                        .and_then(|s| s.parse::<u16>().ok())
                        .unwrap_or(8000)
                };

                let agent_url = std::env::var("WORKER_AGENT_SOURCE_URL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "https://raw.githubusercontent.com/Inventiv-IT-for-AI/inventiv-agents/main/inventiv-worker/agent.py".to_string());

                let worker_auth_token = std::env::var("WORKER_AUTH_TOKEN").unwrap_or_default();
                let worker_hf_token = worker_hf_token();

                Some(build_worker_cloud_init(
                    &ssh_pub,
                    &instance_uuid.to_string(),
                    &cp_url,
                    &worker_model,
                    &vllm_image,
                    worker_vllm_port,
                    worker_health_port,
                    &agent_url,
                    &worker_auth_token,
                    &worker_hf_token,
                ))
            }
        } else if !ssh_pub.trim().is_empty() {
            Some(build_ssh_key_cloud_init(&ssh_pub))
        } else {
            None
        }
    } else {
        None
    };

    let server_id_result = provider
        .create_instance(
            &zone,
            &instance_type,
            &image_id,
            cloud_init_for_create.as_deref(),
        )
        .await;

    match server_id_result {
        Ok(server_id) => {
            println!("✅ Server Created: {}", server_id);

            if let Some(log_id) = log_id_provider {
                let api_duration = api_start.elapsed().as_millis() as i32;
                let metadata = json!({"server_id": server_id, "zone": zone, "correlation_id": correlation_id_meta});
                logger::log_event_complete_with_metadata(
                    &pool,
                    log_id,
                    "success",
                    api_duration,
                    None,
                    Some(metadata),
                )
                .await
                .ok();
            }

            // Persist provider_instance_id immediately (prevents "stuck provisioning with no server_id" on later failures/hangs)
            let persist_start = Instant::now();
            let log_id_persist = logger::log_event_with_metadata(
                &pool,
                "PERSIST_PROVIDER_ID",
                "in_progress",
                instance_uuid,
                None,
                Some(json!({"server_id": server_id, "zone": zone, "correlation_id": correlation_id_meta})),
            ).await.ok();

            let persist_res = sqlx::query(
                "UPDATE instances
                 SET provider_instance_id = COALESCE(provider_instance_id, $1)
                 WHERE id = $2",
            )
            .bind(&server_id)
            .bind(instance_uuid)
            .execute(&pool)
            .await;

            if let Some(lid) = log_id_persist {
                let dur = persist_start.elapsed().as_millis() as i32;
                match &persist_res {
                    Ok(_) => logger::log_event_complete(&pool, lid, "success", dur, None)
                        .await
                        .ok(),
                    Err(e) => logger::log_event_complete(
                        &pool,
                        lid,
                        "failed",
                        dur,
                        Some(&format!("DB persist failed: {:?}", e)),
                    )
                    .await
                    .ok(),
                };
            }
            if let Err(e) = persist_res {
                // If we can't persist server_id, better fail fast to avoid an untraceable leak.
                let msg = format!(
                    "Failed to persist provider_instance_id after create: {:?}",
                    e
                );
                if let Some(log_id) = log_id_execute {
                    let duration = start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                        .await
                        .ok();
                }
                return;
            }

            // Optional: open ports so the control-plane (and dev laptop) can reach:
            // - vLLM: 8000
            // - worker health: 8080
            //
            // Without this, Scaleway security groups may block inbound traffic by default,
            // which makes `curl http://<ip>:8000/...` fail even if the worker is running.
            if auto_install && is_scaleway && is_worker_target {
                let provider_id: Option<Uuid> =
                    sqlx::query_scalar("SELECT provider_id FROM instances WHERE id = $1")
                        .bind(instance_uuid)
                        .fetch_optional(&pool)
                        .await
                        .unwrap_or(None);
                let expose = if let Some(pid) = provider_id {
                    sqlx::query_scalar("SELECT value_bool FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_EXPOSE_PORTS'")
                        .bind(pid)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| {
                            std::env::var("WORKER_EXPOSE_PORTS")
                                .ok()
                                .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
                                .unwrap_or(true)
                        })
                } else {
                    std::env::var("WORKER_EXPOSE_PORTS")
                        .ok()
                        .map(|v| {
                            matches!(
                                v.trim().to_ascii_lowercase().as_str(),
                                "1" | "true" | "yes" | "on"
                            )
                        })
                        .unwrap_or(true)
                };
                if expose {
                    let worker_health_port: u16 = if let Some(pid) = provider_id {
                        sqlx::query_scalar("SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_HEALTH_PORT'")
                            .bind(pid)
                            .fetch_optional(&pool)
                            .await
                            .ok()
                            .flatten()
                            .and_then(|v| u16::try_from(v).ok())
                            .or_else(|| std::env::var("WORKER_HEALTH_PORT").ok().and_then(|s| s.parse::<u16>().ok()))
                            .unwrap_or(8080)
                    } else {
                        std::env::var("WORKER_HEALTH_PORT")
                            .ok()
                            .and_then(|s| s.parse::<u16>().ok())
                            .unwrap_or(8080)
                    };
                    let worker_vllm_port: u16 = if let Some(pid) = provider_id {
                        sqlx::query_scalar("SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_PORT'")
                            .bind(pid)
                            .fetch_optional(&pool)
                            .await
                            .ok()
                            .flatten()
                            .and_then(|v| u16::try_from(v).ok())
                            .or_else(|| std::env::var("WORKER_VLLM_PORT").ok().and_then(|s| s.parse::<u16>().ok()))
                            .unwrap_or(8000)
                    } else {
                        std::env::var("WORKER_VLLM_PORT")
                            .ok()
                            .and_then(|s| s.parse::<u16>().ok())
                            .unwrap_or(8000)
                    };
                    match provider
                        .ensure_inbound_tcp_ports(
                            &zone,
                            &server_id,
                            vec![worker_vllm_port as i32, worker_health_port as i32],
                        )
                        .await
                    {
                        Ok(true) => println!(
                            "🔓 worker ports opened on security group ({}/{})",
                            worker_vllm_port, worker_health_port
                        ),
                        Ok(false) => eprintln!(
                            "⚠️ provider does not support ensure_inbound_tcp_ports (skipped)"
                        ),
                        Err(e) => eprintln!("⚠️ failed to open worker ports: {}", e),
                    }
                }
            }

            // Optional: create + attach a data volume (SBS) based on instance type allocation params.
            // allocation_params shape:
            // {
            //   "<provider_code>": {
            //     "data_volume_gb": 200,
            //     "data_volume_perf_iops": 5000,
            //     "data_volume_delete_on_terminate": true
            //   }
            // }
            let data_conf_row: Option<(Option<i64>, Option<i32>, bool)> = sqlx::query_as(
                r#"
                SELECT
                  NULLIF(TRIM(it.allocation_params->($2::text)->>'data_volume_gb'), '')::bigint AS gb,
                  NULLIF(TRIM(it.allocation_params->($2::text)->>'data_volume_perf_iops'), '')::int AS perf,
                  COALESCE((it.allocation_params->($2::text)->>'data_volume_delete_on_terminate')::bool, TRUE) AS del
                FROM instance_types it
                WHERE it.id = $1
                "#,
            )
            .bind(type_id)
            .bind(&provider_name)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None)
            ;

            let mut data_conf: Option<(i64, Option<i32>, bool)> = data_conf_row
                .and_then(|(gb_opt, perf_opt, del)| gb_opt.map(|gb| (gb, perf_opt, del)));

            // Fallback: if instance type doesn't specify a data volume, infer a safe size from the model.
            // This helps prevent "no space left on device" during docker + image + model pulls on diskless GPUs.
            if data_conf.is_none() && auto_install && is_scaleway && is_worker_target {
                let (model_from_db, vol_from_db) =
                    resolve_instance_model_and_volume(&pool, instance_uuid).await;

                if let Some(gb) = vol_from_db.filter(|gb| *gb > 0) {
                    data_conf = Some((gb, None, true));
                } else {
                    // model is mandatory; do not fallback silently here
                    let worker_model =
                        model_from_db.expect("model is mandatory (validated before provisioning)");
                    let provider_id: Option<Uuid> =
                        sqlx::query_scalar("SELECT provider_id FROM instances WHERE id = $1")
                            .bind(instance_uuid)
                            .fetch_optional(&pool)
                            .await
                            .unwrap_or(None);
                    let default_gb: i64 = if let Some(pid) = provider_id {
                        sqlx::query_scalar("SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_DATA_VOLUME_GB_DEFAULT'")
                            .bind(pid)
                            .fetch_optional(&pool)
                            .await
                            .ok()
                            .flatten()
                            .unwrap_or_else(|| {
                                std::env::var("WORKER_DATA_VOLUME_GB_DEFAULT")
                                    .ok()
                                    .and_then(|v| v.trim().parse::<i64>().ok())
                                    .filter(|gb| *gb > 0)
                                    .unwrap_or(200)
                            })
                    } else {
                        std::env::var("WORKER_DATA_VOLUME_GB_DEFAULT")
                            .ok()
                            .and_then(|v| v.trim().parse::<i64>().ok())
                            .filter(|gb| *gb > 0)
                            .unwrap_or(200)
                    };
                    if let Some(gb) =
                        worker_storage::recommended_data_volume_gb(&worker_model, default_gb)
                    {
                        data_conf = Some((gb, None, true));
                    }
                }
            }

            if let Some((gb, perf_iops, delete_on_terminate)) = data_conf {
                if gb > 0 {
                    let vol_name = format!("inventiv-data-{}", instance_uuid);
                    let create_log = logger::log_event_with_metadata(
                        &pool,
                        "PROVIDER_CREATE_VOLUME",
                        "in_progress",
                        instance_uuid,
                        None,
                        Some(json!({"zone": zone, "server_id": server_id, "name": vol_name, "size_gb": gb, "correlation_id": correlation_id_meta})),
                    )
                    .await
                    .ok();
                    let vol_start = Instant::now();
                    let created = provider
                        .create_volume(&zone, &vol_name, gb_to_bytes(gb), "sbs_volume", perf_iops)
                        .await;
                    let vol_id = match created {
                        Ok(Some(id)) => id,
                        Ok(None) => {
                            let msg = "Provider does not support volume creation".to_string();
                            if let Some(lid) = create_log {
                                let dur = vol_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(&pool, lid, "failed", dur, Some(&msg))
                                    .await
                                    .ok();
                            }
                            // Don't fail provisioning for providers without volume support.
                            String::new()
                        }
                        Err(e) => {
                            let msg = format!("Failed to create data volume: {}", e);
                            if let Some(lid) = create_log {
                                let dur = vol_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(&pool, lid, "failed", dur, Some(&msg))
                                    .await
                                    .ok();
                            }
                            // Cleanup created server to avoid leak.
                            let _ = provider.terminate_instance(&zone, &server_id).await;
                            let _ = sqlx::query(
                                "UPDATE instances SET status='failed', error_code=COALESCE(error_code,'PROVIDER_VOLUME_CREATE_FAILED'), error_message=COALESCE($2,error_message), failed_at=COALESCE(failed_at,NOW()) WHERE id=$1"
                            )
                            .bind(instance_uuid)
                            .bind(&msg)
                            .execute(&pool)
                            .await;
                            if let Some(log_id) = log_id_execute {
                                let duration = start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    log_id,
                                    "failed",
                                    duration,
                                    Some(&msg),
                                )
                                .await
                                .ok();
                            }
                            return;
                        }
                    };
                    if !vol_id.is_empty() {
                        if let Some(lid) = create_log {
                            let dur = vol_start.elapsed().as_millis() as i32;
                            logger::log_event_complete(&pool, lid, "success", dur, None)
                                .await
                                .ok();
                        }

                        let row_id = Uuid::new_v4();
                        let _ = sqlx::query(
                            "INSERT INTO instance_volumes (id, instance_id, provider_id, zone_code, provider_volume_id, volume_type, size_bytes, perf_iops, delete_on_terminate, status, attached_at)
                             VALUES ($1,$2,$3,$4,$5,'sbs_volume',$6,$7,$8,'creating',NULL)"
                        )
                        .bind(row_id)
                        .bind(instance_uuid)
                        .bind(provider_id)
                        .bind(&zone)
                        .bind(&vol_id)
                        .bind(gb_to_bytes(gb))
                        .bind(perf_iops)
                        .bind(delete_on_terminate)
                        .execute(&pool)
                        .await;

                        let attach_log = logger::log_event_with_metadata(
                            &pool,
                            "PROVIDER_ATTACH_VOLUME",
                            "in_progress",
                            instance_uuid,
                            None,
                            Some(json!({"zone": zone, "server_id": server_id, "volume_id": vol_id, "correlation_id": correlation_id_meta})),
                        )
                        .await
                        .ok();
                        let attach_start = Instant::now();
                        let attach_res = provider.attach_volume(&zone, &server_id, &vol_id).await;
                        if let Some(lid) = attach_log {
                            let dur = attach_start.elapsed().as_millis() as i32;
                            match &attach_res {
                                Ok(true) => {
                                    logger::log_event_complete(&pool, lid, "success", dur, None)
                                        .await
                                        .ok()
                                }
                                Ok(false) => logger::log_event_complete(
                                    &pool,
                                    lid,
                                    "failed",
                                    dur,
                                    Some("Provider returned false"),
                                )
                                .await
                                .ok(),
                                Err(e) => logger::log_event_complete(
                                    &pool,
                                    lid,
                                    "failed",
                                    dur,
                                    Some(&e.to_string()),
                                )
                                .await
                                .ok(),
                            };
                        }
                        if attach_res.unwrap_or(false) {
                            let _ = sqlx::query(
                                "UPDATE instance_volumes SET status='attached', attached_at=NOW() WHERE id=$1"
                            )
                            .bind(row_id)
                            .execute(&pool)
                            .await;
                        } else {
                            let msg = "Failed to attach data volume".to_string();
                            let _ = sqlx::query("UPDATE instance_volumes SET status='failed', error_message=$2 WHERE id=$1")
                                .bind(row_id)
                                .bind(&msg)
                                .execute(&pool)
                                .await;
                            // Best-effort cleanup of the created volume to avoid cost leak.
                            let _ = provider.delete_volume(&zone, &vol_id).await;
                            let _ = sqlx::query(
                                "UPDATE instance_volumes SET status='deleted', deleted_at=NOW() WHERE id=$1"
                            )
                            .bind(row_id)
                            .execute(&pool)
                            .await;
                            // Cleanup server to avoid leak
                            let _ = provider.terminate_instance(&zone, &server_id).await;
                            let _ = sqlx::query(
                                "UPDATE instances SET status='failed', error_code=COALESCE(error_code,'PROVIDER_VOLUME_ATTACH_FAILED'), error_message=COALESCE($2,error_message), failed_at=COALESCE(failed_at,NOW()) WHERE id=$1"
                            )
                            .bind(instance_uuid)
                            .bind(&msg)
                            .execute(&pool)
                            .await;
                            if let Some(log_id) = log_id_execute {
                                let duration = start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    log_id,
                                    "failed",
                                    duration,
                                    Some(&msg),
                                )
                                .await
                                .ok();
                            }
                            return;
                        }
                    }
                }
            }

            // LOG 3.1: PROVIDER_START (API call)
            let start_api = Instant::now();
            let log_id_start = logger::log_event_with_metadata(
                &pool,
                "PROVIDER_START",
                "in_progress",
                instance_uuid,
                None,
                Some(json!({"zone": zone, "server_id": server_id, "image_id": image_id, "correlation_id": correlation_id_meta})),
            ).await.ok();

            // 3. Power On (fail-fast if provider rejects)
            if let Err(e) = provider.start_instance(&zone, &server_id).await {
                let msg = format!("Failed to start instance on provider: {:?}", e);
                println!("❌ {}", msg);
                if let Some(lid) = log_id_start {
                    let duration = start_api.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, lid, "failed", duration, Some(&msg))
                        .await
                        .ok();
                }
                if let Some(log_id) = log_id_execute {
                    let duration = start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                        .await
                        .ok();
                }

                // Best-effort cleanup: if create succeeded but start failed, we must not leak provider resources.
                let terminate_log = logger::log_event_with_metadata(
                    &pool,
                    "PROVIDER_TERMINATE",
                    "in_progress",
                    instance_uuid,
                    None,
                    Some(json!({"zone": zone, "server_id": server_id, "correlation_id": correlation_id_meta, "reason": "start_failed_cleanup"})),
                )
                .await
                .ok();
                let terminate_start = Instant::now();
                let terminate_res = provider.terminate_instance(&zone, &server_id).await;
                if let Some(lid) = terminate_log {
                    let dur = terminate_start.elapsed().as_millis() as i32;
                    match &terminate_res {
                        Ok(true) => logger::log_event_complete(&pool, lid, "success", dur, None)
                            .await
                            .ok(),
                        Ok(false) => logger::log_event_complete(
                            &pool,
                            lid,
                            "failed",
                            dur,
                            Some("Provider terminate returned false"),
                        )
                        .await
                        .ok(),
                        Err(err) => logger::log_event_complete(
                            &pool,
                            lid,
                            "failed",
                            dur,
                            Some(&err.to_string()),
                        )
                        .await
                        .ok(),
                    };
                }

                // Best-effort cleanup: delete volumes if any were created.
                let vols: Vec<(Uuid, String, bool)> = sqlx::query_as(
                    r#"
                    SELECT id, provider_volume_id, delete_on_terminate
                    FROM instance_volumes
                    WHERE instance_id = $1
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(instance_uuid)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();
                for (vol_row_id, provider_volume_id, delete_on_terminate) in vols {
                    if !delete_on_terminate {
                        continue;
                    }
                    let _ = provider.delete_volume(&zone, &provider_volume_id).await;
                    let _ = sqlx::query(
                        "UPDATE instance_volumes SET status='deleted', deleted_at=NOW() WHERE id=$1"
                    )
                    .bind(vol_row_id)
                    .execute(&pool)
                    .await;
                }

                // Persist error on the instance row, and move it to terminating if cleanup was requested.
                // This ensures the terminator job can confirm provider deletion and finalize termination.
                let next_status = match terminate_res {
                    Ok(true) => "terminating",
                    _ => "provisioning_failed",
                };
                let _ = sqlx::query(
                    "UPDATE instances
                     SET status = $2::instance_status,
                         error_code = COALESCE(error_code, 'PROVIDER_START_FAILED'),
                         error_message = COALESCE($3, error_message),
                         failed_at = COALESCE(failed_at, NOW()),
                         deletion_reason = COALESCE(deletion_reason, 'provider_start_failed_cleanup')
                     WHERE id = $1"
                )
                .bind(instance_uuid)
                .bind(next_status)
                .bind(&msg)
                .execute(&pool)
                .await;
                return;
            } else if let Some(lid) = log_id_start {
                let duration = start_api.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, lid, "success", duration, None)
                    .await
                    .ok();
            }

            // 3.5. Retrieve IP
            println!("🔍 Retrieving IP address for {}...", server_id);
            let mut ip_address: Option<String> = None;
            let ip_api = Instant::now();
            let log_id_ip = logger::log_event_with_metadata(
                &pool,
                "PROVIDER_GET_IP",
                "in_progress",
                instance_uuid,
                None,
                Some(json!({"zone": zone, "server_id": server_id, "correlation_id": correlation_id_meta, "max_attempts": 5})),
             ).await.ok();
            for attempt in 1..=5 {
                match provider.get_instance_ip(&zone, &server_id).await {
                    Ok(Some(ip)) => {
                        println!("✅ IP Address retrieved: {}", ip);
                        ip_address = Some(ip);
                        break;
                    }
                    Ok(None) => {
                        if attempt < 5 {
                            sleep(Duration::from_secs(2)).await;
                        }
                    }
                    Err(_) => break,
                }
            }
            if let Some(lid) = log_id_ip {
                let duration = ip_api.elapsed().as_millis() as i32;
                let meta = json!({"ip_address": ip_address, "zone": zone, "server_id": server_id, "correlation_id": correlation_id_meta});
                if ip_address.is_some() {
                    logger::log_event_complete_with_metadata(
                        &pool,
                        lid,
                        "success",
                        duration,
                        None,
                        Some(meta),
                    )
                    .await
                    .ok();
                } else {
                    logger::log_event_complete_with_metadata(
                        &pool,
                        lid,
                        "failed",
                        duration,
                        Some("IP not available after retries"),
                        Some(meta),
                    )
                    .await
                    .ok();
                }
            }

            // LOG 4: INSTANCE_CREATED
            let db_start = Instant::now();
            let log_id_created = logger::log_event_with_metadata(
                &pool, "INSTANCE_CREATED", "in_progress", instance_uuid, None,
                Some(json!({"ip_address": ip_address, "server_id": server_id, "correlation_id": correlation_id_meta})),
             ).await.ok();

            // 4. Update DB
            let update_result = sqlx::query(
                "UPDATE instances
                   SET provider_instance_id = $1,
                       ip_address = $2::inet,
                       status = 'booting'
                   WHERE id = $3 AND status NOT IN ('terminating', 'terminated')",
            )
            .bind(&server_id)
            .bind(ip_address)
            .bind(instance_uuid)
            .execute(&pool)
            .await;

            if let Err(e) = update_result {
                let msg = format!("DB update failed: {:?}", e);
                if let Some(log_id) = log_id_created {
                    let duration = db_start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                        .await
                        .ok();
                }
                if let Some(log_id) = log_id_execute {
                    let duration = start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                        .await
                        .ok();
                }
                return;
            }

            if let Some(log_id) = log_id_created {
                let duration = db_start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "success", duration, None)
                    .await
                    .ok();
            }

            // FinOps domain event: instance is now allocated (booting) → start cost counting ASAP
            let _ = finops_events::emit_instance_cost_start(
                &pool,
                &redis_client,
                instance_uuid,
                "inventiv-orchestrator/services",
                Some("status=booting"),
            )
            .await;

            // Complete LOG 2
            if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "success", duration, None)
                    .await
                    .ok();
            }
        }
        Err(e) => {
            let msg = format!("Failed to create instance: {:?}", e);
            if let Some(log_id) = log_id_provider {
                let api_duration = api_start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "failed", api_duration, Some(&msg))
                    .await
                    .ok();
            }
            if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                    .await
                    .ok();
            }
            let _ = sqlx::query(
                "UPDATE instances
                  SET status = 'failed',
                      error_code = COALESCE(error_code, 'PROVIDER_CREATE_FAILED'),
                      error_message = COALESCE($2, error_message),
                      failed_at = COALESCE(failed_at, NOW())
                  WHERE id = $1",
            )
            .bind(instance_uuid)
            .bind(&msg)
            .execute(&pool)
            .await;
        }
    }
}

fn build_worker_cloud_init(
    ssh_pub: &str,
    instance_id: &str,
    control_plane_url: &str,
    model_id: &str,
    vllm_image: &str,
    vllm_port: u16,
    worker_health_port: u16,
    agent_source_url: &str,
    worker_auth_token: &str,
    worker_hf_token: &str,
) -> String {
    // Keep it simple for initial DEV->Scaleway validation:
    // - Run vLLM from upstream image
    // - Run agent from python image (mount agent.py downloaded at boot)
    // - Use host network for agent so it can talk to vLLM at 127.0.0.1
    let mut cloud = String::new();
    cloud.push_str("#cloud-config\n");
    if !ssh_pub.trim().is_empty() {
        cloud.push_str("ssh_authorized_keys:\n");
        cloud.push_str(&format!("  - {}\n", ssh_pub.trim()));
    }
    cloud.push_str("\nwrite_files:\n");
    cloud.push_str("  - path: /usr/local/bin/inventiv-worker-bootstrap.sh\n");
    cloud.push_str("    permissions: '0755'\n");
    cloud.push_str("    content: |\n");
    cloud.push_str("      #!/usr/bin/env bash\n");
    cloud.push_str("      set -euo pipefail\n");
    cloud.push_str("      echo '[inventiv-worker] bootstrap starting'\n");
    cloud.push_str(&format!("      INSTANCE_ID=\"{}\"\n", instance_id));
    cloud.push_str(&format!(
        "      CONTROL_PLANE_URL=\"{}\"\n",
        control_plane_url
    ));
    cloud.push_str(&format!("      MODEL_ID=\"{}\"\n", model_id));
    cloud.push_str(&format!("      VLLM_IMAGE=\"{}\"\n", vllm_image));
    cloud.push_str(&format!("      VLLM_PORT=\"{}\"\n", vllm_port));
    cloud.push_str(&format!(
        "      WORKER_HEALTH_PORT=\"{}\"\n",
        worker_health_port
    ));
    cloud.push_str(&format!("      AGENT_URL=\"{}\"\n", agent_source_url));
    cloud.push_str(&format!(
        "      WORKER_AUTH_TOKEN=\"{}\"\n",
        worker_auth_token
    ));
    cloud.push_str(&format!("      WORKER_HF_TOKEN=\"{}\"\n", worker_hf_token));
    cloud.push_str("      export DEBIAN_FRONTEND=noninteractive\n");
    cloud.push_str("\n");
    cloud.push_str("      if ! command -v docker >/dev/null 2>&1; then\n");
    cloud.push_str("        echo '[inventiv-worker] installing docker'\n");
    cloud.push_str("        apt-get update -y\n");
    cloud.push_str("        apt-get install -y ca-certificates curl gnupg\n");
    cloud.push_str("        curl -fsSL https://get.docker.com | sh\n");
    cloud.push_str("      fi\n");
    cloud.push_str("      systemctl enable --now docker || true\n");
    cloud.push_str("\n");
    cloud.push_str("      # Enable NVIDIA runtime for docker (required for --gpus all)\n");
    cloud.push_str("      if command -v nvidia-smi >/dev/null 2>&1; then\n");
    cloud.push_str("        echo '[inventiv-worker] installing nvidia-container-toolkit'\n");
    cloud.push_str("        set +e\n");
    cloud.push_str("        . /etc/os-release\n");
    cloud.push_str("        distribution=\"${ID}${VERSION_ID}\"\n");
    cloud.push_str("        curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | gpg --batch --yes --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg\n");
    cloud.push_str("        curl -fsSL \"https://nvidia.github.io/libnvidia-container/${distribution}/libnvidia-container.list\" \\\n");
    cloud.push_str("          | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \\\n");
    cloud.push_str("          > /etc/apt/sources.list.d/nvidia-container-toolkit.list\n");
    cloud.push_str("        apt-get update -y\n");
    cloud.push_str("        apt-get install -y nvidia-container-toolkit\n");
    cloud.push_str("        nvidia-ctk runtime configure --runtime=docker\n");
    cloud.push_str("        systemctl restart docker\n");
    cloud.push_str("        echo '[inventiv-worker] nvidia-container-toolkit configured'\n");
    cloud.push_str("        set -e\n");
    cloud.push_str("      else\n");
    cloud.push_str("        echo '[inventiv-worker] nvidia-smi not found; skipping nvidia-container-toolkit'\n");
    cloud.push_str("      fi\n");
    cloud.push_str("\n");
    cloud.push_str("      mkdir -p /opt/inventiv-worker\n");
    cloud.push_str("      curl -fsSL \"$AGENT_URL\" -o /opt/inventiv-worker/agent.py\n");
    cloud.push_str("\n");
    cloud.push_str(
        "      for i in 1 2 3 4 5; do docker pull \"$VLLM_IMAGE\" && break || sleep 5; done\n",
    );
    cloud.push_str(
        "      for i in 1 2 3 4 5; do docker pull python:3.11-slim && break || sleep 5; done\n",
    );
    cloud.push_str("\n");
    cloud.push_str("      docker rm -f vllm >/dev/null 2>&1 || true\n");
    cloud.push_str("      docker run -d --restart unless-stopped \\\n");
    cloud.push_str("        --name vllm \\\n");
    cloud.push_str("        --gpus all \\\n");
    cloud.push_str(&format!("        -p {0}:{0} \\\n", vllm_port));
    cloud.push_str("        -e HUGGING_FACE_HUB_TOKEN=\"$WORKER_HF_TOKEN\" \\\n");
    cloud.push_str("        -e HUGGINGFACE_HUB_TOKEN=\"$WORKER_HF_TOKEN\" \\\n");
    cloud.push_str("        -e HF_TOKEN=\"$WORKER_HF_TOKEN\" \\\n");
    cloud.push_str("        -e HF_HOME=/opt/inventiv-worker/hf \\\n");
    cloud.push_str("        -e TRANSFORMERS_CACHE=/opt/inventiv-worker/hf \\\n");
    cloud.push_str("        -v /opt/inventiv-worker:/opt/inventiv-worker \\\n");
    cloud.push_str("        \"$VLLM_IMAGE\" \\\n");
    cloud.push_str(&format!("        --host 0.0.0.0 --port {} \\\n", vllm_port));
    cloud.push_str("        --model \"$MODEL_ID\" \\\n");
    cloud.push_str("        --dtype float16\n");
    cloud.push_str("\n");
    cloud.push_str("      docker rm -f inventiv-agent >/dev/null 2>&1 || true\n");
    cloud.push_str("      docker run -d --restart unless-stopped \\\n");
    cloud.push_str("        --name inventiv-agent \\\n");
    cloud.push_str("        --network host \\\n");
    cloud.push_str("        -e CONTROL_PLANE_URL=\"$CONTROL_PLANE_URL\" \\\n");
    cloud.push_str("        -e INSTANCE_ID=\"$INSTANCE_ID\" \\\n");
    cloud.push_str("        -e MODEL_ID=\"$MODEL_ID\" \\\n");
    cloud.push_str(&format!(
        "        -e VLLM_BASE_URL=\"http://127.0.0.1:{}\" \\\n",
        vllm_port
    ));
    cloud.push_str("        -e WORKER_HEALTH_PORT=\"$WORKER_HEALTH_PORT\" \\\n");
    cloud.push_str("        -e WORKER_VLLM_PORT=\"$VLLM_PORT\" \\\n");
    cloud.push_str("        -e WORKER_HEARTBEAT_INTERVAL_S=10 \\\n");
    cloud.push_str("        -e WORKER_AUTH_TOKEN=\"$WORKER_AUTH_TOKEN\" \\\n");
    cloud.push_str("        -v /opt/inventiv-worker/agent.py:/app/agent.py:ro \\\n");
    cloud.push_str("        python:3.11-slim \\\n");
    cloud.push_str("        bash -lc \"pip install --no-cache-dir requests >/dev/null && python /app/agent.py\"\n");
    cloud.push_str("\n");
    cloud.push_str("      echo '[inventiv-worker] bootstrap done'\n");
    cloud.push_str("\n");
    cloud.push_str("runcmd:\n");
    cloud.push_str("  - [ bash, -lc, /usr/local/bin/inventiv-worker-bootstrap.sh ]\n");
    cloud
}

fn build_ssh_key_cloud_init(ssh_pub: &str) -> String {
    let mut cloud = String::new();
    cloud.push_str("#cloud-config\n");
    if !ssh_pub.trim().is_empty() {
        cloud.push_str("ssh_authorized_keys:\n");
        cloud.push_str(&format!("  - {}\n", ssh_pub.trim()));
    }
    cloud
}

pub async fn process_catalog_sync(pool: Pool<Postgres>) {
    println!("🔄 [Catalog Sync] Starting catalog synchronization...");

    // 1. Get Provider (Scaleway)
    let provider_name = ProviderManager::current_provider_name();
    if let Some(provider) = ProviderManager::get_provider(&provider_name, pool.clone()) {
        // Ensure the provider exists in DB (required for Settings UI and FK integrity).
        let provider_uuid: Option<Uuid> = sqlx::query_scalar(
            r#"
            INSERT INTO providers (id, name, code, description, is_active)
            VALUES (gen_random_uuid(), $1, $2, $3, true)
            ON CONFLICT (code)
            DO UPDATE SET
              name = EXCLUDED.name,
              description = EXCLUDED.description,
              is_active = true
            RETURNING id
            "#,
        )
        .bind(match provider_name.as_str() {
            "scaleway" => "Scaleway",
            "mock" => "Mock",
            _ => provider_name.as_str(),
        })
        .bind(&provider_name)
        .bind(format!("Auto-managed provider entry for {}", provider_name))
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

        let Some(provider_uuid) = provider_uuid else {
            println!(
                "❌ [Catalog Sync] Could not resolve provider id in DB for code={}",
                provider_name
            );
            return;
        };

        // Prefer zones configured in DB for this provider; fallback to a sane default list.
        let zones: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT z.code
            FROM zones z
            JOIN regions r ON r.id = z.region_id
            WHERE z.is_active = true
              AND r.provider_id = $1
            ORDER BY z.code
            "#,
        )
        .bind(provider_uuid)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        let zones: Vec<String> = if zones.is_empty() {
            vec![
                // fallback only makes sense for scaleway; mock will typically be in DB
                "fr-par-1".to_string(),
                "fr-par-2".to_string(),
            ]
        } else {
            zones
        };

        for zone in &zones {
            println!("🔄 [Catalog Sync] Fetching catalog for zone: {}", zone);

            // Ensure region+zone exist (so Settings UI doesn't stay empty).
            // Region code heuristic: drop the trailing "-<digit>" (e.g., fr-par-2 -> fr-par).
            let region_code = zone.rsplitn(2, '-').nth(1).unwrap_or(zone).to_string();
            let region_name = region_code.clone();
            let region_id: Option<Uuid> = sqlx::query_scalar(
                r#"
                 INSERT INTO regions (id, provider_id, name, code, is_active)
                 VALUES (gen_random_uuid(), $1, $2, $3, true)
                 ON CONFLICT (provider_id, code)
                 DO UPDATE SET
                   name = EXCLUDED.name,
                   is_active = true
                 RETURNING id
                 "#,
            )
            .bind(provider_uuid)
            .bind(&region_name)
            .bind(&region_code)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

            let zone_id: Option<Uuid> = if let Some(rid) = region_id {
                sqlx::query_scalar(
                    r#"
                     INSERT INTO zones (id, region_id, name, code, is_active)
                     VALUES (gen_random_uuid(), $1, $2, $3, true)
                     ON CONFLICT (region_id, code)
                     DO UPDATE SET
                       name = EXCLUDED.name,
                       is_active = true
                     RETURNING id
                     "#,
                )
                .bind(rid)
                .bind(zone)
                .bind(zone)
                .fetch_optional(&pool)
                .await
                .unwrap_or(None)
            } else {
                None
            };

            if zone_id.is_none() {
                println!(
                    "⚠️ [Catalog Sync] Zone '{}' not found in DB; skipping availability mapping",
                    zone
                );
            }

            match provider.fetch_catalog(zone).await {
                Ok(items) => {
                    let mut count = 0;
                    for item in items {
                        // Convert f64 to BigDecimal for NUMERIC column
                        // Using primitive cast via string to avoid precision issues if possible or just use FromPrimitive
                        // sqlx BigDecimal feature allows direct usage usually if From f64 is implemented.
                        // But safer to cast in SQL or use bigdecimal crate types.
                        let hourly_price = bigdecimal::BigDecimal::from_f64(item.cost_per_hour)
                            .unwrap_or_default();

                        // Upsert instance type and get its id (needed to map availability to zones)
                        let type_id: Option<Uuid> = sqlx::query_scalar(
                            "INSERT INTO instance_types (id, provider_id, name, code, is_active, cost_per_hour, cpu_count, ram_gb, gpu_count, vram_per_gpu_gb, bandwidth_bps)
                             VALUES (gen_random_uuid(), $1, $2, $3, true, $4, $5, $6, $7, $8, $9)
                             ON CONFLICT (provider_id, code)
                             DO UPDATE SET
                                name = EXCLUDED.name,
                                cost_per_hour = EXCLUDED.cost_per_hour,
                                cpu_count = EXCLUDED.cpu_count,
                                ram_gb = EXCLUDED.ram_gb,
                                gpu_count = EXCLUDED.gpu_count,
                                vram_per_gpu_gb = EXCLUDED.vram_per_gpu_gb,
                                bandwidth_bps = EXCLUDED.bandwidth_bps,
                                is_active = true
                             RETURNING id"
                        )
                        .bind(provider_uuid)
                        .bind(&item.name)
                        .bind(&item.code)
                        .bind(hourly_price)
                        .bind(item.cpu_count)
                        .bind(item.ram_gb)
                        .bind(item.gpu_count)
                        .bind(item.vram_per_gpu_gb)
                        .bind(item.bandwidth_bps)
                        .fetch_optional(&pool)
                        .await
                        .unwrap_or(None);

                        // Map availability: all items returned by provider for this zone are available.
                        if let (Some(tid), Some(zid)) = (type_id, zone_id) {
                            let _ = sqlx::query(
                                "INSERT INTO instance_type_zones (instance_type_id, zone_id, is_available)
                                 VALUES ($1, $2, true)
                                 ON CONFLICT (instance_type_id, zone_id)
                                 DO UPDATE SET is_available = EXCLUDED.is_available"
                            )
                            .bind(tid)
                            .bind(zid)
                            .execute(&pool)
                            .await;
                        }
                        count += 1;
                    }
                    println!(
                        "✅ [Catalog Sync] Updated {} types for zone {}",
                        count, zone
                    );
                }
                Err(e) => println!("❌ [Catalog Sync] Error for {}: {:?}", zone, e),
            }
        }
    } else {
        println!("❌ [Catalog Sync] Provider Scaleway not configured.");
    }
}

pub async fn process_full_reconciliation(pool: Pool<Postgres>) {
    println!("🔄 [Full Reconciliation] Starting...");
    let provider_name = ProviderManager::current_provider_name();
    if let Some(provider) = ProviderManager::get_provider(&provider_name, pool.clone()) {
        // Zones for this provider
        let zones: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT z.code
            FROM zones z
            JOIN regions r ON r.id = z.region_id
            JOIN providers p ON p.id = r.provider_id
            WHERE z.is_active = true
              AND p.code = $1
            ORDER BY z.code
            "#,
        )
        .bind(&provider_name)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        for zone in zones {
            match provider.list_instances(&zone).await {
                Ok(instances) => {
                    println!(
                        "🔍 [Full Reconciliation] List returned {} instances in {}",
                        instances.len(),
                        zone
                    );
                    let mut import_count = 0;
                    for inst in instances {
                        // Check if exists
                        let exists_res = sqlx::query_scalar(
                             "SELECT EXISTS(SELECT 1 FROM instances WHERE provider_instance_id = $1)"
                         )
                         .bind(&inst.provider_id)
                         .fetch_one(&pool)
                         .await;

                        let exists = exists_res.unwrap_or(false);

                        // Import if not exists and status is active-ish
                        if !exists && inst.status != "terminated" && inst.status != "archived" {
                            println!(
                                "🔍 [Full Reconciliation] Found orphan: {} ({}) Status: {}",
                                inst.name, inst.provider_id, inst.status
                            );

                            // Resolve Provider ID (by code) + Zone ID (by provider + zone code)
                            let provider_id: Option<Uuid> = sqlx::query_scalar(
                                "SELECT id FROM providers WHERE code = $1 LIMIT 1",
                            )
                            .bind(&provider_name)
                            .fetch_optional(&pool)
                            .await
                            .unwrap_or(None);

                            let zone_id: Option<Uuid> = if let Some(pid) = provider_id {
                                sqlx::query_scalar(
                                    r#"
                                    SELECT z.id
                                    FROM zones z
                                    JOIN regions r ON r.id = z.region_id
                                    WHERE z.code = $1
                                      AND r.provider_id = $2
                                    LIMIT 1
                                    "#,
                                )
                                .bind(&zone)
                                .bind(pid)
                                .fetch_optional(&pool)
                                .await
                                .unwrap_or(None)
                            } else {
                                None
                            };

                            if let (Some(pid), Some(zid)) = (provider_id, zone_id) {
                                let new_id = Uuid::new_v4();
                                let type_id: Option<Uuid> = sqlx::query_scalar(
                                    r#"
                                    SELECT it.id
                                    FROM instance_types it
                                    WHERE it.provider_id = $1
                                      AND it.is_active = true
                                    ORDER BY it.gpu_count DESC, it.vram_per_gpu_gb DESC, it.name ASC
                                    LIMIT 1
                                    "#,
                                )
                                .bind(pid)
                                .fetch_optional(&pool)
                                .await
                                .unwrap_or(None);

                                let Some(type_id) = type_id else {
                                    println!("⚠️ [Full Reconciliation] No instance_types found for provider '{}', skipping orphan import.", provider_name);
                                    continue;
                                };

                                // Map Status (Simplistic)
                                let status = match inst.status.as_str() {
                                    "running" | "starting" => "ready",
                                    "stopped" => "failed",
                                    _ => "provisioning",
                                };

                                let insert_res = sqlx::query(
                                     "INSERT INTO instances 
                                     (id, provider_id, zone_id, instance_type_id, status, provider_instance_id, ip_address, created_at, gpu_profile)
                                     VALUES ($1, $2, $3, $4, $5::instance_status, $6, $7::inet, NOW(), '{}')"
                                 )
                                 .bind(new_id)
                                .bind(pid)
                                 .bind(zid)
                                 .bind(type_id)
                                 .bind(status)
                                 .bind(&inst.provider_id)
                                 .bind(inst.ip_address)
                                 .execute(&pool)
                                 .await;

                                if let Err(e) = insert_res {
                                    println!(
                                        "❌ [Full Reconciliation] Failed to import orphan {}: {:?}",
                                        inst.provider_id, e
                                    );
                                } else {
                                    println!(
                                        "✅ [Full Reconciliation] Imported orphan {} => {}",
                                        inst.provider_id, new_id
                                    );
                                    import_count += 1;
                                }
                            } else {
                                println!(
                                    "⚠️ [Full Reconciliation] Unknown zone '{}' for orphan {}",
                                    zone, inst.provider_id
                                );
                            }
                        } else if exists {
                            // Check for Zombie State (DB=terminated vs Cloud=running)
                            let current_status: Option<String> = sqlx::query_scalar(
                                "SELECT status::text FROM instances WHERE provider_instance_id = $1"
                            )
                            .bind(&inst.provider_id)
                            .fetch_optional(&pool)
                            .await.unwrap_or(None);

                            if let Some(db_status) = current_status {
                                if (db_status == "terminated" || db_status == "archived")
                                    && (inst.status == "running" || inst.status == "starting")
                                {
                                    println!("⚠️ [Full Reconciliation] ZOMBIE DETECTED: {} is {} on Cloud but {} in DB. Reactivating...", inst.provider_id, inst.status, db_status);

                                    let _ = sqlx::query(
                                         "UPDATE instances SET status = 'ready', terminated_at = NULL, is_archived = false WHERE provider_instance_id = $1"
                                     )
                                     .bind(&inst.provider_id)
                                     .execute(&pool)
                                     .await;
                                    println!(
                                        "✅ [Full Reconciliation] Zombie {} reactivated in DB.",
                                        inst.provider_id
                                    );
                                }
                            }
                        }
                    }
                    if import_count > 0 {
                        println!(
                            "✅ [Full Reconciliation] Imported {} orphaned instances in {}",
                            import_count, zone
                        );
                    }
                }
                Err(e) => println!(
                    "❌ [Full Reconciliation] Failed to list instances in {}: {:?}",
                    zone, e
                ),
            }
        }
        println!("✅ [Full Reconciliation] Completed.");
    } else {
        println!("❌ [Full Reconciliation] Provider Scaleway not configured.");
    }
}
