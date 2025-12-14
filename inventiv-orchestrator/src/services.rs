
use sqlx::{Pool, Postgres};
use uuid::Uuid;
use std::time::{Instant, Duration};
use tokio::time::sleep;
use serde_json::json;
use crate::provider_manager::ProviderManager;
use crate::logger;
use bigdecimal::FromPrimitive;

pub async fn process_termination(pool: Pool<Postgres>, instance_id: String, correlation_id: Option<String>) {
    let start = Instant::now();
    let id_uuid = match Uuid::parse_str(&instance_id) {
        Ok(v) => v,
        Err(e) => {
            println!("❌ Invalid instance_id for termination '{}': {:?}", instance_id, e);
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
    ).await.ok();

    // 1. Get instance details from DB
    let row_result = sqlx::query_as::<_, (Option<String>, Option<String>, String)>(
        "SELECT i.provider_instance_id, z.code as zone, i.status::text
         FROM instances i
         LEFT JOIN zones z ON i.zone_id = z.id
         WHERE i.id = $1"
    )
    .bind(id_uuid)
    .fetch_optional(&pool)
    .await;

    match row_result {
        Ok(Some((provider_id_opt, zone_opt, current_status))) => {
            let zone = zone_opt.unwrap_or_default();
            println!("🔍 Found instance {} (Zone: {}) Status: {}", 
                     provider_id_opt.as_deref().unwrap_or("None"), zone, current_status);
            
            // 2. Try to terminate on Provider
            if let Some(provider_instance_id) = provider_id_opt {
                if let Some(provider) = ProviderManager::get_provider("scaleway") {
                    
                    // LOG 2: PROVIDER_TERMINATE (API call to provider)
                    let api_start = Instant::now();
                    let log_id_provider = logger::log_event_with_metadata(
                        &pool, 
                        "PROVIDER_TERMINATE", 
                        "in_progress", 
                        id_uuid, 
                        None,
                        Some(json!({"zone": zone, "provider_instance_id": provider_instance_id, "correlation_id": correlation_id_meta})),
                    ).await.ok();
                    
                    let result = provider.terminate_instance(&zone, &provider_instance_id).await;
                    
                    match &result {
                        Ok(true) => {
                            println!("✅ Successfully terminated instance on Provider");
                            
                            if let Some(log_id) = log_id_provider {
                                let api_duration = api_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(&pool, log_id, "success", api_duration, None).await.ok();
                            }
                        }
                        Ok(false) => {
                            let err_msg = "Provider termination call returned non-success status";
                            println!("⚠️ {}", err_msg);

                            if let Some(log_id) = log_id_provider {
                                let api_duration = api_start.elapsed().as_millis() as i32;
                                logger::log_event_complete(&pool, log_id, "failed", api_duration, Some(err_msg)).await.ok();
                            }
                            if let Some(log_id) = log_id_execute {
                                let duration = start.elapsed().as_millis() as i32;
                                logger::log_event_complete(&pool, log_id, "failed", duration, Some(err_msg)).await.ok();
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
                                    logger::log_event_complete(&pool, log_id, "success", api_duration, Some("Instance already deleted")).await.ok();
                                }
                            } else {
                                println!("⚠️ Error terminating on Provider: {:?}", e);
                                if let Some(log_id) = log_id_provider {
                                    let api_duration = api_start.elapsed().as_millis() as i32;
                                    logger::log_event_complete(&pool, log_id, "failed", api_duration, Some(&err_msg)).await.ok();
                                }
                                // Don't proceed to mark as terminated if provider call failed
                                if let Some(log_id) = log_id_execute {
                                    let duration = start.elapsed().as_millis() as i32;
                                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&err_msg)).await.ok();
                                }
                                return;
                            }
                        }
                    }

                    // 2.5 Verify deletion (avoid marking terminated while still running)
                    // Scaleway termination is async; we poll for a short, bounded period.
                    let verify_start = Instant::now();
                    let mut deleted = false;
                    while verify_start.elapsed() < Duration::from_secs(60) {
                        match provider.check_instance_exists(&zone, &provider_instance_id).await {
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
                            logger::log_event_complete(&pool, log_id, "success", duration, None).await.ok();
                        }

                        if let Some(log_id) = log_id_execute {
                            let duration = start.elapsed().as_millis() as i32;
                            logger::log_event_complete(&pool, log_id, "success", duration, Some("Termination in progress (not yet deleted on provider)")).await.ok();
                        }
                        return;
                    }
                } else {
                    println!("⚠️ Provider configuration missing or provider not found");
                    if let Some(log_id) = log_id_execute {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "failed", duration, Some("Provider not configured")).await.ok();
                    }
                    return;
                }
            } else {
                println!("ℹ️ No provider_instance_id found, skipping Provider API call (just updating DB)");
            }

            // LOG 3: INSTANCE_TERMINATED (update DB)
            let db_start = Instant::now();
            let log_id_terminated = logger::log_event(
                &pool,
                "INSTANCE_TERMINATED",
                "in_progress",
                id_uuid,
                None,
            ).await.ok();
            
            // 3. Update DB status to terminated
            let update_result = sqlx::query(
                "UPDATE instances SET status = 'terminated', terminated_at = NOW() WHERE id = $1"
            )
            .bind(id_uuid)
            .execute(&pool)
            .await;

            match update_result {
                Ok(_) => {
                    println!("✅ Instance {} marked as terminated in DB", id_uuid);
                    
                    if let Some(log_id) = log_id_terminated {
                        let duration = db_start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "success", duration, None).await.ok();
                    }
                    
                    if let Some(log_id) = log_id_execute {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "success", duration, None).await.ok();
                    }
                }
                Err(e) => {
                    println!("❌ Failed to update instance status in DB: {:?}", e);
                    let msg = format!("DB update failed: {:?}", e);
                    
                    if let Some(log_id) = log_id_terminated {
                        let duration = db_start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg)).await.ok();
                    }
                    
                    if let Some(log_id) = log_id_execute {
                        let duration = start.elapsed().as_millis() as i32;
                        logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg)).await.ok();
                    }
                }
            }
        },
        Ok(None) => {
            println!("⚠️ Instance {} not found in DB for termination.", id_uuid);
            if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "failed", duration, Some("Instance not found")).await.ok();
            }
        },
        Err(e) => {
            println!("❌ Database Error during termination fetch: {:?}", e);
            if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                let msg = format!("DB error: {:?}", e);
                logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg)).await.ok();
            }
        },
    }
}

pub async fn process_provisioning(
    pool: Pool<Postgres>,
    instance_id: String,
    zone: String,
    instance_type: String,
    correlation_id: Option<String>,
) {
    let start = Instant::now();
    let instance_uuid = match Uuid::parse_str(&instance_id) {
        Ok(v) => v,
        Err(e) => {
            println!("❌ Invalid instance_id for provisioning '{}': {:?}", instance_id, e);
            return;
        }
    };
    let correlation_id_meta = correlation_id.clone();
    println!("🔨 [Orchestrator] Processing Provision for instance: {}", instance_uuid);
    
    // 0. Resolve Catalog IDs dynamically
    let provider_name = "scaleway"; // TODO: Dynamic?
    let provider_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(); // Scaleway ID

    let zone_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM zones WHERE code = $1 AND is_active = true")
        .bind(&zone)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

    let type_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM instance_types WHERE code = $1 AND provider_id = $2 AND is_active = true")
        .bind(&instance_type)
        .bind(provider_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None);

    if zone_id.is_none() || type_id.is_none() {
        println!("❌ Error: Zone '{}' or Type '{}' not found in catalog.", zone, instance_type);
        let _msg = format!("Catalog lookup failed: Zone={} Type={}", zone, instance_type);
         sqlx::query("UPDATE instances SET status = 'failed' WHERE id = $1")
             .bind(instance_uuid).execute(&pool).await.ok();
         // TODO: Log failure
        return;
    }

    // 0.5. Ensure row exists (idempotent; do NOT regress status on retries)
    let insert_result = sqlx::query(
         "INSERT INTO instances (id, provider_id, zone_id, instance_type_id, status, created_at, gpu_profile)
          VALUES ($1, $2, $3, $4, 'provisioning', NOW(), '{}')
          ON CONFLICT (id) DO NOTHING"
    )
    .bind(instance_uuid)
    .bind(provider_id)
    .bind(zone_id.unwrap())
    .bind(type_id.unwrap())
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
    ).await.ok();
    
    // 1. Init Provider
    let provider_opt = ProviderManager::get_provider(provider_name);
    
    if provider_opt.is_none() {
         let msg = "Missing Provider Credentials";
         println!("❌ Error: {}", msg);
         if let Some(log_id) = log_id_execute {
            let duration = start.elapsed().as_millis() as i32;
            logger::log_event_complete(&pool, log_id, "failed", duration, Some(msg)).await.ok();
         }
         sqlx::query("UPDATE instances SET status = 'failed' WHERE id = $1")
             .bind(instance_uuid).execute(&pool).await.ok();
         return;
    }
    let provider = provider_opt.unwrap();

    // 1.5 Idempotence guard: if provider_instance_id already exists, don't create a second server
    let existing: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT provider_instance_id, status::text FROM instances WHERE id = $1"
    )
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
                    if attempt < 5 { sleep(Duration::from_secs(2)).await; }
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
             WHERE id = $2"
        )
        .bind(ip_address)
        .bind(instance_uuid)
        .execute(&pool)
        .await;

        if let Some(log_id) = log_id_execute {
            let duration = start.elapsed().as_millis() as i32;
            logger::log_event_complete(&pool, log_id, "success", duration, Some("Idempotent retry: provider server already exists")).await.ok();
        }
        return;
    }
    
    // 2. Create Server
    let image_id = "8e0da557-5d75-40ba-b928-5984075aa255"; 
    
    // LOG 3: PROVIDER_CREATE (API call)
    let api_start = Instant::now();
    let log_id_provider = logger::log_event_with_metadata(
        &pool, "PROVIDER_CREATE", "in_progress", instance_uuid, None,
        Some(json!({"zone": zone, "instance_type": instance_type, "correlation_id": correlation_id_meta})),
    ).await.ok();
    
    let server_id_result = provider.create_instance(&zone, &instance_type, image_id).await;

    match server_id_result {
        Ok(server_id) => {
             println!("✅ Server Created: {}", server_id);
             
             if let Some(log_id) = log_id_provider {
                let api_duration = api_start.elapsed().as_millis() as i32;
                let metadata = json!({"server_id": server_id, "zone": zone, "correlation_id": correlation_id_meta});
                logger::log_event_complete_with_metadata(&pool, log_id, "success", api_duration, None, Some(metadata)).await.ok();
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
                 WHERE id = $2"
            )
            .bind(&server_id)
            .bind(instance_uuid)
            .execute(&pool)
            .await;

            if let Some(lid) = log_id_persist {
                let dur = persist_start.elapsed().as_millis() as i32;
                match &persist_res {
                    Ok(_) => logger::log_event_complete(&pool, lid, "success", dur, None).await.ok(),
                    Err(e) => logger::log_event_complete(&pool, lid, "failed", dur, Some(&format!("DB persist failed: {:?}", e))).await.ok(),
                };
            }
            if let Err(e) = persist_res {
                // If we can't persist server_id, better fail fast to avoid an untraceable leak.
                let msg = format!("Failed to persist provider_instance_id after create: {:?}", e);
                if let Some(log_id) = log_id_execute {
                    let duration = start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg)).await.ok();
                }
                return;
            }
             
            // LOG 3.1: PROVIDER_START (API call)
            let start_api = Instant::now();
            let log_id_start = logger::log_event_with_metadata(
                &pool,
                "PROVIDER_START",
                "in_progress",
                instance_uuid,
                None,
                Some(json!({"zone": zone, "server_id": server_id, "correlation_id": correlation_id_meta})),
            ).await.ok();

            // 3. Power On (fail-fast if provider rejects)
            if let Err(e) = provider.start_instance(&zone, &server_id).await {
                let msg = format!("Failed to start instance on provider: {:?}", e);
                println!("❌ {}", msg);
                if let Some(lid) = log_id_start {
                    let duration = start_api.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, lid, "failed", duration, Some(&msg)).await.ok();
                }
                if let Some(log_id) = log_id_execute {
                    let duration = start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg)).await.ok();
                }
                let _ = sqlx::query("UPDATE instances SET status = 'provisioning_failed' WHERE id = $1")
                    .bind(instance_uuid)
                    .execute(&pool)
                    .await;
                return;
            } else if let Some(lid) = log_id_start {
                let duration = start_api.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, lid, "success", duration, None).await.ok();
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
                         if attempt < 5 { sleep(Duration::from_secs(2)).await; }
                     }
                     Err(_) => break,
                 }
             }
             if let Some(lid) = log_id_ip {
                let duration = ip_api.elapsed().as_millis() as i32;
                let meta = json!({"ip_address": ip_address, "zone": zone, "server_id": server_id, "correlation_id": correlation_id_meta});
                if ip_address.is_some() {
                    logger::log_event_complete_with_metadata(&pool, lid, "success", duration, None, Some(meta)).await.ok();
                } else {
                    logger::log_event_complete_with_metadata(&pool, lid, "failed", duration, Some("IP not available after retries"), Some(meta)).await.ok();
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
                   WHERE id = $3 AND status NOT IN ('terminating', 'terminated')"
             )
             .bind(&server_id).bind(ip_address).bind(instance_uuid)
             .execute(&pool).await;
             
             if let Err(e) = update_result {
                 let msg = format!("DB update failed: {:?}", e);
                 if let Some(log_id) = log_id_created {
                    let duration = db_start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg)).await.ok();
                 }
                  if let Some(log_id) = log_id_execute {
                    let duration = start.elapsed().as_millis() as i32;
                    logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg)).await.ok();
                 }
                  return;
              }

              if let Some(log_id) = log_id_created {
                 let duration = db_start.elapsed().as_millis() as i32;
                 logger::log_event_complete(&pool, log_id, "success", duration, None).await.ok();
              }
            
            // Complete LOG 2
            if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "success", duration, None).await.ok();
            }
        }
        Err(e) => {
             let msg = format!("Failed to create instance: {:?}", e);
             if let Some(log_id) = log_id_provider {
                let api_duration = api_start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "failed", api_duration, Some(&msg)).await.ok();
             }
             if let Some(log_id) = log_id_execute {
                let duration = start.elapsed().as_millis() as i32;
                logger::log_event_complete(&pool, log_id, "failed", duration, Some(&msg)).await.ok();
             }
             sqlx::query("UPDATE instances SET status = 'failed' WHERE id = $1")
                 .bind(instance_uuid).execute(&pool).await.ok();
        }
    }
}

pub async fn process_catalog_sync(pool: Pool<Postgres>) {
    println!("🔄 [Catalog Sync] Starting catalog synchronization...");

    // 1. Get Provider (Scaleway)
    if let Some(provider) = ProviderManager::get_provider("scaleway") {
        let provider_uuid = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap_or_default();
        let zones = vec!["fr-par-1", "fr-par-2", "nl-ams-1", "pl-waw-1"];
        // TODO: Could fetch zones from DB: SELECT code FROM zones WHERE is_active=true

        for zone in zones {
             println!("🔄 [Catalog Sync] Fetching catalog for zone: {}", zone);
             match provider.fetch_catalog(zone).await {
                 Ok(items) => {
                     let mut count = 0;
                     for item in items {
                         // Convert f64 to BigDecimal for NUMERIC column
                         // Using primitive cast via string to avoid precision issues if possible or just use FromPrimitive
                         // sqlx BigDecimal feature allows direct usage usually if From f64 is implemented.
                         // But safer to cast in SQL or use bigdecimal crate types.
                         let hourly_price = bigdecimal::BigDecimal::from_f64(item.cost_per_hour).unwrap_or_default();

                        let _ = sqlx::query(
                            "INSERT INTO instance_types (id, provider_id, name, code, is_active, cost_per_hour, cpu_count, ram_gb, n_gpu, vram_per_gpu_gb, bandwidth_bps)
                             VALUES (gen_random_uuid(), $1, $2, $3, true, $4, $5, $6, $7, $8, $9)
                             ON CONFLICT (provider_id, code)
                             DO UPDATE SET
                                cost_per_hour = EXCLUDED.cost_per_hour,
                                cpu_count = EXCLUDED.cpu_count,
                                ram_gb = EXCLUDED.ram_gb,
                                n_gpu = EXCLUDED.n_gpu,
                                vram_per_gpu_gb = EXCLUDED.vram_per_gpu_gb,
                                bandwidth_bps = EXCLUDED.bandwidth_bps,
                                is_active = true"
                        )
                        .bind(provider_uuid)
                        .bind(&item.name)
                        .bind(&item.code)
                        .bind(hourly_price)
                        .bind(item.cpu_count)
                        .bind(item.ram_gb)
                        .bind(item.n_gpu)
                        .bind(item.vram_per_gpu_gb)
                        .bind(item.bandwidth_bps)
                        .execute(&pool)
                        .await;
                        count += 1;
                     }
                     println!("✅ [Catalog Sync] Updated {} types for zone {}", count, zone);
                 },
                 Err(e) => println!("❌ [Catalog Sync] Error for {}: {:?}", zone, e),
             }
        }
    } else {
        println!("❌ [Catalog Sync] Provider Scaleway not configured.");
    }
}

pub async fn process_full_reconciliation(pool: Pool<Postgres>) {
    println!("🔄 [Full Reconciliation] Starting...");
    if let Some(provider) = ProviderManager::get_provider("scaleway") {
        let zones = vec!["fr-par-2"]; // TODO: Fetch from DB / Config

        for zone in zones {
             match provider.list_instances(zone).await {
                 Ok(instances) => {
                     println!("🔍 [Full Reconciliation] List returned {} instances in {}", instances.len(), zone);
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
                             println!("🔍 [Full Reconciliation] Found orphan: {} ({}) Status: {}", inst.name, inst.provider_id, inst.status);
                             
                             // Resolve Zone ID
                             let zone_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM zones WHERE code = $1 OR name = $1 LIMIT 1")
                                 .bind(zone)
                                 .fetch_optional(&pool)
                                 .await
                                 .unwrap_or(None);
                                 
                            if let Some(zid) = zone_id {
                                 let new_id = Uuid::new_v4();
                                 let provider_uuid = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap_or_default(); // Scaleway
                                 let type_id = Uuid::parse_str("00000000-0000-0000-0000-000000000030").unwrap_or_default(); // Default RENDER-S

                                 // Map Status (Simplistic)
                                 let status = match inst.status.as_str() {
                                     "running" | "starting" => "ready",
                                     "stopped" => "failed", 
                                     _ => "provisioning"
                                 };

                                 let insert_res = sqlx::query(
                                     "INSERT INTO instances 
                                     (id, provider_id, zone_id, instance_type_id, status, provider_instance_id, ip_address, created_at, gpu_profile)
                                     VALUES ($1, $2, $3, $4, $5::instance_status, $6, $7::inet, NOW(), '{}')"
                                 )
                                 .bind(new_id)
                                 .bind(provider_uuid)
                                 .bind(zid)
                                 .bind(type_id)
                                 .bind(status)
                                 .bind(&inst.provider_id)
                                 .bind(inst.ip_address)
                                 .execute(&pool)
                                 .await;

                                 if let Err(e) = insert_res {
                                     println!("❌ [Full Reconciliation] Failed to import orphan {}: {:?}", inst.provider_id, e);
                                 } else {
                                     println!("✅ [Full Reconciliation] Imported orphan {} => {}", inst.provider_id, new_id);
                                     import_count += 1;
                                 }
                            } else {
                                println!("⚠️ [Full Reconciliation] Unknown zone '{}' for orphan {}", zone, inst.provider_id);
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
                                if (db_status == "terminated" || db_status == "archived") && (inst.status == "running" || inst.status == "starting") {
                                     println!("⚠️ [Full Reconciliation] ZOMBIE DETECTED: {} is {} on Cloud but {} in DB. Reactivating...", inst.provider_id, inst.status, db_status);
                                     
                                     let _ = sqlx::query(
                                         "UPDATE instances SET status = 'ready', terminated_at = NULL, is_archived = false WHERE provider_instance_id = $1"
                                     )
                                     .bind(&inst.provider_id)
                                     .execute(&pool)
                                     .await;
                                     println!("✅ [Full Reconciliation] Zombie {} reactivated in DB.", inst.provider_id);
                                }
                            }
                         }
                     }
                     if import_count > 0 {
                        println!("✅ [Full Reconciliation] Imported {} orphaned instances in {}", import_count, zone);
                     }
                 },
                 Err(e) => println!("❌ [Full Reconciliation] Failed to list instances in {}: {:?}", zone, e),
             }
        }
        println!("✅ [Full Reconciliation] Completed.");
    } else {
        println!("❌ [Full Reconciliation] Provider Scaleway not configured.");
    }
}
