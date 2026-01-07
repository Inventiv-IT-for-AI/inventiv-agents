use crate::finops_events;
use crate::health_check_flow;
use crate::logger;
use crate::provider_manager::ProviderManager;
use crate::state_machine;
use bigdecimal::FromPrimitive;
use inventiv_common::worker_storage;
use serde_json::json;
use sqlx::{Pool, Postgres};
use std::fs;
use std::net::TcpStream;
use std::time::Duration as StdDuration;
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

/// Check if SSH port 22 is accessible on the given IP address
async fn check_ssh_accessible(ip: &str) -> bool {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let addr = format!("{}:22", clean_ip);

    tokio::task::spawn_blocking(move || {
        let socket_addr = match addr.parse() {
            Ok(a) => a,
            Err(_) => return false,
        };
        TcpStream::connect_timeout(&socket_addr, StdDuration::from_secs(3)).is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Determine if an error is a normal retry scenario (instance in transitional state) vs a real failure.
/// Returns (is_retry, retry_message) where:
/// - is_retry: true if this is a normal retry (instance is starting/stopping), false if real error
/// - retry_message: descriptive message for retry scenarios
async fn is_normal_retry(
    provider: &dyn inventiv_providers::CloudProvider,
    zone: &str,
    provider_instance_id: &str,
    error_msg: Option<&str>,
) -> (bool, Option<String>) {
    // Check error message for common retry indicators
    if let Some(msg) = error_msg {
        // Check for stopping/stopped states in error messages (termination retries)
        if msg.contains("current state: stopping")
            || msg.contains("current state: stopped")
            || msg.contains("failed to stop")
        {
            let state_msg = if msg.contains("current state:") {
                msg.split("current state: ")
                    .nth(1)
                    .and_then(|s| s.split_whitespace().next())
                    .map(|s| format!("Instance is {} - retrying", s))
                    .unwrap_or_else(|| "Instance is stopping - retrying".to_string())
            } else {
                "Instance is stopping - retrying".to_string()
            };
            return (true, Some(state_msg));
        }

        // Check for starting/starting states in error messages
        if msg.contains("current state: starting")
            || msg.contains("current state: booting")
            || msg.contains("instance is starting")
        {
            return (true, Some("Instance is starting - retrying".to_string()));
        }
    }

    // Check actual instance state via provider API
    if let Ok(Some(state)) = provider.get_server_state(zone, provider_instance_id).await {
        let state_lower = state.to_ascii_lowercase();
        // Transitional states that indicate normal retry scenarios
        if matches!(
            state_lower.as_str(),
            "starting" | "booting" | "stopping" | "stopped" | "stopped_in_place"
        ) {
            let retry_msg = match state_lower.as_str() {
                "starting" | "booting" => "Instance is starting - retrying",
                "stopping" => "Instance is stopping - retrying",
                "stopped" | "stopped_in_place" => "Instance is stopped - retrying",
                _ => "Instance is in transitional state - retrying",
            };
            return (true, Some(retry_msg.to_string()));
        }
    }

    // Not a retry scenario - real error
    (false, None)
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
            eprintln!(
                "❌ [process_termination] Invalid instance_id '{}': {:?}",
                instance_id, e
            );
            return;
        }
    };
    let correlation_id_meta = correlation_id.clone();
    eprintln!(
        "🔵 [process_termination] Starting termination for instance {} (correlation_id={:?})",
        id_uuid, correlation_id_meta
    );

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
            eprintln!(
                "🔵 [process_termination] Found instance {}: provider_instance_id={:?}, zone={}, status={}",
                id_uuid,
                provider_id_opt.as_deref(),
                zone,
                current_status
            );

            // Ensure instance is in terminating status (idempotent transition)
            let _ = sqlx::query(
                "UPDATE instances 
                 SET status = 'terminating'
                 WHERE id = $1 
                   AND status NOT IN ('terminated', 'archived')
                   AND status != 'terminating'",
            )
            .bind(id_uuid)
            .execute(&pool)
            .await;

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
                .unwrap_or_else(ProviderManager::current_provider_name);

                let provider_res =
                    ProviderManager::get_provider(&provider_code, pool.clone()).await;
                match provider_res {
                    Ok(provider) => {
                        eprintln!(
                            "🔵 [process_termination] Got provider '{}' for instance {}",
                            provider_code, id_uuid
                        );
                        // STEP 1: Discover volumes attached to instance (best effort - don't fail if this errors)
                        eprintln!("🔵 [process_termination] Step 1: Discovering volumes attached to instance {}", provider_instance_id);

                        // Discover volumes attached to the instance (even if not in instance_volumes table)
                        // Use best-effort: if this fails, we'll still try to delete volumes from DB
                        match provider
                            .list_attached_volumes(&zone, &provider_instance_id)
                            .await
                        {
                            Ok(attached_volumes) => {
                                for av in attached_volumes {
                                    // Check if this volume is already tracked in instance_volumes
                                    let exists: bool = sqlx::query_scalar(
                                    "SELECT EXISTS(SELECT 1 FROM instance_volumes WHERE instance_id=$1 AND provider_volume_id=$2 AND deleted_at IS NULL)",
                                )
                                .bind(id_uuid)
                                .bind(&av.provider_volume_id)
                                .fetch_one(&pool)
                                .await
                                .unwrap_or(false);

                                    if !exists {
                                        // Volume exists at provider but not in our DB - track it so we can delete it
                                        eprintln!(
                                        "🔍 [process_termination] Discovered untracked volume {} for instance {} - adding to deletion queue",
                                        av.provider_volume_id, id_uuid
                                    );

                                        let row_id = Uuid::new_v4();
                                        let provider_id: Option<Uuid> = sqlx::query_scalar(
                                            "SELECT provider_id FROM instances WHERE id = $1",
                                        )
                                        .bind(id_uuid)
                                        .fetch_optional(&pool)
                                        .await
                                        .ok()
                                        .flatten();

                                        if let Some(pid) = provider_id {
                                            let _ = sqlx::query(
                                            r#"
                                            INSERT INTO instance_volumes 
                                            (id, instance_id, provider_id, zone_code, provider_volume_id, provider_volume_name, volume_type, size_bytes, delete_on_terminate, status, attached_at, is_boot)
                                            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE, 'attached', NOW(), $9)
                                            ON CONFLICT (instance_id, provider_volume_id) DO NOTHING
                                            "#,
                                        )
                                        .bind(row_id)
                                        .bind(id_uuid)
                                        .bind(pid)
                                        .bind(&zone)
                                        .bind(&av.provider_volume_id)
                                        .bind(av.provider_volume_name.as_deref())
                                        .bind(&av.volume_type)
                                        .bind(av.size_bytes.unwrap_or(0))
                                        .bind(av.boot)
                                        .execute(&pool)
                                        .await;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("⚠️ [process_termination] Failed to list attached volumes (continuing anyway): {}", e);
                                // Continue - we'll delete volumes from DB anyway
                            }
                        }

                        // Note: We don't try to detach volumes before termination because:
                        // 1. Scaleway will detach volumes automatically when instance is deleted
                        // 2. Detaching volumes while instance is running can cause issues
                        // 3. If instance deletion fails, we'll delete volumes anyway in Step 3

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

                        // Call terminate_instance with timeout to avoid indefinite blocking
                        // Note: terminate_instance may need to stop the instance first (up to 60s), then delete it
                        // So we use a longer timeout (90s) to account for stop + delete operations
                        eprintln!("🔵 [process_termination] Step 2: Terminating instance {} on provider (timeout: 90s)", provider_instance_id);
                        let terminate_future =
                            provider.terminate_instance(&zone, &provider_instance_id);
                        let result =
                            tokio::time::timeout(Duration::from_secs(90), terminate_future).await;

                        let termination_ok: bool = match result {
                            Ok(Ok(true)) => {
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
                            Ok(Ok(false)) => {
                                let err_msg =
                                    "Provider termination call returned non-success status";
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
                                // Don't return here - continue to delete volumes even if instance termination failed
                                false
                            }
                            Ok(Err(e)) => {
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
                                    // Don't return here - continue to delete volumes even if instance termination failed
                                    false
                                }
                            }
                            Err(_timeout) => {
                                let err_msg = "Instance termination timed out after 90s";
                                eprintln!("❌ [process_termination] {}", err_msg);
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
                                // Don't return here - continue to delete volumes even if instance termination timed out
                                false
                            }
                        };

                        // STEP 3: Delete volumes (ALWAYS, even if instance termination failed)
                        // This ensures we don't leak resources even if instance deletion fails
                        eprintln!("🔵 [process_termination] Step 3: Deleting volumes (instance termination: {})", if termination_ok { "success" } else { "failed" });

                        // Get volumes to delete (refresh list in case volumes were discovered)
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
                                eprintln!("⏭️ [process_termination] Skipping volume {} (delete_on_terminate=false)", provider_volume_id);
                                continue;
                            }

                            eprintln!(
                                "🔵 [process_termination] Deleting volume {}",
                                provider_volume_id
                            );

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
                                        eprintln!("✅ [process_termination] Successfully deleted volume {}", provider_volume_id);
                                        logger::log_event_complete(&pool, lid, "success", dur, None)
                                            .await
                                            .ok()
                                    }
                                    Ok(false) => {
                                        eprintln!("⚠️ [process_termination] Volume deletion returned false for {}", provider_volume_id);
                                        logger::log_event_complete(
                                            &pool,
                                            lid,
                                            "failed",
                                            dur,
                                            Some("Provider returned false"),
                                        )
                                        .await
                                        .ok()
                                    }
                                    Err(e) => {
                                        eprintln!("❌ [process_termination] Failed to delete volume {}: {}", provider_volume_id, e);
                                        logger::log_event_complete(
                                            &pool,
                                            lid,
                                            "failed",
                                            dur,
                                            Some(&e.to_string()),
                                        )
                                        .await
                                        .ok()
                                    }
                                };
                            }
                            if del_res.unwrap_or(false) {
                                let _ = sqlx::query(
                                "UPDATE instance_volumes SET status='deleted', deleted_at=NOW() WHERE id=$1"
                            )
                            .bind(vol_row_id)
                            .execute(&pool)
                            .await;
                            } else {
                                // Even if deletion failed, mark as deleted in DB to avoid retry loops
                                // The volume might be deleted by Scaleway when instance is deleted
                                eprintln!("⚠️ [process_termination] Marking volume {} as deleted in DB despite deletion failure", provider_volume_id);
                                let _ = sqlx::query(
                                "UPDATE instance_volumes SET status='deleted', deleted_at=NOW() WHERE id=$1"
                            )
                            .bind(vol_row_id)
                            .execute(&pool)
                            .await;
                            }
                        }

                        // If instance termination failed, log error but don't return - volumes are cleaned up
                        if !termination_ok {
                            let err_msg =
                                "Instance termination failed, but volumes have been cleaned up";
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

                        // STEP 4: Verify instance deletion (only if termination was successful)
                        // Note: Volumes have already been deleted in Step 3, so we only verify instance deletion here
                        if termination_ok {
                            // 2.5 Verify deletion (avoid marking terminated while still running)
                            // Scaleway termination is async; we poll for a short, bounded period.
                            eprintln!("🔵 [process_termination] Step 4: Verifying instance deletion (timeout: 60s)");
                            let verify_start = Instant::now();
                            let mut deleted = false;
                            while verify_start.elapsed() < Duration::from_secs(60) {
                                match provider
                                    .check_instance_exists(&zone, &provider_instance_id)
                                    .await
                                {
                                    Ok(false) => {
                                        deleted = true;
                                        eprintln!("✅ [process_termination] Instance {} confirmed deleted on provider", provider_instance_id);
                                        break;
                                    }
                                    Ok(true) => {
                                        if verify_start.elapsed().as_secs().is_multiple_of(10) {
                                            eprintln!("⏳ [process_termination] Instance {} still exists, waiting... ({:.0}s elapsed)", provider_instance_id, verify_start.elapsed().as_secs());
                                        }
                                        sleep(Duration::from_secs(5)).await;
                                    }
                                    Err(e) => {
                                        eprintln!("⚠️ [process_termination] Error checking deletion status on provider: {:?}", e);
                                        // Keep waiting a bit; reconciliation watchdog will retry later if needed.
                                        sleep(Duration::from_secs(5)).await;
                                    }
                                }
                            }

                            if !deleted {
                                eprintln!("⚠️ [process_termination] Instance {} still exists after 60s - marking as terminating (reconciliation will retry)", provider_instance_id);
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
                                    logger::log_event_complete(
                                        &pool, log_id, "success", duration, None,
                                    )
                                    .await
                                    .ok();
                                }

                                // Complete EXECUTE_TERMINATE even if instance is not yet deleted
                                if let Some(log_id) = log_id_execute {
                                    let duration = start.elapsed().as_millis() as i32;
                                    logger::log_event_complete(
                                    &pool,
                                    log_id,
                                    "success",
                                    duration,
                                    Some("Termination in progress (not yet deleted on provider, volumes cleaned up)"),
                                )
                                .await
                                .ok();
                                }
                                // Return early - instance will be cleaned up by reconciliation
                                return;
                            }
                            // Instance confirmed deleted - continue to mark as terminated in DB
                            eprintln!("✅ [process_termination] Instance {} successfully terminated and verified deleted", provider_instance_id);
                        } else {
                            // Instance termination failed - volumes are already cleaned up, complete EXECUTE_TERMINATE
                            eprintln!("⚠️ [process_termination] Instance termination failed, but volumes have been cleaned up");
                            if let Some(log_id) = log_id_execute {
                                let duration = start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                &pool,
                                log_id,
                                "failed",
                                duration,
                                Some("Instance termination failed, but volumes have been cleaned up"),
                            )
                            .await
                            .ok();
                            }
                            // Don't mark as terminated in DB if termination failed
                            return;
                        }

                        // If we reach here, instance was successfully terminated and verified deleted
                        // Continue to mark as terminated in DB (code below)
                    }
                    Err(e) => {
                        let err_msg = format!("Provider '{}' not available: {}", provider_code, e);
                        eprintln!("❌ [process_termination] {}", err_msg);
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
                        // Don't return - let job-terminator handle retries
                        return;
                    }
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

pub async fn process_reinstall(
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
                "❌ Invalid instance_id for reinstall '{}': {:?}",
                instance_id, e
            );
            return;
        }
    };

    println!("🔧 Processing Reinstall Async: {}", id_uuid);

    let log_id_execute = logger::log_event_with_metadata(
        &pool,
        "EXECUTE_REINSTALL",
        "in_progress",
        id_uuid,
        None,
        Some(json!({ "correlation_id": correlation_id })),
    )
    .await
    .ok();

    // Fetch instance IP and provider_instance_id (must exist for a real reinstall)
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT ip_address::text, provider_instance_id::text
         FROM instances
         WHERE id = $1",
    )
    .bind(id_uuid)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    let Some((ip_opt, provider_instance_id_opt)) = row else {
        if let Some(lid) = log_id_execute {
            let dur = start.elapsed().as_millis() as i32;
            logger::log_event_complete(&pool, lid, "failed", dur, Some("Instance not found"))
                .await
                .ok();
        }
        return;
    };

    let ip = ip_opt.unwrap_or_default();
    let provider_instance_id = provider_instance_id_opt.unwrap_or_default();
    if ip.trim().is_empty() || provider_instance_id.trim().is_empty() {
        if let Some(lid) = log_id_execute {
            let dur = start.elapsed().as_millis() as i32;
            logger::log_event_complete(
                &pool,
                lid,
                "failed",
                dur,
                Some("Missing ip_address or provider_instance_id"),
            )
            .await
            .ok();
        }
        return;
    }

    // Move instance back to booting so health checks can converge to READY again.
    let _ = sqlx::query(
        "UPDATE instances
         SET status = 'booting',
             boot_started_at = NOW(),
             last_health_check = NULL,
             health_check_failures = 0,
             failed_at = NULL,
             error_code = NULL,
             error_message = NULL
         WHERE id = $1
           AND status NOT IN ('terminated', 'terminating')",
    )
    .bind(id_uuid)
    .execute(&pool)
    .await;

    // Force SSH bootstrap (restarts vLLM/agent) even if auto-install is disabled.
    health_check_flow::trigger_worker_reinstall_over_ssh(
        &pool,
        id_uuid,
        &ip,
        correlation_id.clone(),
    )
    .await;

    if let Some(lid) = log_id_execute {
        let dur = start.elapsed().as_millis() as i32;
        logger::log_event_complete(&pool, lid, "success", dur, Some("Reinstall triggered"))
            .await
            .ok();
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
    eprintln!(
        "🔵 [process_provisioning] Starting provisioning for instance {} (zone={}, type={}, correlation_id={:?})",
        instance_uuid, zone, instance_type, correlation_id_meta
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
        .unwrap_or_else(ProviderManager::current_provider_name);

    eprintln!(
        "🔵 [process_provisioning] Resolved provider '{}' (provider_id={}) for instance {}",
        provider_name, provider_id, instance_uuid
    );

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

    eprintln!(
        "🔵 [process_provisioning] Catalog lookup: zone='{}' (found={}), type='{}' (found={})",
        zone,
        zone_id.is_some(),
        instance_type,
        type_id.is_some()
    );

    if zone_id.is_none() || type_id.is_none() {
        let msg = format!(
            "Catalog lookup failed: Zone='{}' (found={}) Type='{}' (found={})",
            zone,
            zone_id.is_some(),
            instance_type,
            type_id.is_some()
        );
        eprintln!(
            "❌ [process_provisioning] {} for instance {}",
            msg, instance_uuid
        );
        sqlx::query(
            "UPDATE instances 
             SET status = 'failed',
                 error_code = COALESCE(error_code, 'CATALOG_LOOKUP_FAILED'),
                 error_message = COALESCE($2, error_message),
                 failed_at = COALESCE(failed_at, NOW())
             WHERE id = $1",
        )
        .bind(instance_uuid)
        .bind(&msg)
        .execute(&pool)
        .await
        .ok();
        eprintln!(
            "❌ [process_provisioning] Failed to update instance {} status to failed",
            instance_uuid
        );
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
    if auto_install_guard && provider_name.eq_ignore_ascii_case("scaleway") {
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
    let provider = match ProviderManager::get_provider(&provider_name, pool.clone()).await {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("Missing Provider Credentials: {}", e);
            eprintln!("❌ {}", msg);
            if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                    .await
                    .ok();
            }
            let _ = sqlx::query(
                "UPDATE instances
                 SET status = 'failed',
                     error_code = COALESCE(error_code, 'MISSING_PROVIDER_CREDENTIALS'),
                     error_message = COALESCE($2, error_message),
                     failed_at = COALESCE(failed_at, NOW())
                 WHERE id = $1",
            )
            .bind(instance_uuid)
            .bind(&msg)
            .execute(&pool)
            .await;
            return;
        }
    };

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
    // Provider-specific boot image resolution: some providers require diskless boot for certain instance types
    // This is now handled via provider.requires_diskless_boot() abstraction
    let mut image_id = "8e0da557-5d75-40ba-b928-5984075aa255".to_string();

    // Provider-specific image override (e.g. GPU-optimized images).
    // Expected: instance_types.allocation_params = {provider_code: {"image_id":"<uuid>"}}.
    let override_image: Option<String> = sqlx::query_scalar(
        r#"
        SELECT NULLIF(TRIM(it.allocation_params->($2::text)->>'image_id'), '')
            FROM instance_types it
            WHERE it.id = $1
            "#,
    )
    .bind(type_id)
    .bind(&provider_name)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);
    if let Some(img) = override_image {
        image_id = img;
    }

    // Check if provider requires diskless boot for this instance type
    if provider.requires_diskless_boot(&instance_type) {
        // Prefer a provider-specific diskless boot image configured on the instance type.
        // Expected shape: instance_types.allocation_params = {provider_code: {"boot_image_id": "<uuid>" }}
        let configured: Option<String> = sqlx::query_scalar(
            r#"
            SELECT NULLIF(TRIM(it.allocation_params->($2::text)->>'boot_image_id'), '')
            FROM instance_types it
            WHERE it.id = $1
            "#,
        )
        .bind(type_id)
        .bind(&provider_name)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

        if let Some(img) = configured {
            // Diskless override has priority.
            image_id = img;
        } else {
            // Try provider-side auto-discovery (reuse already-initialized provider).
            match provider.resolve_boot_image(&zone, &instance_type).await {
                Ok(Some(img)) => {
                    println!(
                        "ℹ️ Provider diskless boot: auto-resolved boot image '{}' for zone '{}' (type '{}')",
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
                                ARRAY[$2::text],
                                COALESCE(allocation_params->$2, '{}'::jsonb)
                                  || jsonb_build_object('boot_image_id', to_jsonb($3::text)),
                                true
                            )
                        WHERE id = $1
                          AND NULLIF(TRIM(allocation_params->$2->>'boot_image_id'), '') IS NULL
                        "#,
                    )
                    .bind(type_id)
                    .bind(&provider_name)
                    .bind(&image_id)
                    .execute(&pool)
                    .await;
                }
                Ok(None) => {
                    let msg = format!("Provider requires a diskless/compatible boot image for this instance type. Auto-discovery did not find a suitable image. Configure instance_types.allocation_params.{}.boot_image_id for this type.", provider_name);
                    eprintln!("❌ {}", msg);
                    if let Some(log_id) = log_id_execute {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                            .await
                            .ok();
                    }
                    let _ = sqlx::query(
                        "UPDATE instances
                         SET status = 'failed',
                             error_code = COALESCE(error_code, 'DISKLESS_BOOT_IMAGE_REQUIRED'),
                             error_message = COALESCE($2, error_message),
                             failed_at = COALESCE(failed_at, NOW())
                         WHERE id = $1",
                    )
                    .bind(instance_uuid)
                    .bind(&msg)
                    .execute(&pool)
                    .await;
                    return;
                }
                Err(e) => {
                    let msg = format!("Provider diskless boot image auto-discovery failed: {}", e);
                    eprintln!("❌ {}", msg);
                    if let Some(log_id) = log_id_execute {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg))
                            .await
                            .ok();
                    }
                    let _ = sqlx::query(
                        "UPDATE instances
                         SET status = 'failed',
                             error_code = COALESCE(error_code, 'DISKLESS_BOOT_IMAGE_RESOLVE_FAILED'),
                             error_message = COALESCE($2, error_message),
                             failed_at = COALESCE(failed_at, NOW())
                         WHERE id = $1",
                    )
                    .bind(instance_uuid)
                    .bind(&msg)
                    .execute(&pool)
                    .await;
                    return;
                }
            }
        }
    }

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
    // Provider-specific SSH key path should be configured via provider_settings or env vars
    let ssh_pub_path = std::env::var("WORKER_SSH_PUBLIC_KEY_FILE")
        .or_else(|_| std::env::var("SSH_PUBLIC_KEY_FILE"))
        .unwrap_or_else(|_| "/app/.ssh/llm-studio-key.pub".to_string());
    let ssh_pub = fs::read_to_string(&ssh_pub_path)
        .ok()
        .map(|s| s.trim().replace('\n', " "))
        .unwrap_or_default();

    // Build cloud-init for worker auto-install (provider-agnostic)
    let cloud_init_for_create: Option<String> = if auto_install && is_worker_target {
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

            // Resolve vLLM image with hierarchy:
            // 1. instance_types.allocation_params.vllm_image (instance-type specific)
            // 2. provider_settings.WORKER_VLLM_IMAGE_<INSTANCE_TYPE_CODE> (per instance type)
            // 3. provider_settings.WORKER_VLLM_IMAGE (provider default)
            // 4. WORKER_VLLM_IMAGE (env var)
            // 5. Hardcoded default (stable version, not "latest")
            let instance_type_id: Option<Uuid> =
                sqlx::query_scalar("SELECT instance_type_id FROM instances WHERE id = $1")
                    .bind(instance_uuid)
                    .fetch_optional(&pool)
                    .await
                    .ok()
                    .flatten();

            let vllm_image =
                resolve_vllm_image(&pool, instance_type_id, provider_id, &instance_type).await;

            let worker_health_port: u16 = if let Some(pid) = provider_id {
                sqlx::query_scalar::<_, i64>(
                        "SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_HEALTH_PORT'",
                    )
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
                sqlx::query_scalar::<_, i64>(
                        "SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_PORT'",
                    )
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
    };

    // Get data volume configuration (if any)
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
    .unwrap_or(None);

    let mut data_conf: Option<(i64, Option<i32>, bool)> =
        data_conf_row.and_then(|(gb_opt, perf_opt, del)| gb_opt.map(|gb| (gb, perf_opt, del)));

    // Fallback: if instance type doesn't specify a data volume, infer a safe size from the model.
    // This helps prevent "no space left on device" during docker + image + model pulls on diskless GPUs.
    // Storage strategy is provider-specific and handled via provider abstractions
    if data_conf.is_none() && auto_install && is_worker_target {
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
            if let Some(gb) = worker_storage::recommended_data_volume_gb(&worker_model, default_gb)
            {
                data_conf = Some((gb, None, true));
            }
        }
    }

    // Provider-specific volume creation strategy: some providers require volumes to be created BEFORE instance
    // This is determined by provider.should_pre_create_data_volume() abstraction
    let mut pre_created_volume_id: Option<String> = None;
    if is_worker_target && provider.should_pre_create_data_volume(&instance_type) {
        if let Some((gb, perf_iops, delete_on_terminate)) = data_conf {
            if gb > 0 {
                let vol_name = format!("inventiv-data-{}", instance_uuid);
                eprintln!(
                    "🔵 [process_create] Creating Block Storage volume BEFORE instance creation: name={}, size={}GB",
                    vol_name, gb
                );

                let create_log = logger::log_event_with_metadata(
                    &pool,
                    "PROVIDER_CREATE_VOLUME",
                    "in_progress",
                    instance_uuid,
                    None,
                    Some(json!({"zone": zone, "name": vol_name, "size_gb": gb, "correlation_id": correlation_id_meta, "pre_create": true})),
                )
                .await
                .ok();
                let vol_start = Instant::now();
                let volume_type = provider.get_data_volume_type(&instance_type);
                let created = provider
                    .create_volume(&zone, &vol_name, gb_to_bytes(gb), &volume_type, perf_iops)
                    .await;
                match created {
                    Ok(Some(vol_id)) => {
                        pre_created_volume_id = Some(vol_id.clone());
                        if let Some(lid) = create_log {
                            let dur = vol_start.elapsed().as_millis() as i32;
                            logger::log_event_complete(&pool, lid, "success", dur, None)
                                .await
                                .ok();
                        }
                        eprintln!(
                            "✅ [process_create] Block Storage volume created BEFORE instance: id={}, name={}, size={}GB",
                            vol_id, vol_name, gb
                        );

                        // Track the created volume in instance_volumes with status 'created' (not yet attached)
                        let row_id = Uuid::new_v4();
                        let provider_id: Option<Uuid> =
                            sqlx::query_scalar("SELECT provider_id FROM instances WHERE id = $1")
                                .bind(instance_uuid)
                                .fetch_optional(&pool)
                                .await
                                .ok()
                                .flatten();

                        if let Some(pid) = provider_id {
                            let _ = sqlx::query(
                                r#"
                                INSERT INTO instance_volumes 
                                (id, instance_id, provider_id, zone_code, provider_volume_id, provider_volume_name, volume_type, size_bytes, delete_on_terminate, status, attached_at, is_boot)
                                VALUES ($1, $2, $3, $4, $5, $6, 'sbs_volume', $7, $8, 'created', NULL, FALSE)
                                ON CONFLICT (instance_id, provider_volume_id) DO UPDATE
                                SET status = 'created', deleted_at = NULL
                                WHERE instance_volumes.deleted_at IS NOT NULL
                                "#,
                            )
                            .bind(row_id)
                            .bind(instance_uuid)
                            .bind(pid)
                            .bind(&zone)
                            .bind(&vol_id)
                            .bind(&vol_name)
                            .bind(gb_to_bytes(gb))
                            .bind(delete_on_terminate)
                            .execute(&pool)
                            .await;
                        }
                    }
                    Ok(None) => {
                        let msg = "Provider does not support volume creation".to_string();
                        if let Some(lid) = create_log {
                            let dur = vol_start.elapsed().as_millis() as i32;
                            logger::log_event_complete(&pool, lid, "failed", dur, Some(&msg))
                                .await
                                .ok();
                        }
                        eprintln!("⚠️ [process_create] Volume creation not supported, continuing without pre-created volume");
                    }
                    Err(e) => {
                        let msg = format!("Failed to create data volume before instance: {}", e);
                        if let Some(lid) = create_log {
                            let dur = vol_start.elapsed().as_millis() as i32;
                            logger::log_event_complete(&pool, lid, "failed", dur, Some(&msg))
                                .await
                                .ok();
                        }
                        eprintln!("❌ [process_create] {}", msg);
                        // Continue without volume - we'll try to create it after instance creation as fallback
                    }
                }
            }
        }
    }

    // LOG 3: PROVIDER_CREATE (API call)
    let api_start = Instant::now();
    let log_id_provider = logger::log_event_with_metadata(
        &pool,
        "PROVIDER_CREATE",
        "in_progress",
        instance_uuid,
        None,
        Some(json!({
            "zone": zone,
            "instance_type": instance_type,
            "image_id": image_id,
            "correlation_id": correlation_id_meta,
            "provider": provider_name,
            "has_cloud_init": cloud_init_for_create.is_some(),
            "cloud_init_length": cloud_init_for_create.as_ref().map(|ci| ci.len()).unwrap_or(0),
            "pre_created_volume_id": pre_created_volume_id.as_deref()
        })),
    )
    .await
    .ok();

    // Prepare volumes list for instance creation (if volume was pre-created)
    // Use as_ref() to avoid moving pre_created_volume_id so it can be used later
    let volumes_for_create: Option<Vec<String>> =
        pre_created_volume_id.as_ref().map(|vid| vec![vid.clone()]);
    let volumes_ref: Option<&[String]> = volumes_for_create.as_deref();

    let server_id_result = provider
        .create_instance(
            &zone,
            &instance_type,
            &image_id,
            cloud_init_for_create.as_deref(),
            volumes_ref,
        )
        .await;

    match server_id_result {
        Ok(server_id) => {
            println!("✅ Server Created: {}", server_id);

            if let Some(log_id) = log_id_provider {
                let api_duration = api_start.elapsed().as_millis() as i32;
                let metadata = json!({
                    "server_id": server_id,
                    "zone": zone,
                    "correlation_id": correlation_id_meta,
                    "instance_type": instance_type,
                    "image_id": image_id,
                    "provider": provider_name
                });
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

            // Scaleway automatically applies SSH keys from the project to all instances.
            // No need to set cloud-init - SSH keys are configured automatically.

            // Check if this instance requires diskless boot (needed for volume discovery and resize)
            let requires_diskless = provider.requires_diskless_boot(&instance_type);

            // Discover and track all volumes attached to the instance (including auto-created boot volumes)
            // This ensures storage_count and storage_sizes_gb are populated immediately
            // Track ALL volumes (boot + data) so they can be displayed and cleaned up on termination
            // For Scaleway L4-1-24G: Scaleway creates a Block Storage (sbs_volume) automatically from image snapshot (20GB bootable)
            let mut auto_created_boot_volume_id: Option<String> = None;
            let mut auto_created_boot_volume_size_gb: Option<u64> = None;

            if let Ok(attached_volumes) = provider.list_attached_volumes(&zone, &server_id).await {
                if !attached_volumes.is_empty() {
                    eprintln!(
                        "🔍 [process_create] Discovering volumes for instance {} (found {} volumes)",
                        instance_uuid, attached_volumes.len()
                    );

                    for av in attached_volumes {
                        // For Scaleway L4-1-24G: Scaleway creates a Block Storage automatically (20GB bootable)
                        // We need to resize it to target size before starting the instance
                        // NOTE: Scaleway API may not mark the volume as boot=true, so for diskless instances,
                        // we consider the first sbs_volume as the bootable volume
                        if requires_diskless
                            && av.volume_type == "sbs_volume"
                            && (av.boot || auto_created_boot_volume_id.is_none())
                        {
                            // Only set if not already set (first sbs_volume found)
                            if auto_created_boot_volume_id.is_none() {
                                auto_created_boot_volume_id = Some(av.provider_volume_id.clone());
                                if let Some(size_bytes) = av.size_bytes {
                                    auto_created_boot_volume_size_gb =
                                        Some((size_bytes as u64) / 1_000_000_000);
                                } else {
                                    // Default to 20GB if size not available (Scaleway auto-creates 20GB Block Storage)
                                    auto_created_boot_volume_size_gb = Some(20);
                                }
                                eprintln!(
                                    "🔍 [process_create] Found auto-created Block Storage boot volume: id={}, size={}GB, boot={}",
                                    av.provider_volume_id,
                                    auto_created_boot_volume_size_gb.unwrap_or(20),
                                    av.boot
                                );
                            }
                        }

                        // Track all volumes (including auto-created Block Storage)
                        // Skip tracking local storage volumes for diskless instances - Scaleway shouldn't create them
                        if requires_diskless && av.volume_type == "l_ssd" {
                            eprintln!(
                                "⚠️ [process_create] Unexpected local volume {} found on diskless instance {} - this should not happen",
                                av.provider_volume_id, instance_uuid
                            );
                            // Still track it for cleanup, but log a warning
                        }

                        // Check if this volume is already tracked
                        let exists: bool = sqlx::query_scalar(
                            "SELECT EXISTS(SELECT 1 FROM instance_volumes WHERE instance_id=$1 AND provider_volume_id=$2 AND deleted_at IS NULL)",
                        )
                        .bind(instance_uuid)
                        .bind(&av.provider_volume_id)
                        .fetch_one(&pool)
                        .await
                        .unwrap_or(false);

                        if !exists {
                            let row_id = Uuid::new_v4();
                            let provider_id: Option<Uuid> = sqlx::query_scalar(
                                "SELECT provider_id FROM instances WHERE id = $1",
                            )
                            .bind(instance_uuid)
                            .fetch_optional(&pool)
                            .await
                            .ok()
                            .flatten();

                            if let Some(pid) = provider_id {
                                // If size_bytes is not available from list_attached_volumes, try to fetch it from Block Storage API
                                let mut size_bytes = av.size_bytes.unwrap_or(0);
                                if size_bytes == 0 && av.volume_type == "sbs_volume" {
                                    // Try to fetch size from Block Storage API
                                    eprintln!("🔍 [process_create] Size not available from list_attached_volumes, fetching from Block Storage API for volume {}", av.provider_volume_id);
                                    if let Ok(Some(block_size)) = provider
                                        .get_block_storage_size(&zone, &av.provider_volume_id)
                                        .await
                                    {
                                        size_bytes = block_size as i64;
                                        eprintln!("✅ [process_create] Retrieved size from Block Storage API: {}GB", size_bytes / 1_000_000_000);
                                    } else {
                                        // For auto-created Block Storage volumes on diskless instances, default to 20GB if size cannot be retrieved
                                        if requires_diskless && av.volume_type == "sbs_volume" {
                                            size_bytes = 20_000_000_000; // 20GB default (Scaleway auto-creates 20GB Block Storage)
                                            eprintln!("⚠️ [process_create] Using default size 20GB for auto-created Block Storage volume {} (size not available from API)", av.provider_volume_id);
                                        }
                                    }
                                }

                                eprintln!(
                                    "📦 [process_create] Tracking volume {} for instance {}: type={}, size={}GB, boot={}",
                                    av.provider_volume_id, instance_uuid, av.volume_type,
                                    if size_bytes > 0 { size_bytes / 1_000_000_000 } else { 0 }, av.boot
                                );

                                // For volumes created automatically by Scaleway:
                                // - Boot volumes (boot=true) should be deleted on termination
                                // - Local Storage volumes for RENDER-S (boot=false, volume_type=l_ssd) should also be deleted
                                //   since they're auto-created and attached to the instance
                                // - Block Storage volumes (sbs_volume) auto-created by Scaleway for diskless instances should be deleted
                                //   (Scaleway creates these automatically, they're not user-created data volumes)
                                // - Data volumes we create will have delete_on_terminate set later based on config
                                // Determine if volume should be deleted on instance termination
                                // Boot volumes, local storage, and auto-created diskless volumes should be deleted
                                let delete_on_terminate = av.boot
                                    || av.volume_type == "l_ssd"
                                    || (requires_diskless && av.volume_type == "sbs_volume");

                                let insert_result = sqlx::query(
                                    r#"
                                    INSERT INTO instance_volumes 
                                    (id, instance_id, provider_id, zone_code, provider_volume_id, provider_volume_name, volume_type, size_bytes, delete_on_terminate, status, attached_at, is_boot)
                                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'attached', NOW(), $10)
                                    ON CONFLICT (instance_id, provider_volume_id) DO UPDATE
                                    SET deleted_at = NULL, status = 'attached', attached_at = NOW()
                                    WHERE instance_volumes.deleted_at IS NOT NULL
                                    "#,
                                )
                                .bind(row_id)
                                .bind(instance_uuid)
                                .bind(pid)
                                .bind(&zone)
                                .bind(&av.provider_volume_id)
                                .bind(av.provider_volume_name.as_deref())
                                .bind(&av.volume_type)
                                .bind(size_bytes)
                                .bind(delete_on_terminate)
                                .bind(av.boot)
                                .execute(&pool)
                                .await;

                                match insert_result {
                                    Ok(result) => {
                                        if result.rows_affected() > 0 {
                                            eprintln!("✅ [process_create] Successfully inserted volume {} into DB (first discovery)", av.provider_volume_id);
                                        } else {
                                            eprintln!("ℹ️ [process_create] Volume {} already exists in DB (ON CONFLICT - first discovery)", av.provider_volume_id);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("❌ [process_create] Failed to insert volume {} into DB (first discovery): {:?}", av.provider_volume_id, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Security Groups configuration will be done AFTER IP retrieval (see below)
            // This ensures we have the IP address before configuring firewall rules
            // The old code that checked WORKER_EXPOSE_PORTS here has been moved to after IP retrieval

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
            if data_conf.is_none() && auto_install && is_worker_target {
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
                    // Some instance types have auto-created storage (e.g., Scaleway RENDER-S with Local Storage)
                    // Check via provider abstraction if we should skip data volume creation
                    if is_worker_target && provider.should_skip_data_volume_creation(&instance_type)
                    {
                        eprintln!(
                            "ℹ️ [process_create] Skipping data volume creation for instance {} (type {}) - using auto-created storage",
                            instance_uuid, instance_type
                        );
                        // Still track any volumes that provider created automatically
                        if provider.has_auto_created_storage(&instance_type) {
                            if let Ok(attached) =
                                provider.list_attached_volumes(&zone, &server_id).await
                            {
                                for av in attached {
                                    // Track auto-created volumes if not already tracked
                                    let exists: bool = sqlx::query_scalar(
                                        "SELECT EXISTS(SELECT 1 FROM instance_volumes WHERE instance_id=$1 AND provider_volume_id=$2 AND deleted_at IS NULL)",
                                    )
                                    .bind(instance_uuid)
                                    .bind(&av.provider_volume_id)
                                    .fetch_one(&pool)
                                    .await
                                    .unwrap_or(false);

                                    if !exists {
                                        let row_id = Uuid::new_v4();
                                        let provider_id: Option<Uuid> = sqlx::query_scalar(
                                            "SELECT provider_id FROM instances WHERE id = $1",
                                        )
                                        .bind(instance_uuid)
                                        .fetch_optional(&pool)
                                        .await
                                        .ok()
                                        .flatten();

                                        if let Some(pid) = provider_id {
                                            let size_bytes = av.size_bytes.unwrap_or(0);
                                            eprintln!(
                                                "📦 [process_create] Tracking auto-created volume {} for instance {}: type={}, size={}GB, boot={}",
                                                av.provider_volume_id, instance_uuid, av.volume_type,
                                                if size_bytes > 0 { size_bytes / 1_000_000_000 } else { 0 }, av.boot
                                            );

                                            // For auto-created volumes:
                                            // - They should be deleted on termination since they're auto-created and attached to the instance
                                            // - Set delete_on_terminate=true for auto-created volumes
                                            let delete_on_terminate_local = true;

                                            let insert_result = sqlx::query(
                                                r#"
                                                INSERT INTO instance_volumes 
                                                (id, instance_id, provider_id, zone_code, provider_volume_id, provider_volume_name, volume_type, size_bytes, delete_on_terminate, status, attached_at, is_boot)
                                                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'attached', NOW(), $10)
                                                ON CONFLICT (instance_id, provider_volume_id) DO UPDATE
                                                SET deleted_at = NULL, status = 'attached', attached_at = NOW(), delete_on_terminate = $9
                                                WHERE instance_volumes.deleted_at IS NOT NULL
                                                "#,
                                            )
                                            .bind(row_id)
                                            .bind(instance_uuid)
                                            .bind(pid)
                                            .bind(&zone)
                                            .bind(&av.provider_volume_id)
                                            .bind(av.provider_volume_name.as_deref())
                                            .bind(&av.volume_type)
                                            .bind(size_bytes)
                                            .bind(delete_on_terminate_local)
                                            .bind(av.boot)
                                            .execute(&pool)
                                            .await;

                                            match insert_result {
                                                Ok(result) => {
                                                    if result.rows_affected() > 0 {
                                                        eprintln!("✅ [process_create] Successfully inserted auto-created volume {} into DB", av.provider_volume_id);
                                                    } else {
                                                        eprintln!("ℹ️ [process_create] Auto-created volume {} already exists in DB (ON CONFLICT)", av.provider_volume_id);
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!("❌ [process_create] Failed to insert auto-created volume {} into DB: {:?}", av.provider_volume_id, e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Skip data volume creation - auto-created storage is handled above
                    } else {
                        // For instances without auto-created storage, proceed with data volume creation
                        // Note: Boot volumes are already tracked above (after PROVIDER_CREATE).
                        if is_worker_target {
                            if let Ok(attached) =
                                provider.list_attached_volumes(&zone, &server_id).await
                            {
                                for av in attached {
                                    // Update existing volumes with complete metadata if needed
                                    let exists: bool = sqlx::query_scalar(
                                        "SELECT EXISTS(SELECT 1 FROM instance_volumes WHERE instance_id=$1 AND provider_volume_id=$2 AND deleted_at IS NULL)",
                                )
                                .bind(instance_uuid)
                                .bind(&av.provider_volume_id)
                                .fetch_one(&pool)
                                .await
                                .unwrap_or(false);

                                    if exists {
                                        // Best effort: if we previously stored incomplete metadata (size/name),
                                        // update it now so UI can show the expected sizes.
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
                                                  AND deleted_at IS NULL
                                            "#,
                                        )
                                        .bind(instance_uuid)
                                        .bind(&av.provider_volume_id)
                                        .bind(av.provider_volume_name.as_deref())
                                        .bind(av.size_bytes.unwrap_or(0))
                                        .bind(av.boot)
                                        .execute(&pool)
                                        .await;
                                        }
                                    }
                                }
                            }
                        }
                    } // Close the else block for instances without auto-created storage

                    // Attach data volume if not skipped AND not diskless boot
                    // NOTE: requires_diskless is already defined above (after instance creation)
                    // NOTE: For diskless boot instances, skip attachment here - attach_block_storage_after_boot will handle it AFTER startup and SSH
                    // If pre_created_volume_id exists, use it (volume created BEFORE instance)
                    // Otherwise, create volume now (fallback path - should not happen in normal flow)
                    if requires_diskless {
                        eprintln!(
                            "⏭️ [process_create] Skipping Block Storage attachment for diskless instance {} - attach_block_storage_after_boot will handle it AFTER startup and SSH",
                            server_id
                        );
                    } else if !provider.should_skip_data_volume_creation(&instance_type) {
                        let vol_id_to_attach_opt: Option<String> = if let Some(pre_vol_id) =
                            &pre_created_volume_id
                        {
                            // Volume was pre-created before instance creation - use it
                            eprintln!(
                                "ℹ️ [process_create] Using pre-created Block Storage volume {} (created before instance)",
                                pre_vol_id
                            );
                            Some(pre_vol_id.clone())
                        } else {
                            // Fallback: create volume now (should not happen in normal flow)
                            eprintln!(
                                "⚠️ [process_create] No pre-created volume found - creating data volume AFTER instance creation (fallback): size={}GB",
                                gb
                            );
                            let vol_name = format!("inventiv-data-{}", instance_uuid);
                            let create_log = logger::log_event_with_metadata(
                        &pool,
                        "PROVIDER_CREATE_VOLUME",
                        "in_progress",
                        instance_uuid,
                        None,
                                Some(json!({"zone": zone, "server_id": server_id, "name": vol_name, "size_gb": gb, "correlation_id": correlation_id_meta, "post_create": true})),
                    )
                    .await
                    .ok();
                            let vol_start = Instant::now();
                            let volume_type = provider.get_data_volume_type(&instance_type);
                            let created = provider
                                .create_volume(
                                    &zone,
                                    &vol_name,
                                    gb_to_bytes(gb),
                                    &volume_type,
                                    perf_iops,
                                )
                                .await;
                            match created {
                                Ok(Some(id)) => {
                                    if let Some(lid) = create_log {
                                        let dur = vol_start.elapsed().as_millis() as i32;
                                        logger::log_event_complete(
                                            &pool, lid, "success", dur, None,
                                        )
                                        .await
                                        .ok();
                                    }
                                    // Track the created volume
                                    let row_id = Uuid::new_v4();
                                    let provider_id: Option<Uuid> = sqlx::query_scalar(
                                        "SELECT provider_id FROM instances WHERE id = $1",
                                    )
                                    .bind(instance_uuid)
                                    .fetch_optional(&pool)
                                    .await
                                    .ok()
                                    .flatten();

                                    if let Some(pid) = provider_id {
                                        let _ = sqlx::query(
                                            r#"
                                            INSERT INTO instance_volumes 
                                            (id, instance_id, provider_id, zone_code, provider_volume_id, provider_volume_name, volume_type, size_bytes, delete_on_terminate, status, attached_at, is_boot)
                                            VALUES ($1, $2, $3, $4, $5, $6, 'sbs_volume', $7, $8, 'created', NULL, FALSE)
                                            ON CONFLICT (instance_id, provider_volume_id) DO UPDATE
                                            SET status = 'created', deleted_at = NULL
                                            WHERE instance_volumes.deleted_at IS NOT NULL
                                            "#,
                                        )
                                        .bind(row_id)
                            .bind(instance_uuid)
                                        .bind(pid)
                                        .bind(&zone)
                                        .bind(&id)
                                        .bind(&vol_name)
                                        .bind(gb_to_bytes(gb))
                                        .bind(delete_on_terminate)
                            .execute(&pool)
                            .await;
                                    }
                                    Some(id)
                                }
                                Ok(None) => {
                                    let msg =
                                        "Provider does not support volume creation".to_string();
                                    if let Some(lid) = create_log {
                                        let dur = vol_start.elapsed().as_millis() as i32;
                                        logger::log_event_complete(
                                            &pool,
                                            lid,
                                            "failed",
                                            dur,
                                            Some(&msg),
                                        )
                                        .await
                                        .ok();
                                    }
                                    eprintln!("⚠️ [process_create] Provider does not support volume creation - skipping attachment");
                                    None // Skip attachment, continue provisioning
                                }
                                Err(e) => {
                                    let msg = format!("Failed to create data volume: {}", e);
                                    if let Some(lid) = create_log {
                                        let dur = vol_start.elapsed().as_millis() as i32;
                                        logger::log_event_complete(
                                            &pool,
                                            lid,
                                            "failed",
                                            dur,
                                            Some(&msg),
                                        )
                                        .await
                                        .ok();
                                    }
                                    eprintln!("❌ [process_create] {}", msg);
                                    // Don't fail provisioning - continue without volume attachment
                                    None
                                }
                            }
                        };

                        // If volume creation succeeded, attach it
                        if let Some(vol_id_to_attach) = vol_id_to_attach_opt {
                            // Attach the volume (either pre-created or just created)
                            let attach_log = logger::log_event_with_metadata(
                            &pool,
                            "PROVIDER_ATTACH_VOLUME",
                            "in_progress",
                            instance_uuid,
                            None,
                            Some(json!({"zone": zone, "server_id": server_id, "volume_id": vol_id_to_attach, "correlation_id": correlation_id_meta, "pre_created": pre_created_volume_id.is_some()})),
                        )
                        .await
                        .ok();
                            let attach_start = Instant::now();
                            let attach_res = provider
                                .attach_volume(
                                    &zone,
                                    &server_id,
                                    &vol_id_to_attach,
                                    delete_on_terminate,
                                )
                                .await;
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
                                // Update volume status to 'attached' using provider_volume_id
                                let _ = sqlx::query(
                                r#"
                                UPDATE instance_volumes 
                                SET status='attached', attached_at=NOW() 
                                WHERE instance_id=$1 AND provider_volume_id=$2 AND deleted_at IS NULL
                                "#
                            )
                            .bind(instance_uuid)
                            .bind(&vol_id_to_attach)
                            .execute(&pool)
                            .await;
                                eprintln!("✅ [process_create] Block Storage volume {} attached successfully", vol_id_to_attach);
                            } else {
                                let msg = "Failed to attach data volume".to_string();
                                // Update volume status to 'failed' using provider_volume_id
                                let _ = sqlx::query(
                                r#"
                                UPDATE instance_volumes 
                                SET status='failed', error_message=$3 
                                WHERE instance_id=$1 AND provider_volume_id=$2 AND deleted_at IS NULL
                                "#
                            )
                            .bind(instance_uuid)
                            .bind(&vol_id_to_attach)
                                .bind(&msg)
                                .execute(&pool)
                                .await;
                                // Best-effort cleanup of the created volume to avoid cost leak.
                                let _ = provider.delete_volume(&zone, &vol_id_to_attach).await;
                                let _ = sqlx::query(
                                    r#"
                                UPDATE instance_volumes 
                                SET status='deleted', deleted_at=NOW() 
                                WHERE instance_id=$1 AND provider_volume_id=$2
                                "#,
                                )
                                .bind(instance_uuid)
                                .bind(&vol_id_to_attach)
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
                        } else {
                            eprintln!("⚠️ [process_create] Skipping Block Storage attachment - volume creation failed or not supported");
                        } // Close the if let Some(vol_id_to_attach) block
                    } // Close the data volume attachment block
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
                Some(json!({
                    "zone": zone,
                    "server_id": server_id,
                    "image_id": image_id,
                    "correlation_id": correlation_id_meta,
                    "provider": provider_name,
                    "instance_type": instance_type
                })),
            )
            .await
            .ok();

            // 3. Resize auto-created Block Storage if required (BEFORE starting the instance)
            // For Scaleway L4-1-24G: Scaleway creates a Block Storage automatically (20GB bootable from image snapshot)
            // We need to resize it based on the model size before starting the instance
            if requires_diskless {
                if let Some(boot_volume_id) = &auto_created_boot_volume_id {
                    let current_size_gb = auto_created_boot_volume_size_gb.unwrap_or(20);

                    // Calculate target size based on model requirements using worker_storage logic
                    let (model_from_db_for_size, _) =
                        resolve_instance_model_and_volume(&pool, instance_uuid).await;
                    let model_code = model_from_db_for_size.as_deref().unwrap_or("");

                    // Get default size from provider settings or env (fallback to 200GB)
                    let provider_id: Option<Uuid> =
                        sqlx::query_scalar("SELECT provider_id FROM instances WHERE id = $1")
                            .bind(instance_uuid)
                            .fetch_optional(&pool)
                            .await
                            .ok()
                            .flatten();

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

                    // Use worker_storage logic to determine optimal size based on model
                    let recommended_gb =
                        worker_storage::recommended_data_volume_gb(model_code, default_gb)
                            .unwrap_or(default_gb);

                    let target_size_gb = recommended_gb as u64; // Convert to u64 for resize_block_storage

                    if current_size_gb < target_size_gb {
                        eprintln!(
                            "🔵 [process_create] Resizing auto-created Block Storage {} from {}GB to {}GB BEFORE startup",
                            boot_volume_id, current_size_gb, target_size_gb
                        );

                        let resize_log = logger::log_event_with_metadata(
                            &pool,
                            "PROVIDER_VOLUME_RESIZE",
                            "in_progress",
                            instance_uuid,
                            None,
                            Some(json!({
                                "zone": zone,
                                "server_id": server_id,
                                "volume_id": boot_volume_id,
                                "current_size_gb": current_size_gb,
                                "target_size_gb": target_size_gb,
                                "correlation_id": correlation_id_meta,
                                "provider": provider_name
                            })),
                        )
                        .await
                        .ok();

                        let resize_start = Instant::now();

                        eprintln!("🔵 [process_create] Calling resize_block_storage for volume {} to {}GB", boot_volume_id, target_size_gb);
                        match provider
                            .resize_block_storage(&zone, boot_volume_id, target_size_gb)
                            .await
                        {
                            Ok(true) => {
                                eprintln!("✅ [process_create] resize_block_storage returned Ok(true) for volume {}", boot_volume_id);
                                if let Some(lid) = resize_log {
                                    let duration = resize_start.elapsed().as_millis() as i32;
                                    eprintln!("🔵 [process_create] Completing PROVIDER_VOLUME_RESIZE log {} with success (duration: {}ms)", lid, duration);
                                    if let Err(e) = logger::log_event_complete(
                                        &pool, lid, "success", duration, None,
                                    )
                                    .await
                                    {
                                        eprintln!("❌ [process_create] Failed to complete PROVIDER_VOLUME_RESIZE log {}: {:?}", lid, e);
                                    } else {
                                        eprintln!("✅ [process_create] Successfully completed PROVIDER_VOLUME_RESIZE log {}", lid);
                                    }
                                } else {
                                    eprintln!("⚠️ [process_create] No resize_log ID available to complete");
                                }

                                // Update volume size in DB
                                let update_result = sqlx::query(
                                    r#"
                                    UPDATE instance_volumes
                                    SET size_bytes = $3
                                    WHERE instance_id = $1 AND provider_volume_id = $2 AND deleted_at IS NULL
                                    "#
                                )
                                .bind(instance_uuid)
                                .bind(boot_volume_id)
                                .bind((target_size_gb * 1_000_000_000) as i64)
                                .execute(&pool)
                                .await;

                                if let Err(e) = update_result {
                                    eprintln!("⚠️ [process_create] Failed to update volume size in DB: {:?}", e);
                                } else {
                                    eprintln!(
                                        "✅ [process_create] Updated volume size in DB to {}GB",
                                        target_size_gb
                                    );
                                }

                                eprintln!("✅ [process_create] Successfully resized Block Storage {} to {}GB", boot_volume_id, target_size_gb);
                            }
                            Ok(false) => {
                                eprintln!("⚠️ [process_create] Block Storage resize not supported by provider");
                                if let Some(lid) = resize_log {
                                    let duration = resize_start.elapsed().as_millis() as i32;
                                    logger::log_event_complete(
                                        &pool,
                                        lid,
                                        "failed",
                                        duration,
                                        Some("Provider does not support resize"),
                                    )
                                    .await
                                    .ok();
                                }
                            }
                            Err(e) => {
                                let error_msg = format!(
                                    "Failed to resize Block Storage {} from {}GB to {}GB: {}",
                                    boot_volume_id, current_size_gb, target_size_gb, e
                                );

                                eprintln!("❌ [process_create] {}", error_msg);

                                if let Some(lid) = resize_log {
                                    let duration = resize_start.elapsed().as_millis() as i32;
                                    logger::log_event_complete(
                                        &pool,
                                        lid,
                                        "failed",
                                        duration,
                                        Some(&error_msg),
                                    )
                                    .await
                                    .ok();
                                }

                                // Don't fail provisioning - instance can still boot with 20GB (though not ideal)
                                eprintln!("⚠️ [process_create] Continuing with {}GB Block Storage (resize failed)", current_size_gb);
                            }
                        }
                    } else {
                        eprintln!("ℹ️ [process_create] Block Storage {} already has sufficient size ({}GB >= {}GB)", boot_volume_id, current_size_gb, target_size_gb);
                    }
                } else {
                    eprintln!("⚠️ [process_create] No auto-created Block Storage found for diskless instance {} - this may cause boot failure", server_id);
                }
            }

            // 4. Power On and ensure server reaches "running" state
            // Generic logic: works for providers that support get_server_state()
            // and providers that don't (e.g., Mock - they just call start_instance() once)

            println!("🔌 Starting server {}...", server_id);
            let mut start_success = false;
            let mut last_state: Option<String> = None;

            // Check if provider supports state checking
            let state_check_result = provider.get_server_state(&zone, &server_id).await;
            let provider_supports_state_check = matches!(state_check_result, Ok(Some(_)));

            if provider_supports_state_check {
                // Provider supports state checking (e.g., Scaleway)
                // Loop until server is "running" or timeout
                for start_attempt in 1..=60 {
                    if let Ok(Some(state)) = provider.get_server_state(&zone, &server_id).await {
                        last_state = Some(state.clone());
                        if state == "running" {
                            println!(
                                "✅ Server {} is now running (attempt {})",
                                server_id, start_attempt
                            );
                            start_success = true;
                            break;
                        } else if state == "stopped" || state == "stopped_in_place" {
                            // Server is stopped, try to poweron
                            println!(
                                "🔌 Server {} is {}, attempting poweron (attempt {})",
                                server_id, state, start_attempt
                            );
                            match provider.start_instance(&zone, &server_id).await {
                                Ok(true) => {
                                    println!("✅ Poweron command sent successfully");
                                    // Wait a bit for state to change
                                    sleep(Duration::from_secs(3)).await;
                                }
                                Ok(false) => {
                                    // This shouldn't happen with improved error handling, but keep for safety
                                    println!("⚠️ Poweron command returned false");
                                    if start_attempt >= 60 {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    let msg =
                                        format!("Failed to start instance on provider: {:?}", e);

                                    // Check if this is a normal retry (instance in transitional state) vs real error
                                    let (is_retry, retry_msg) = is_normal_retry(
                                        provider.as_ref(),
                                        &zone,
                                        &server_id,
                                        Some(&msg),
                                    )
                                    .await;

                                    if is_retry {
                                        println!(
                                            "⏳ {}",
                                            retry_msg.as_ref().unwrap_or(
                                                &"Instance is starting - retrying".to_string()
                                            )
                                        );
                                        if let Some(lid) = log_id_start {
                                            let duration = start_api.elapsed().as_millis() as i32;
                                            logger::log_event_complete(
                                                &pool,
                                                lid,
                                                "retry",
                                                duration,
                                                retry_msg.as_deref(),
                                            )
                                            .await
                                            .ok();
                                        }
                                        if let Some(log_id) = log_id_execute {
                                            let duration = start.elapsed().as_millis() as i32;
                                            logger::log_event_complete(
                                                &pool,
                                                log_id,
                                                "retry",
                                                duration,
                                                retry_msg.as_deref(),
                                            )
                                            .await
                                            .ok();
                                        }
                                    } else {
                                        println!("❌ {}", msg);
                                        if let Some(lid) = log_id_start {
                                            let duration = start_api.elapsed().as_millis() as i32;
                                            logger::log_event_complete(
                                                &pool,
                                                lid,
                                                "failed",
                                                duration,
                                                Some(&msg),
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
                                                Some(&msg),
                                            )
                                            .await
                                            .ok();
                                        }
                                    }

                                    // Best-effort cleanup
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
                                    let terminate_res =
                                        provider.terminate_instance(&zone, &server_id).await;
                                    if let Some(lid) = terminate_log {
                                        let dur = terminate_start.elapsed().as_millis() as i32;
                                        match &terminate_res {
                                            Ok(true) => logger::log_event_complete(
                                                &pool, lid, "success", dur, None,
                                            )
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
                                    for (vol_row_id, provider_volume_id, delete_on_terminate) in
                                        vols
                                    {
                                        if !delete_on_terminate {
                                            continue;
                                        }
                                        let _ = provider
                                            .delete_volume(&zone, &provider_volume_id)
                                            .await;
                                        let _ = sqlx::query(
                                        "UPDATE instance_volumes SET status='deleted', deleted_at=NOW() WHERE id=$1"
                                    )
                                    .bind(vol_row_id)
                                    .execute(&pool)
                                    .await;
                                    }

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
                                }
                            }
                        } else {
                            // Server is in another state (starting, etc.), wait for it to change
                            if start_attempt % 10 == 0 {
                                println!(
                                    "⏳ Server {} state: {} (attempt {}/60)",
                                    server_id, state, start_attempt
                                );
                            }
                        }
                    }

                    if start_attempt < 60 {
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            } else {
                // Provider doesn't support state checking (e.g., Mock)
                // Just call start_instance() once - if it succeeds, we're done
                println!(
                    "🔌 Provider doesn't support state checking, calling start_instance() directly"
                );
                match provider.start_instance(&zone, &server_id).await {
                    Ok(true) => {
                        println!("✅ start_instance() succeeded");
                        start_success = true;
                    }
                    Ok(false) => {
                        let msg = "start_instance() returned false";
                        println!("❌ {}", msg);
                        if let Some(lid) = log_id_start {
                            let duration = start_api.elapsed().as_millis() as i32;
                            logger::log_event_complete(&pool, lid, "failed", duration, Some(msg))
                                .await
                                .ok();
                        }
                    }
                    Err(e) => {
                        let msg = format!("Failed to start instance on provider: {:?}", e);

                        // Check if this is a normal retry (instance in transitional state) vs real error
                        let (is_retry, retry_msg) =
                            is_normal_retry(provider.as_ref(), &zone, &server_id, Some(&msg)).await;

                        if is_retry {
                            println!(
                                "⏳ {}",
                                retry_msg
                                    .as_ref()
                                    .unwrap_or(&"Instance is starting - retrying".to_string())
                            );
                            if let Some(lid) = log_id_start {
                                let duration = start_api.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    lid,
                                    "retry",
                                    duration,
                                    retry_msg.as_deref(),
                                )
                                .await
                                .ok();
                            }
                            if let Some(log_id) = log_id_execute {
                                let duration = start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    log_id,
                                    "retry",
                                    duration,
                                    retry_msg.as_deref(),
                                )
                                .await
                                .ok();
                            }
                        } else {
                            println!("❌ {}", msg);
                            if let Some(lid) = log_id_start {
                                let duration = start_api.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    lid,
                                    "failed",
                                    duration,
                                    Some(&msg),
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
                                    Some(&msg),
                                )
                                .await
                                .ok();
                            }
                        }

                        // Best-effort cleanup
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
                    }
                }
            }

            if !start_success {
                let msg = if let Some(state) = last_state {
                    format!(
                        "Server {} did not reach 'running' state after 2 minutes. Last state: {}",
                        server_id, state
                    )
                } else {
                    format!("Server {} could not be started after 2 minutes", server_id)
                };
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

                // Best-effort cleanup
                let terminate_log = logger::log_event_with_metadata(
                    &pool,
                    "PROVIDER_TERMINATE",
                    "in_progress",
                    instance_uuid,
                    None,
                    Some(json!({"zone": zone, "server_id": server_id, "correlation_id": correlation_id_meta, "reason": "start_timeout_cleanup"})),
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

                let next_status = match terminate_res {
                    Ok(true) => "terminating",
                    _ => "provisioning_failed",
                };
                let _ = sqlx::query(
                    "UPDATE instances
                     SET status = $2::instance_status,
                         error_code = COALESCE(error_code, 'PROVIDER_START_TIMEOUT'),
                         error_message = COALESCE($3, error_message),
                         failed_at = COALESCE(failed_at, NOW()),
                         deletion_reason = COALESCE(deletion_reason, 'provider_start_timeout_cleanup')
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

            // Immediately move the instance to BOOTING after provider start succeeds, even if IP discovery
            // is delayed/unavailable. This prevents "stuck provisioning" when async tasks are interrupted
            // and allows health-check convergence once the worker later reports a routable IP.
            // IMPORTANT: Transition to booting after PROVIDER_START to allow progress calculation to work correctly
            let _ = sqlx::query(
                r#"
                UPDATE instances
                SET status = 'booting',
                    boot_started_at = COALESCE(boot_started_at, NOW()),
                    last_health_check = NULL
                WHERE id = $1
                  AND status = 'provisioning'
                  AND provider_instance_id IS NOT NULL
                  AND status NOT IN ('terminating', 'terminated')
                "#,
            )
            .bind(instance_uuid)
            .execute(&pool)
            .await;

            // 3.5. Wait for server to be running, then retrieve IP
            // Scaleway assigns IP dynamically only after the server reaches "running" state.
            // This matches the behavior in scw_instance_provision.sh which waits for "running" before checking IP.
            println!(
                "⏳ Waiting for server {} to be running before retrieving IP...",
                server_id
            );
            let mut server_running = false;
            let mut last_state: Option<String> = None;
            for wait_attempt in 1..=150 {
                // Check server state (if provider supports it)
                if let Ok(Some(state)) = provider.get_server_state(&zone, &server_id).await {
                    last_state = Some(state.clone());
                    if state == "running" {
                        println!(
                            "✅ Server {} is now running (attempt {})",
                            server_id, wait_attempt
                        );
                        server_running = true;
                        break;
                    } else if wait_attempt % 10 == 0 {
                        // Log every 10th attempt to avoid spam
                        println!(
                            "⏳ Server {} state: {} (attempt {}/150, ~{}s elapsed)",
                            server_id,
                            state,
                            wait_attempt,
                            wait_attempt * 2
                        );
                    }
                } else {
                    // Provider doesn't support get_server_state, or API call failed
                    // Proceed anyway - get_instance_ip will handle it
                    println!(
                        "⚠️ Could not retrieve server state, proceeding with IP retrieval anyway"
                    );
                    break;
                }
                if wait_attempt < 150 {
                    sleep(Duration::from_secs(2)).await;
                }
            }
            if !server_running {
                if let Some(state) = last_state {
                    println!("⚠️ Server {} did not reach 'running' state after 5 minutes. Last state: {}", server_id, state);
                } else {
                    println!(
                        "⚠️ Server {} state could not be determined after 5 minutes",
                        server_id
                    );
                }
            }

            // 3.6. Retrieve IP with exponential backoff
            // Scaleway assigns dynamic IPs only after the server reaches "running" state
            // Wait a bit if server just started to ensure IP is assigned
            if server_running {
                println!("⏳ Waiting 5s for Scaleway to assign IP address...");
                sleep(Duration::from_secs(5)).await;
            }

            println!("🔍 Retrieving IP address for {}...", server_id);
            let mut ip_address: Option<String> = None;
            let ip_api = Instant::now();
            let log_id_ip = logger::log_event_with_metadata(
                &pool,
                "PROVIDER_GET_IP",
                "in_progress",
                instance_uuid,
                None,
                Some(json!({
                    "zone": zone,
                    "server_id": server_id,
                    "correlation_id": correlation_id_meta,
                    "max_attempts": 10,
                    "server_running": server_running
                })),
            )
            .await
            .ok();

            // Use exponential backoff: 2s, 3s, 5s, 8s, 13s, 21s, 34s, 55s, 89s, 144s
            // Total max wait: ~470 seconds (~8 minutes) if IP is not available
            let backoff_delays = vec![2, 3, 5, 8, 13, 21, 34, 55, 89, 144];
            for (attempt_idx, delay_secs) in backoff_delays.iter().enumerate() {
                let attempt = attempt_idx + 1;

                // Verify server is still running before each attempt
                if let Ok(Some(state)) = provider.get_server_state(&zone, &server_id).await {
                    if state != "running" {
                        eprintln!(
                            "⚠️ Server {} is not running (state: {}), IP may not be available",
                            server_id, state
                        );
                    }
                }

                match provider.get_instance_ip(&zone, &server_id).await {
                    Ok(Some(ip)) => {
                        println!("✅ IP Address retrieved: {} (attempt {})", ip, attempt);
                        ip_address = Some(ip);
                        break;
                    }
                    Ok(None) => {
                        if attempt < backoff_delays.len() {
                            println!(
                                "⏳ IP not available yet, waiting {}s before retry (attempt {}/{})",
                                delay_secs,
                                attempt,
                                backoff_delays.len()
                            );
                            sleep(Duration::from_secs(*delay_secs)).await;
                        } else {
                            println!("⚠️ IP not available after {} attempts (server may still be starting or IP assignment delayed)", attempt);
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Error retrieving IP: {}", e);
                        // Don't break on first error - retry with backoff
                        if attempt < backoff_delays.len() {
                            sleep(Duration::from_secs(*delay_secs)).await;
                        } else {
                            break;
                        }
                    }
                }
            }

            if let Some(lid) = log_id_ip {
                let duration = ip_api.elapsed().as_millis() as i32;
                let meta = json!({
                    "ip_address": ip_address,
                    "zone": zone,
                    "server_id": server_id,
                    "correlation_id": correlation_id_meta,
                    "server_running": server_running
                });
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
                        Some("IP not available after retries (server may still be starting)"),
                        Some(meta),
                    )
                    .await
                    .ok();
                }
            }

            // Configure Security Groups (AFTER IP retrieval, BEFORE SSH check)
            // For Scaleway: Open ports SSH (22), worker HTTP (8000), worker metrics (8080)
            if ip_address.is_some() && auto_install && is_worker_target {
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
                        sqlx::query_scalar::<_, i64>(
                            "SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_HEALTH_PORT'",
                        )
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
                        sqlx::query_scalar::<_, i64>(
                            "SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_PORT'",
                        )
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

                    // Include SSH port (22) in addition to worker ports
                    let mut ports_to_open = vec![22u16]; // SSH port
                    ports_to_open.push(worker_vllm_port);
                    ports_to_open.push(worker_health_port);

                    let security_group_log = logger::log_event_with_metadata(
                        &pool,
                        "PROVIDER_SECURITY_GROUP",
                        "in_progress",
                        instance_uuid,
                        None,
                        Some(json!({
                            "zone": zone,
                            "server_id": server_id,
                            "ports": ports_to_open,
                            "correlation_id": correlation_id_meta,
                            "provider": provider_name
                        })),
                    )
                    .await
                    .ok();

                    let security_group_start = Instant::now();

                    match provider
                        .ensure_inbound_tcp_ports(&zone, &server_id, ports_to_open.clone())
                        .await
                    {
                        Ok(true) => {
                            if let Some(lid) = security_group_log {
                                let duration = security_group_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(&pool, lid, "success", duration, None)
                                    .await
                                    .ok();
                            }
                            eprintln!(
                                "✅ [process_create] Security Groups configured: ports opened (SSH: 22, worker: {}/{})",
                                worker_vllm_port, worker_health_port
                            );
                        }
                        Ok(false) => {
                            if let Some(lid) = security_group_log {
                                let duration = security_group_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    lid,
                                    "failed",
                                    duration,
                                    Some("Provider does not support ensure_inbound_tcp_ports"),
                                )
                                .await
                                .ok();
                            }
                            eprintln!("⚠️ [process_create] Provider does not support ensure_inbound_tcp_ports (skipped)");
                        }
                        Err(e) => {
                            if let Some(lid) = security_group_log {
                                let duration = security_group_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(
                                    &pool,
                                    lid,
                                    "failed",
                                    duration,
                                    Some(&e.to_string()),
                                )
                                .await
                                .ok();
                            }
                            eprintln!(
                                "⚠️ [process_create] Failed to configure Security Groups: {}",
                                e
                            );
                        }
                    }
                }
            }

            // Check SSH accessibility (AFTER Security Groups configuration)
            // For Scaleway: SSH should be accessible after ~20 seconds
            if let Some(ip_for_ssh) = ip_address
                .as_ref()
                .filter(|_| auto_install && is_worker_target)
            {
                eprintln!("⏳ [process_create] Waiting for SSH to become accessible on {} (max 3 minutes)...", ip_for_ssh);

                let ssh_check_log = logger::log_event_with_metadata(
                    &pool,
                    "WORKER_SSH_ACCESSIBLE",
                    "in_progress",
                    instance_uuid,
                    None,
                    Some(json!({
                        "zone": zone,
                        "server_id": server_id,
                        "ip_address": ip_for_ssh,
                        "correlation_id": correlation_id_meta,
                        "provider": provider_name,
                        "max_wait_seconds": 180
                    })),
                )
                .await
                .ok();

                let ssh_check_start = Instant::now();
                let mut ssh_accessible = false;
                let max_wait_seconds = 180; // 3 minutes max (as per validation test)
                let check_interval = 10; // Check every 10 seconds
                let max_attempts = max_wait_seconds / check_interval; // 18 attempts

                for ssh_check_attempt in 1..=max_attempts {
                    if check_ssh_accessible(ip_for_ssh).await {
                        let elapsed_seconds = ssh_check_attempt * check_interval;
                        eprintln!("✅ [process_create] SSH is accessible on {} after {} seconds (attempt {}/{})", 
                            ip_for_ssh, elapsed_seconds, ssh_check_attempt, max_attempts);
                        ssh_accessible = true;

                        if let Some(lid) = ssh_check_log {
                            let duration = ssh_check_start.elapsed().as_millis() as i32;
                            logger::log_event_complete_with_metadata(
                                &pool,
                                lid,
                                "success",
                                duration,
                                None,
                                Some(json!({
                                    "ip_address": ip_for_ssh,
                                    "elapsed_seconds": elapsed_seconds,
                                    "attempt": ssh_check_attempt
                                })),
                            )
                            .await
                            .ok();
                        }

                        // Transition to "installing" status when SSH becomes accessible
                        // This indicates we're ready to start worker installation
                        if is_worker_target {
                            eprintln!("🔄 [process_create] Transitioning instance {} from booting to installing (SSH accessible)", instance_uuid);
                            match state_machine::booting_to_installing(
                                &pool,
                                instance_uuid,
                                "SSH accessible - starting worker installation",
                            ).await {
                                Ok(true) => eprintln!("✅ [process_create] Successfully transitioned to installing"),
                                Ok(false) => eprintln!("⚠️ [process_create] Transition to installing skipped (already in different status)"),
                                Err(e) => eprintln!("❌ [process_create] Failed to transition to installing: {:?}", e),
                            }
                        } else {
                            eprintln!("ℹ️ [process_create] Skipping transition to installing (not a worker target: instance_type={})", instance_type);
                        }

                        break;
                    }
                    if ssh_check_attempt < max_attempts {
                        if ssh_check_attempt % 3 == 0 {
                            eprintln!("⏳ [process_create] SSH not yet accessible on {} (attempt {}/{}, ~{}s elapsed)", 
                                ip_for_ssh, ssh_check_attempt, max_attempts, ssh_check_attempt * check_interval);
                        }
                        sleep(Duration::from_secs(check_interval)).await;
                    }
                }

                if !ssh_accessible {
                    let elapsed_seconds = max_attempts * check_interval;
                    eprintln!("❌ [process_create] SSH did not become accessible on {} after {} seconds (3 minutes)", ip_for_ssh, elapsed_seconds);

                    // Check if instance is still starting (normal retry scenario)
                    let (is_retry, retry_msg) = is_normal_retry(
                        provider.as_ref(),
                        &zone,
                        &server_id,
                        Some(&format!(
                            "SSH not accessible after {} seconds",
                            elapsed_seconds
                        )),
                    )
                    .await;

                    if let Some(lid) = ssh_check_log {
                        let duration = ssh_check_start.elapsed().as_millis() as i32;
                        if is_retry {
                            // Instance is still starting - this is a normal retry, not a failure
                            eprintln!(
                                "⏳ [process_create] {}",
                                retry_msg.as_ref().unwrap_or(
                                    &"Instance is starting - retrying SSH check".to_string()
                                )
                            );
                            logger::log_event_complete_with_metadata(
                                &pool,
                                lid,
                                "retry",
                                duration,
                                retry_msg.as_deref(),
                                Some(json!({
                                    "ip_address": ip_for_ssh,
                                    "elapsed_seconds": elapsed_seconds,
                                    "max_attempts": max_attempts
                                })),
                            )
                            .await
                            .ok();
                        } else {
                            // Real failure - SSH timeout after instance should be ready
                            eprintln!("⚠️ [process_create] Instance may not be booting correctly. Check Scaleway console for instance state and logs.");
                            logger::log_event_complete_with_metadata(
                                &pool,
                                lid,
                                "failed",
                                duration,
                                Some(&format!(
                                    "SSH not accessible after {} seconds",
                                    elapsed_seconds
                                )),
                                Some(json!({
                                    "ip_address": ip_for_ssh,
                                    "elapsed_seconds": elapsed_seconds,
                                    "max_attempts": max_attempts
                                })),
                            )
                            .await
                            .ok();
                        }
                    }

                    // Mark instance as failed if SSH is not accessible after 3 minutes
                    // This prevents indefinite waiting
                    let _ = sqlx::query(
                        "UPDATE instances
                         SET status = 'startup_failed'::instance_status,
                             error_code = COALESCE(error_code, 'SSH_NOT_ACCESSIBLE'),
                             error_message = COALESCE($2, error_message),
                             failed_at = COALESCE(failed_at, NOW())
                         WHERE id = $1",
                    )
                    .bind(instance_uuid)
                    .bind(&format!(
                        "SSH not accessible after {} seconds on {}",
                        elapsed_seconds, ip_for_ssh
                    ))
                    .execute(&pool)
                    .await;

                    // Don't return - let the function complete normally so cleanup can happen
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
                       status = 'booting',
                       boot_started_at = NOW(),
                       last_health_check = NULL,
                       health_check_failures = 0,
                       failed_at = NULL,
                       error_code = NULL,
                       error_message = NULL
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

/// Resolve vLLM Docker image with hierarchy:
/// 1. instance_types.allocation_params.vllm_image (instance-type specific)
/// 2. provider_settings.WORKER_VLLM_IMAGE_<INSTANCE_TYPE_CODE> (per instance type)
/// 3. provider_settings.WORKER_VLLM_IMAGE (provider default)
/// 4. WORKER_VLLM_IMAGE (env var)
/// 5. Hardcoded default (stable version, not "latest")
pub async fn resolve_vllm_image(
    pool: &Pool<Postgres>,
    instance_type_id: Option<Uuid>,
    provider_id: Option<Uuid>,
    instance_type_code: &str,
) -> String {
    // 1. Check instance_types.allocation_params.vllm_image (instance-type specific)
    if let Some(type_id) = instance_type_id {
        if let Ok(Some(vllm_image)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT NULLIF(TRIM(allocation_params->>'vllm_image'), '') FROM instance_types WHERE id = $1"
        )
        .bind(type_id)
        .fetch_optional(pool)
        .await
        {
            if let Some(img) = vllm_image {
                if !img.trim().is_empty() {
                    eprintln!("✅ [resolve_vllm_image] Using instance-type specific image: {} (from allocation_params)", img);
                    return img;
                }
            }
        }
    }

    // 2. Check provider_settings.WORKER_VLLM_IMAGE_<INSTANCE_TYPE_CODE> (per instance type)
    if let Some(pid) = provider_id {
        let setting_key = format!(
            "WORKER_VLLM_IMAGE_{}",
            instance_type_code.replace("-", "_").to_uppercase()
        );
        if let Ok(Some(img)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT NULLIF(TRIM(value_text), '') FROM provider_settings WHERE provider_id = $1 AND key = $2"
        )
        .bind(pid)
        .bind(&setting_key)
        .fetch_optional(pool)
        .await
        {
            if let Some(img) = img {
                if !img.trim().is_empty() {
                    eprintln!("✅ [resolve_vllm_image] Using provider setting for {}: {}", instance_type_code, img);
                    return img;
                }
            }
        }
    }

    // 3. Check provider_settings.WORKER_VLLM_IMAGE (provider default)
    if let Some(pid) = provider_id {
        if let Ok(Some(img)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT NULLIF(TRIM(value_text), '') FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_IMAGE'"
        )
        .bind(pid)
        .fetch_optional(pool)
        .await
        {
            if let Some(img) = img {
                if !img.trim().is_empty() {
                    eprintln!("✅ [resolve_vllm_image] Using provider default image: {}", img);
                    return img;
                }
            }
        }
    }

    // 4. Check environment variable
    if let Ok(img) = std::env::var("WORKER_VLLM_IMAGE") {
        if !img.trim().is_empty() {
            eprintln!(
                "✅ [resolve_vllm_image] Using env var WORKER_VLLM_IMAGE: {}",
                img
            );
            return img;
        }
    }

    // 5. Hardcoded default (stable version, not "latest")
    // Default: v0.13.0 is a stable version available on Docker Hub
    // Note: For P100 (RENDER-S), this may need to be a version compiled with sm_60 support
    // For L4/L40S, this version should work fine
    let default_image = "vllm/vllm-openai:v0.13.0".to_string();
    eprintln!("ℹ️ [resolve_vllm_image] Using hardcoded default: {} (consider configuring instance_types.allocation_params.vllm_image)", default_image);
    default_image
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
    if let Ok(provider) = ProviderManager::get_provider(&provider_name, pool.clone()).await {
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
        println!("❌ [Catalog Sync] Provider not configured (missing credentials).");
    }
}

pub async fn process_full_reconciliation(pool: Pool<Postgres>) {
    println!("🔄 [Full Reconciliation] Starting...");
    let provider_name = ProviderManager::current_provider_name();
    if let Ok(provider) = ProviderManager::get_provider(&provider_name, pool.clone()).await {
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
        println!("❌ [Full Reconciliation] Provider not configured (missing credentials).");
    }
}
