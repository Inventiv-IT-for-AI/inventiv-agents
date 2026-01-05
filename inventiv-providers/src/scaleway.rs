use crate::{inventory, CloudProvider};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

pub struct ScalewayProvider {
    client: Client,
    project_id: String,
    secret_key: String,
    ssh_public_key: Option<String>,
}

impl ScalewayProvider {
    pub fn new(project_id: String, secret_key: String, ssh_public_key: Option<String>) -> Self {
        // Default reqwest client has no overall timeout. If Scaleway stalls, a job can hang forever.
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap();
        let project_id = project_id.trim().to_string();
        let secret_key = secret_key.trim().to_string();
        let ssh_public_key = ssh_public_key
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        Self {
            client,
            project_id,
            secret_key,
            ssh_public_key,
        }
    }

    // Scaleway-specific instance type helpers
    fn requires_diskless_boot_image(instance_type: &str) -> bool {
        let t = instance_type.trim().to_ascii_uppercase();
        // L4 and L40S require diskless boot images (no local volumes)
        // RENDER-S also requires proper GPU-enabled images (with NVIDIA drivers)
        t.starts_with("L4-") || t.starts_with("L40S-") || t.starts_with("RENDER-")
    }

    fn is_render_s_instance(instance_type: &str) -> bool {
        instance_type.trim().to_ascii_uppercase().starts_with("RENDER-")
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "X-Auth-Token",
            reqwest::header::HeaderValue::from_str(&self.secret_key).unwrap(),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers
    }
}

#[async_trait]
impl CloudProvider for ScalewayProvider {
    async fn create_instance(
        &self,
        zone: &str,
        instance_type: &str,
        image_id: &str,
        cloud_init: Option<&str>,
        volumes: Option<&[String]>, // Optional list of Block Storage volume IDs to attach at creation
    ) -> Result<String> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers",
            zone
        );
        let name = format!("inventiv-worker-{}", Uuid::new_v4());
        
        // Check if this instance type requires diskless boot (L4, L40S, RENDER-S)
        let instance_type_upper = instance_type.to_uppercase();
        let requires_diskless_boot = instance_type_upper.starts_with("L4-")
            || instance_type_upper.starts_with("L40S-")
            || instance_type_upper.starts_with("RENDER-");
        
        let mut body = json!({
            "name": name,
            "commercial_type": instance_type,
            "project": self.project_id,
            "tags": ["inventiv-agents", "worker"],
            "dynamic_ip_required": true
        });
        body["image"] = json!(image_id);
        
        // For diskless boot instances (L4, L40S, H100), we need to:
        // 1. Do NOT include "volumes" field at all - Scaleway will automatically create a Block Storage bootable volume (20GB)
        // 2. Set boot_type to "local"
        // 3. Use the validated GPU image (5c3d28db-33ce-4997-8572-f49506339283) which has the correct root_volume configuration
        // According to Scaleway: L4/L40S/H100 require total local-volume size = 0GB
        // RENDER-S is different: it gets auto-created NVMe storage from the image
        // IMPORTANT: NOT including "volumes" field tells Scaleway to automatically create a Block Storage bootable volume (20GB)
        // This auto-created Block Storage will be resized to the target size BEFORE starting the instance
        if requires_diskless_boot {
            body["boot_type"] = json!("local");
            // Do NOT include "volumes" field - Scaleway will auto-create Block Storage bootable volume (20GB)
            // The auto-created Block Storage will be discovered and resized BEFORE instance startup
            eprintln!(
                "🔵 [Scaleway API] Instance type {} requires diskless boot - creating WITHOUT volumes field. Scaleway will auto-create Block Storage bootable volume (20GB).",
                instance_type
            );
            } else {
            // For non-diskless instances, include Block Storage volumes if provided
            if let Some(vols) = volumes {
                if !vols.is_empty() {
                    // Format: volumes as object with numeric keys: {"0": {"id": "volume-id"}, "1": {"id": "volume-id2"}}
                    let mut volumes_obj = serde_json::Map::new();
                    for (idx, vol_id) in vols.iter().enumerate() {
                        volumes_obj.insert(
                            idx.to_string(),
                            json!({"id": vol_id})
                        );
                    }
                    body["volumes"] = json!(volumes_obj);
                    eprintln!(
                        "🔵 [Scaleway API] Attaching {} Block Storage volume(s) at instance creation",
                        vols.len()
                    );
                }
            }
        }

        // Scaleway automatically applies SSH keys from the project to all instances.
        // No need to pass user_data/cloud-init - SSH keys are configured automatically.
        // Log request details
        eprintln!(
            "🔵 [Scaleway API] POST {} - Creating instance: type={}, image={}, zone={}",
            url, instance_type, image_id, zone
        );
        eprintln!("🔵 [Scaleway API] Request payload: {}", serde_json::to_string_pretty(&body).unwrap_or_default());

        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

            let status = resp.status();
        let status_code = status.as_u16();

        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            eprintln!(
                "❌ [Scaleway API] POST {} failed: status={}, response={}",
                url, status_code, text
            );
                return Err(anyhow::anyhow!(
                    "Scaleway create_instance failed: status={} body={}",
                    status_code,
                    text
                ));
        } else {
            eprintln!("✅ [Scaleway API] POST {} succeeded: status={}", url, status_code);
        }

        let json_resp: serde_json::Value = resp.json().await?;
        let server_id = json_resp["server"]["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No server id in create response"))?
            .to_string();
        
        // Log response details (truncate large payloads)
        let response_summary = serde_json::to_string(&json_resp["server"])
            .unwrap_or_default();
        let response_preview = if response_summary.len() > 500 {
            format!("{}... (truncated)", &response_summary[..500])
        } else {
            response_summary
        };
        
        // Verify boot_type in response for diskless instances
        let boot_type = json_resp["server"]["boot_type"].as_str().unwrap_or("unknown");
        eprintln!(
            "✅ [Scaleway API] Server created: id={}, zone={}, state={}, boot_type={}",
            server_id,
            zone,
            json_resp["server"]["state"].as_str().unwrap_or("unknown"),
            boot_type
        );
        
        // Critical check: if diskless boot was requested but instance was created with local volumes,
        // this will fail on startup. For L40S instances, boot_type="local" is correct, but we need
        // to verify that NO local volumes were created (volumes array should be empty or contain only Block Storage volumes).
        if requires_diskless_boot {
            let volumes = json_resp["server"]["volumes"].as_array();
            let volumes_debug = serde_json::to_string(&json_resp["server"]["volumes"]).unwrap_or_default();
            eprintln!("🔍 [Scaleway API] Instance {} volumes array: {}", server_id, volumes_debug);
            
            let has_local_volumes = volumes.map_or(false, |vols| {
                vols.iter().any(|v| {
                    // Check if volume is local (volume_type="l_ssd" indicates local volume)
                    // For L40S instances, ANY volume with volume_type="l_ssd" is problematic
                    let vol_type = v.get("volume_type").and_then(|t| t.as_str()).unwrap_or("");
                    let vol_id = v.get("id").and_then(|id| id.as_str()).unwrap_or("");
                    let size = v.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                    
                    eprintln!("🔍 [Scaleway API] Volume check: id={}, type={}, size={}GB", vol_id, vol_type, size);
                    
                    // l_ssd = local SSD volume (not allowed for L40S diskless boot)
                    vol_type == "l_ssd"
                })
            });
            
            if has_local_volumes {
                // For L4/L40S/H100, Scaleway GPU image ALWAYS creates a local volume.
                // This is expected. We will detach and replace it with Block Storage before starting.
                eprintln!(
                    "⚠️ [Scaleway API] Instance {} has auto-created local volumes (expected for GPU image). \
                    Will detach and replace with Block Storage before startup.",
                    server_id
                );
            } else {
                eprintln!(
                    "✅ [Scaleway API] Instance {} created successfully with diskless boot (boot_type={}, no local volumes)",
                    server_id, boot_type
                );
            }
        }
        
        eprintln!("🔵 [Scaleway API] Response preview: {}", response_preview);
        
        Ok(server_id)
    }

    /// Phase 1: Remove local volumes from diskless instance (BEFORE startup).
    /// This must be called AFTER instance creation but BEFORE starting the instance.
    /// Returns true if local volumes were found and removed, false if none found.
    async fn remove_local_volumes(
        &self,
        zone: &str,
        server_id: &str,
        instance_type: &str,
        pre_created_volume_id: Option<&str>,
    ) -> Result<bool> {
        eprintln!(
            "🔵 [Scaleway Diskless Phase 1] Removing local volumes from instance {} (type={})",
            server_id, instance_type
        );

        // Get current server state and volumes
        let server_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        let resp = self.client.get(&server_url).headers(self.headers()).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get server details for local volume removal"));
        }
        
        let server_json: serde_json::Value = resp.json().await?;
        let server_state = server_json["server"]["state"].as_str().unwrap_or("unknown");
        let volumes = server_json["server"]["volumes"].as_object();
        
        // Verify instance is stopped (required for volume manipulation)
        if server_state != "stopped" && server_state != "stopped_in_place" {
            return Err(anyhow::anyhow!(
                "Instance {} must be stopped before removing local volumes (current state: {})",
                server_id, server_state
            ));
        }
        
        // Collect local volume IDs to delete AND check for existing Block Storage
        let mut local_volume_ids: Vec<String> = Vec::new();
        let mut has_block_storage = false;
        if let Some(vols) = volumes {
            for (_slot, vol) in vols {
                let vol_type = vol.get("volume_type").and_then(|t| t.as_str()).unwrap_or("");
                let vol_id = vol.get("id").and_then(|id| id.as_str()).unwrap_or("");
                
                if vol_type == "l_ssd" && !vol_id.is_empty() {
                    eprintln!(
                        "🔍 [Scaleway Diskless Phase 1] Found local volume: {} (will be removed)",
                        vol_id
                    );
                    local_volume_ids.push(vol_id.to_string());
                } else if vol_type == "sbs_volume" && !vol_id.is_empty() {
                    has_block_storage = true;
                    eprintln!(
                        "🔍 [Scaleway Diskless Phase 1] Found existing Block Storage volume: {} (will be preserved)",
                        vol_id
                    );
                }
            }
        }

        // CRITICAL: If we have local volumes to remove AND no Block Storage attached,
        // we MUST attach Block Storage BEFORE removing local volumes.
        // Scaleway refuses to start an instance with no volumes attached.
        if !local_volume_ids.is_empty() && !has_block_storage {
            if let Some(pre_vol_id) = pre_created_volume_id {
                eprintln!(
                    "🔵 [Scaleway Diskless Phase 1] Attaching Block Storage {} BEFORE removing local volumes (Scaleway requires at least one volume)",
                    pre_vol_id
                );
                
                // Attach Block Storage via CLI before removing local volumes
                let org_id = std::env::var("SCALEWAY_ORGANIZATION_ID")
                    .or_else(|_| {
                        server_json["server"]["organization"]
                            .as_str()
                            .map(|s| s.to_string())
                            .ok_or(std::env::VarError::NotPresent)
                    })
                    .unwrap_or_default();
                
                let access_key = std::env::var("SCALEWAY_ACCESS_KEY").unwrap_or_default();
                
                let cli_output = std::process::Command::new("scw")
                    .env("SCW_ACCESS_KEY", &access_key)
                    .env("SCW_SECRET_KEY", &self.secret_key)
                    .env("SCW_DEFAULT_PROJECT_ID", &self.project_id)
                    .env("SCW_DEFAULT_ORGANIZATION_ID", &org_id)
                    .arg("instance")
                    .arg("server")
                    .arg("update")
                    .arg(server_id)
                    .arg(format!("zone={}", zone))
                    .arg(format!("volume-ids.0={}", pre_vol_id))
                    .output();
                
                match cli_output {
                    Ok(output) => {
                        if output.status.success() {
                            eprintln!("✅ [Scaleway Diskless Phase 1] Successfully attached Block Storage via CLI");
                            has_block_storage = true;
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            eprintln!("❌ [Scaleway Diskless Phase 1] CLI attachment failed: {}", stderr);
                            return Err(anyhow::anyhow!("Failed to attach Block Storage via CLI before removing local volumes: {}", stderr));
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ [Scaleway Diskless Phase 1] CLI execution failed: {}", e);
                        return Err(anyhow::anyhow!("Failed to execute scw CLI: {}", e));
                    }
                }
                
                // Wait a bit for attachment to propagate
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            } else {
                return Err(anyhow::anyhow!(
                    "Cannot remove local volumes from instance {}: no Block Storage attached and no pre_created_volume_id provided. Scaleway requires at least one volume to be attached.",
                    server_id
                ));
            }
        }

        // If no local volumes found, we're done
        if local_volume_ids.is_empty() {
            eprintln!("✅ [Scaleway Diskless Phase 1] No local volumes found on instance {}", server_id);
            return Ok(false);
        }
        
        // Detach local volumes (Block Storage is now attached, so we can safely remove local volumes)
        eprintln!(
            "🔵 [Scaleway Diskless Phase 1] Detaching {} local volume(s) from instance {} (Block Storage is attached)",
            local_volume_ids.len(), server_id
        );
        
        let patch_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        // Build volumes object with only Block Storage volumes (preserve them)
        let mut volumes_to_keep = serde_json::Map::new();
        if let Some(vols) = server_json["server"]["volumes"].as_object() {
            for (slot, vol) in vols {
                let vol_type = vol.get("volume_type").and_then(|t| t.as_str()).unwrap_or("");
                let vol_id = vol.get("id").and_then(|id| id.as_str()).unwrap_or("");
                
                // Keep only Block Storage volumes (skip local volumes)
                if vol_type == "sbs_volume" && !vol_id.is_empty() {
                    volumes_to_keep.insert(slot.clone(), json!({"id": vol_id}));
                }
            }
        }
        
        // If we attached Block Storage via CLI, we need to refresh the server state
        if has_block_storage && pre_created_volume_id.is_some() {
            let refresh_resp = self.client.get(&server_url).headers(self.headers()).send().await?;
            if refresh_resp.status().is_success() {
                if let Ok(refresh_json) = refresh_resp.json::<serde_json::Value>().await {
                    if let Some(refresh_vols) = refresh_json["server"]["volumes"].as_object() {
                        volumes_to_keep.clear();
                        for (slot, vol) in refresh_vols {
                            let vol_type = vol.get("volume_type").and_then(|t| t.as_str()).unwrap_or("");
                            let vol_id = vol.get("id").and_then(|id| id.as_str()).unwrap_or("");
                            
                            if vol_type == "sbs_volume" && !vol_id.is_empty() {
                                volumes_to_keep.insert(slot.clone(), json!({"id": vol_id}));
                            }
                        }
                    }
                }
            }
        }
        
        // Ensure we have at least one Block Storage volume
        if volumes_to_keep.is_empty() {
            return Err(anyhow::anyhow!(
                "Cannot remove local volumes: no Block Storage volume found after attachment. Instance must have at least one volume attached."
            ));
        }
        
        let patch_body = json!({"volumes": volumes_to_keep});
        
        let patch_resp = self.client
            .patch(&patch_url)
            .headers(self.headers())
            .json(&patch_body)
            .send()
            .await?;
        
        if !patch_resp.status().is_success() {
            let error_text = patch_resp.text().await.unwrap_or_default();
            eprintln!("❌ [Scaleway Diskless Phase 1] Failed to detach local volumes: {}", error_text);
            return Err(anyhow::anyhow!("Failed to detach local volumes: {}", error_text));
        }
        
        eprintln!("✅ [Scaleway Diskless Phase 1] Successfully detached local volumes (Block Storage preserved)");

        // Delete the local volumes
        for vol_id in &local_volume_ids {
            eprintln!("🔵 [Scaleway Diskless Phase 1] Deleting local volume {}", vol_id);
            
            let delete_url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/volumes/{}",
                zone, vol_id
            );
            
            let delete_resp = self.client
                .delete(&delete_url)
                .headers(self.headers())
                .send()
                .await?;
            
            if delete_resp.status().is_success() || delete_resp.status().as_u16() == 204 {
                eprintln!("✅ [Scaleway Diskless Phase 1] Deleted local volume {}", vol_id);
            } else {
                let error_text = delete_resp.text().await.unwrap_or_default();
                eprintln!("⚠️ [Scaleway Diskless Phase 1] Failed to delete local volume {}: {}", vol_id, error_text);
            }
        }

        eprintln!("✅ [Scaleway Diskless Phase 1] Instance {} is ready for diskless boot (no local volumes, Block Storage attached)", server_id);
        Ok(true)
    }

    /// Phase 2: Attach Block Storage to instance (AFTER startup and SSH accessible).
    /// This must be called AFTER the instance has started and SSH is accessible.
    /// Returns the ID of the attached Block Storage volume.
    async fn attach_block_storage_after_boot(
        &self,
        zone: &str,
        server_id: &str,
        instance_type: &str,
        data_volume_size_gb: u64,
        pre_created_volume_id: Option<&str>,
    ) -> Result<String> {
        eprintln!(
            "🔵 [Scaleway Diskless Phase 2] Attaching Block Storage to instance {} (type={})",
            server_id, instance_type
        );

        // Get current server state and volumes
        let server_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        let resp = self.client.get(&server_url).headers(self.headers()).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get server details for Block Storage attachment"));
        }
        
        let server_json: serde_json::Value = resp.json().await?;
        let server_state = server_json["server"]["state"].as_str().unwrap_or("unknown");
        let volumes = server_json["server"]["volumes"].as_object();
        
        // Verify instance is running
        if server_state != "running" {
            return Err(anyhow::anyhow!(
                "Instance {} must be running before attaching Block Storage (current state: {})",
                server_id, server_state
            ));
        }
        
        // Check for existing Block Storage volumes
        let mut existing_sbs_volume_ids: Vec<String> = Vec::new();
        if let Some(vols) = volumes {
            for (slot, vol) in vols {
                let vol_type = vol.get("volume_type").and_then(|t| t.as_str()).unwrap_or("");
                let vol_id = vol.get("id").and_then(|id| id.as_str()).unwrap_or("");
                
                if vol_type == "sbs_volume" && !vol_id.is_empty() {
                    eprintln!(
                        "🔍 [Scaleway Diskless Phase 2] Found existing Block Storage volume in slot {}: {}",
                        slot, vol_id
                    );
                    existing_sbs_volume_ids.push(vol_id.to_string());
                }
            }
        }

        // If Block Storage is already attached, return its ID
        if !existing_sbs_volume_ids.is_empty() {
            let sbs_id = existing_sbs_volume_ids[0].clone();
            eprintln!(
                "✅ [Scaleway Diskless Phase 2] Instance {} already has Block Storage {} attached",
                server_id, sbs_id
            );
            return Ok(sbs_id);
        }

        // Use pre-created Block Storage volume if provided, otherwise create a new one
        let sbs_id = if let Some(pre_vol_id) = pre_created_volume_id {
            eprintln!(
                "✅ [Scaleway Diskless Phase 2] Using pre-created Block Storage volume: id={}",
                pre_vol_id
            );
            pre_vol_id.to_string()
        } else {
            // Create a new Block Storage (SBS) volume
            let size_bytes = data_volume_size_gb * 1_000_000_000;
            let vol_name = format!("inventiv-data-{}", server_id);
            
            eprintln!(
                "🔵 [Scaleway Diskless Phase 2] Creating Block Storage volume: name={}, size={}GB",
                vol_name, data_volume_size_gb
            );
            
            let create_vol_url = format!(
                "https://api.scaleway.com/block/v1/zones/{}/volumes",
                zone
            );
            
            let create_body = json!({
                "name": vol_name,
                "project": self.project_id,
                "volume_type": "sbs_volume",
                "size": size_bytes
            });
            
            let create_resp = self.client
                .post(&create_vol_url)
                .headers(self.headers())
                .json(&create_body)
                .send()
                .await?;
            
            if !create_resp.status().is_success() {
                let error_text = create_resp.text().await.unwrap_or_default();
                eprintln!("❌ [Scaleway Diskless Phase 2] Failed to create Block Storage: {}", error_text);
                return Err(anyhow::anyhow!("Failed to create Block Storage: {}", error_text));
            }
            
            let create_json: serde_json::Value = create_resp.json().await?;
            let new_vol_id = create_json["id"].as_str().ok_or_else(|| {
                anyhow::anyhow!("Block Storage creation response missing 'id' field")
            })?;
            
            eprintln!("✅ [Scaleway Diskless Phase 2] Created Block Storage volume: id={}", new_vol_id);
            
            // Wait for volume to be available
            let sbs_status_url = format!(
                "https://api.scaleway.com/block/v1/zones/{}/volumes/{}",
                zone, new_vol_id
            );
            
            eprintln!("⏳ [Scaleway Diskless Phase 2] Waiting for Block Storage {} to be available...", new_vol_id);
            
            for attempt in 1..=30 {
                let status_resp = self.client
                    .get(&sbs_status_url)
                    .headers(self.headers())
                    .send()
                    .await?;
                
                if status_resp.status().is_success() {
                    if let Ok(json) = status_resp.json::<serde_json::Value>().await {
                        let status = json["status"].as_str().unwrap_or("");
                        if status == "available" {
                            eprintln!("✅ [Scaleway Diskless Phase 2] Block Storage {} is available", new_vol_id);
                            break;
                        }
                        if attempt % 5 == 0 {
                            eprintln!(
                                "🔵 [Scaleway Diskless Phase 2] Block Storage status: {} (attempt {}/30)",
                                status, attempt
                            );
                        }
                    }
                }
                
                if attempt == 30 {
                    eprintln!("⚠️ [Scaleway Diskless Phase 2] Block Storage not available after 30 attempts, proceeding anyway");
                }
                
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
            
            new_vol_id.to_string()
        };

        // Attach SBS volume via CLI
        eprintln!(
            "🔵 [Scaleway Diskless Phase 2] Attaching Block Storage {} to instance {} via CLI",
            sbs_id, server_id
        );
        
        // Get organization ID for CLI
        let org_id = std::env::var("SCALEWAY_ORGANIZATION_ID")
            .or_else(|_| {
                server_json["server"]["organization"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or(std::env::VarError::NotPresent)
            })
            .unwrap_or_default();
        
        // Get access key from environment (required by scw CLI)
        let access_key = std::env::var("SCALEWAY_ACCESS_KEY").unwrap_or_default();
        
        let cli_output = std::process::Command::new("scw")
            .env("SCW_ACCESS_KEY", &access_key)
            .env("SCW_SECRET_KEY", &self.secret_key)
            .env("SCW_DEFAULT_PROJECT_ID", &self.project_id)
            .env("SCW_DEFAULT_ORGANIZATION_ID", &org_id)
            .arg("instance")
            .arg("server")
            .arg("update")
            .arg(server_id)
            .arg(format!("zone={}", zone))
            .arg(format!("volume-ids.0={}", sbs_id))
            .output();
        
        match cli_output {
            Ok(output) => {
                if output.status.success() {
                    eprintln!("✅ [Scaleway Diskless Phase 2] Successfully attached Block Storage via CLI");
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!("❌ [Scaleway Diskless Phase 2] CLI attachment failed: {}", stderr);
                    return Err(anyhow::anyhow!("Failed to attach Block Storage via CLI: {}", stderr));
                }
            }
            Err(e) => {
                eprintln!("❌ [Scaleway Diskless Phase 2] CLI execution failed: {}", e);
                return Err(anyhow::anyhow!("Failed to execute scw CLI: {}", e));
            }
        }

        // Verify attachment
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        
        let verify_resp = self.client
            .get(&server_url)
            .headers(self.headers())
            .send()
            .await?;
        
        if verify_resp.status().is_success() {
            let verify_json: serde_json::Value = verify_resp.json().await?;
            let volumes = verify_json["server"]["volumes"].as_object();
            
            if let Some(vols) = volumes {
                let has_sbs = vols.values().any(|v| {
                    v.get("id").and_then(|id| id.as_str()) == Some(&sbs_id)
                });
                
                if has_sbs {
                    eprintln!("✅ [Scaleway Diskless Phase 2] Verified Block Storage {} is attached", sbs_id);
                } else {
                    eprintln!("⚠️ [Scaleway Diskless Phase 2] Block Storage attachment not yet visible, proceeding anyway");
                }
            }
        }

        eprintln!(
            "✅ [Scaleway Diskless Phase 2] Instance {} has Block Storage {} attached",
            server_id, sbs_id
        );
        
        Ok(sbs_id)
    }

    /// Prepares a diskless boot instance for startup (DEPRECATED - use remove_local_volumes + attach_block_storage_after_boot).
    /// For backward compatibility, this method calls remove_local_volumes and attach_block_storage_after_boot in sequence.
    /// 
    /// This is required for L4/L40S/H100 instances because the GPU image creates
    /// a local volume automatically, but these instance types require 0GB of local storage.
    async fn prepare_diskless_instance(
        &self,
        zone: &str,
        server_id: &str,
        instance_type: &str,
        data_volume_size_gb: u64,
        pre_created_volume_id: Option<&str>,
    ) -> Result<String> {
        eprintln!(
            "🔵 [Scaleway Diskless] Preparing instance {} for diskless boot (type={}) - DEPRECATED: use remove_local_volumes + attach_block_storage_after_boot",
            server_id, instance_type
        );

        // Get current server state
        let server_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        let resp = self.client.get(&server_url).headers(self.headers()).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get server details for diskless prep"));
        }
        
        let server_json: serde_json::Value = resp.json().await?;
        let server_state = server_json["server"]["state"].as_str().unwrap_or("unknown");
        
        // If instance is running, stop it before Phase 1
        let was_running = server_state == "running";
        if was_running {
            eprintln!(
                "🔵 [Scaleway Diskless] Instance {} is running - stopping it before Phase 1",
                server_id
            );
            
            let stop_url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action",
                zone, server_id
            );
            
            let stop_resp = self.client
                .post(&stop_url)
                .headers(self.headers())
                .json(&json!({"action": "poweroff"}))
                .send()
                .await?;
            
            if stop_resp.status().is_success() {
                // Wait for instance to stop
                eprintln!("⏳ [Scaleway Diskless] Waiting for instance {} to stop...", server_id);
                for attempt in 1..=30 {
                    sleep(Duration::from_secs(2)).await;
                    let state_resp = self.client.get(&server_url).headers(self.headers()).send().await?;
                    if state_resp.status().is_success() {
                        if let Ok(state_json) = state_resp.json::<serde_json::Value>().await {
                            let state = state_json["server"]["state"].as_str().unwrap_or("unknown");
                            if state == "stopped" || state == "stopped_in_place" {
                                eprintln!("✅ [Scaleway Diskless] Instance {} is now stopped", server_id);
                                break;
                            }
                        }
                    }
                    if attempt == 30 {
                        eprintln!("⚠️ [Scaleway Diskless] Instance {} did not stop after 60s, continuing anyway", server_id);
                    }
                }
            }
        }

        // Phase 1: Remove local volumes (pass pre_created_volume_id so Block Storage can be attached first)
        self.remove_local_volumes(zone, server_id, instance_type, pre_created_volume_id).await?;

        // If instance was running, restart it before Phase 2
        if was_running {
            eprintln!("🔵 [Scaleway Diskless] Restarting instance {} before Phase 2", server_id);
            self.start_instance(zone, server_id).await?;
            
            // Wait for instance to be running
            eprintln!("⏳ [Scaleway Diskless] Waiting for instance {} to start...", server_id);
            for attempt in 1..=30 {
                sleep(Duration::from_secs(2)).await;
                let state_resp = self.client.get(&server_url).headers(self.headers()).send().await?;
                if state_resp.status().is_success() {
                    if let Ok(state_json) = state_resp.json::<serde_json::Value>().await {
                        let state = state_json["server"]["state"].as_str().unwrap_or("unknown");
                        if state == "running" {
                            eprintln!("✅ [Scaleway Diskless] Instance {} is now running", server_id);
                            break;
                        }
                    }
                }
                if attempt == 30 {
                    eprintln!("⚠️ [Scaleway Diskless] Instance {} did not start after 60s, continuing anyway", server_id);
                }
            }
        }

        // Phase 2: Attach Block Storage
        self.attach_block_storage_after_boot(
            zone,
            server_id,
            instance_type,
            data_volume_size_gb,
            pre_created_volume_id,
        ).await
    }

    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action",
            zone, server_id
        );
        let body = json!({ "action": "poweron" });
        
        eprintln!(
            "🔵 [Scaleway API] POST {} - Starting server: server_id={}, zone={}",
            url, server_id, zone
        );
        eprintln!("🔵 [Scaleway API] Request payload: {}", serde_json::to_string_pretty(&body).unwrap_or_default());
        
        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;
        
        let status = resp.status();
        let status_code = status.as_u16();
        
        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            eprintln!(
                "❌ [Scaleway API] POST {} failed: status={}, response={}",
                url, status_code, error_text
            );
            return Err(anyhow::anyhow!(
                "Scaleway poweron failed: status={} body={}",
                status_code,
                error_text
            ));
        }
        
        eprintln!("✅ [Scaleway API] POST {} succeeded: status={}", url, status_code);
        Ok(true)
    }

    async fn stop_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        // Check current state first
        let current_state = self.get_server_state(zone, server_id).await?;
        if let Some(state) = current_state {
            let state_lower = state.to_ascii_lowercase();
            if state_lower == "stopped" || state_lower == "stopped_in_place" {
                eprintln!(
                    "ℹ️ [Scaleway API] Server {} already stopped (state: {})",
                    server_id, state
                );
                return Ok(true);
            }
        }
        
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action",
            zone, server_id
        );
        let body = json!({ "action": "poweroff" });
        
        eprintln!(
            "🔵 [Scaleway API] POST {} - Stopping server: server_id={}, zone={}",
            url, server_id, zone
        );
        eprintln!("🔵 [Scaleway API] Request payload: {}", serde_json::to_string_pretty(&body).unwrap_or_default());
        
        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;
        
        let status = resp.status();
        let status_code = status.as_u16();
        
        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            eprintln!(
                "❌ [Scaleway API] POST {} failed: status={}, response={}",
                url, status_code, error_text
            );
            return Err(anyhow::anyhow!(
                "Scaleway poweroff failed: status={} body={}",
                status_code,
                error_text
            ));
        }
        
        eprintln!("✅ [Scaleway API] POST {} succeeded: status={}", url, status_code);
        
        // Wait for server to reach stopped state (up to 60 seconds)
        for _ in 0..30 {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            if let Ok(Some(state)) = self.get_server_state(zone, server_id).await {
                let state_lower = state.to_ascii_lowercase();
                if state_lower == "stopped" || state_lower == "stopped_in_place" {
                    eprintln!(
                        "✅ [Scaleway API] Server {} stopped successfully (state: {})",
                        server_id, state
                    );
                    return Ok(true);
                }
            }
        }
        
        eprintln!(
            "⚠️ [Scaleway API] Server {} poweroff command sent but state not confirmed as stopped after 60s",
            server_id
        );
        // Still return Ok(true) as the command was accepted
        Ok(true)
    }

    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        // Scaleway requires instances to be powered off before deletion
        // Stop the instance first if it's running and WAIT for it to be completely stopped
        let current_state = self.get_server_state(zone, server_id).await?;
        if let Some(state) = current_state {
            let state_lower = state.to_ascii_lowercase();
            if state_lower != "stopped" && state_lower != "stopped_in_place" {
                eprintln!(
                    "🔵 [Scaleway API] Instance {} is {} - stopping before termination",
                    server_id, state
                );
                // stop_instance already waits for the instance to be stopped (up to 60s)
                match self.stop_instance(zone, server_id).await {
                    Ok(true) => {
                        eprintln!("✅ [Scaleway API] Instance {} stopped successfully, proceeding with deletion", server_id);
                    }
                    Ok(false) => {
                        eprintln!("⚠️ [Scaleway API] Instance {} stop command returned false, verifying state before proceeding", server_id);
                        // Verify state one more time before giving up
                        let final_state = self.get_server_state(zone, server_id).await?;
                        if let Some(fs) = final_state {
                            let fs_lower = fs.to_ascii_lowercase();
                            if fs_lower != "stopped" && fs_lower != "stopped_in_place" {
                                return Err(anyhow::anyhow!(
                                    "Cannot terminate instance {}: failed to stop (current state: {})",
                                    server_id, fs
                                ));
                            }
                            eprintln!("✅ [Scaleway API] Instance {} is stopped (verified), proceeding with deletion", server_id);
                        } else {
                            return Err(anyhow::anyhow!(
                                "Cannot terminate instance {}: failed to stop and cannot verify state",
                                server_id
                            ));
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️ [Scaleway API] Failed to stop instance {} before termination: {}",
                            server_id, e
                        );
                        // Verify state one more time before giving up
                        let final_state = self.get_server_state(zone, server_id).await?;
                        if let Some(fs) = final_state {
                            let fs_lower = fs.to_ascii_lowercase();
                            if fs_lower != "stopped" && fs_lower != "stopped_in_place" {
                                return Err(anyhow::anyhow!(
                                    "Cannot terminate instance {}: failed to stop (current state: {})",
                                    server_id, fs
                                ));
                            }
                            eprintln!("✅ [Scaleway API] Instance {} is stopped (verified), proceeding with deletion", server_id);
                        } else {
                            return Err(anyhow::anyhow!(
                                "Cannot terminate instance {}: failed to stop and cannot verify state",
                                server_id
                            ));
                        }
                    }
                }
            } else {
                eprintln!("ℹ️ [Scaleway API] Instance {} is already stopped (state: {}), proceeding with deletion", server_id, state);
            }
        } else {
            eprintln!("⚠️ [Scaleway API] Cannot determine state of instance {}, attempting deletion anyway", server_id);
        }
        
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        eprintln!(
            "🔵 [Scaleway API] DELETE {} - Terminating server: server_id={}, zone={}",
            url, server_id, zone
        );
        
        let resp = self
            .client
            .delete(&url)
            .headers(self.headers())
            .send()
            .await?;
        
        let status = resp.status();
        let status_code = status.as_u16();
        
        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            eprintln!(
                "❌ [Scaleway API] DELETE {} failed: status={}, response={}",
                url, status_code, error_text
            );
            
            // If error is about instance needing to be powered off, try stopping again and wait longer
            if error_text.contains("powered off") || error_text.contains("resource_still_in_use") {
                eprintln!(
                    "🔄 [Scaleway API] Instance still not stopped, retrying stop before termination for server {}",
                    server_id
                );
                // Wait longer and verify state before retry
                if let Ok(_) = self.stop_instance(zone, server_id).await {
                    // Wait a bit more and verify state
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    let verify_state = self.get_server_state(zone, server_id).await?;
                    if let Some(vs) = verify_state {
                        let vs_lower = vs.to_ascii_lowercase();
                        if vs_lower != "stopped" && vs_lower != "stopped_in_place" {
                            return Err(anyhow::anyhow!(
                                "Cannot terminate instance {}: still not stopped after retry (state: {})",
                                server_id, vs
                            ));
                        }
                    }
                    
                    // Retry deletion after stopping
                    eprintln!("🔄 [Scaleway API] Retrying DELETE {} after stop", url);
                    let resp2 = self
                        .client
                        .delete(&url)
                        .headers(self.headers())
                        .send()
                        .await?;
                    
                    let status2 = resp2.status();
                    let status_code2 = status2.as_u16();
                    
                    if status2.is_success() {
                        eprintln!("✅ [Scaleway API] DELETE {} succeeded after retry: status={}", url, status_code2);
                        return Ok(true);
                    } else {
                        let error_text2 = resp2.text().await.unwrap_or_default();
                        eprintln!(
                            "❌ [Scaleway API] DELETE {} retry failed: status={}, response={}",
                            url, status_code2, error_text2
                        );
                        return Err(anyhow::anyhow!(
                            "Scaleway terminate failed (retry): status={} body={}",
                            status_code2,
                            error_text2
                        ));
                    }
                } else {
                    return Err(anyhow::anyhow!(
                        "Scaleway terminate failed: cannot stop instance {} before deletion",
                        server_id
                    ));
                }
            }
            
            return Err(anyhow::anyhow!(
                "Scaleway terminate failed: status={} body={}",
                status_code,
                error_text
            ));
        }
        
        eprintln!("✅ [Scaleway API] DELETE {} succeeded: status={}", url, status_code);
        Ok(true)
    }

    async fn get_server_state(&self, zone: &str, server_id: &str) -> Result<Option<String>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        eprintln!(
            "🔵 [Scaleway API] GET {} - Getting server state: server_id={}, zone={}",
            url, server_id, zone
        );
        
        let resp = self.client.get(&url).headers(self.headers()).send().await?;
        let status = resp.status();
        let status_code = status.as_u16();
        
        if !status.is_success() {
            eprintln!(
                "⚠️ [Scaleway API] GET {} failed: status={}",
                url, status_code
            );
            return Ok(None);
        }
        
        let json_resp: serde_json::Value = resp.json().await?;
        let state = json_resp["server"]["state"]
            .as_str()
            .map(|s| s.to_string());
        
        if let Some(ref state_str) = state {
            eprintln!(
                "✅ [Scaleway API] GET {} succeeded: status={}, server_state={}",
                url, status_code, state_str
            );
        } else {
            eprintln!(
                "⚠️ [Scaleway API] GET {} succeeded but no state in response",
                url
            );
        }
        
        Ok(state)
    }

    async fn list_attached_volumes(
        &self,
        zone: &str,
        server_id: &str,
    ) -> Result<Vec<inventory::AttachedVolume>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        eprintln!(
            "🔵 [Scaleway API] GET {} - Listing attached volumes: server_id={}, zone={}",
            url, server_id, zone
        );
        
        let resp = self.client.get(&url).headers(self.headers()).send().await?;
        let status = resp.status();
        let status_code = status.as_u16();
        
        if !status.is_success() {
            eprintln!(
                "⚠️ [Scaleway API] GET {} failed: status={}",
                url, status_code
            );
            return Ok(vec![]);
        }
        
        let json_resp: serde_json::Value = resp.json().await?;
        let server = json_resp.get("server").and_then(|s| s.as_object());
        
        // Debug: log the full server response to understand structure
        if let Some(server_obj) = server {
            let volumes_debug = server_obj.get("volumes").map(|v| serde_json::to_string_pretty(v).unwrap_or_default());
            eprintln!("🔍 [Scaleway API] Server volumes structure for {}: {}", server_id, volumes_debug.as_deref().unwrap_or("null"));
            
            // Also check if there are other volume-related fields
            let all_keys: Vec<String> = server_obj.keys().map(|k| k.to_string()).collect();
            eprintln!("🔍 [Scaleway API] Server object keys: {:?}", all_keys);
        }
        
        let mut volumes = Vec::new();
        
        if let Some(server_obj) = server {
            // Scaleway returns volumes in server.volumes array
            // For RENDER-S, volumes might be in a different format (object with numeric keys like "0", "1")
            let volumes_value = server_obj.get("volumes");
            let volumes_to_iterate: Vec<&serde_json::Value> = if let Some(v) = volumes_value {
                // Try as array first
                if let Some(arr) = v.as_array() {
                    arr.iter().collect()
                } else if let Some(obj) = v.as_object() {
                    // If it's an object (e.g., {"0": {...}, "1": {...}}), convert values to Vec
                    eprintln!("🔍 [Scaleway API] Volumes is an object with {} keys, converting to array", obj.len());
                    obj.values().collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };
            
            if !volumes_to_iterate.is_empty() {
                for vol in volumes_to_iterate {
                    if let Some(vol_obj) = vol.as_object() {
                        let volume_id = vol_obj
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let volume_name = vol_obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let volume_type = vol_obj
                            .get("volume_type")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        let size = vol_obj
                            .get("size")
                            .and_then(|v| v.as_i64());
                        let boot = vol_obj
                            .get("boot")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        
                        if let Some(id) = volume_id {
                            volumes.push(inventory::AttachedVolume {
                                provider_volume_id: id,
                                provider_volume_name: volume_name,
                                volume_type,
                                size_bytes: size,
                                boot,
                            });
                        }
                    }
                }
            }
        }
        
        eprintln!(
            "✅ [Scaleway API] Found {} attached volume(s) for server {}",
            volumes.len(), server_id
        );
        
        Ok(volumes)
    }

    async fn delete_volume(&self, zone: &str, volume_id: &str) -> Result<bool> {
        // Scaleway volumes can be deleted via the Block Storage API
        // First, try to delete via instance API (for local volumes)
        // If that fails, try Block Storage API (for SBS volumes)
        
        // Try Instance API first (for local volumes like l_ssd)
        let instance_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/volumes/{}",
            zone, volume_id
        );
        
        eprintln!(
            "🔵 [Scaleway API] DELETE {} - Deleting volume: volume_id={}, zone={}",
            instance_url, volume_id, zone
        );
        
        let resp = self
            .client
            .delete(&instance_url)
            .headers(self.headers())
            .send()
            .await?;
        
        let status = resp.status();
        let status_code = status.as_u16();
        
        if status.is_success() {
            eprintln!("✅ [Scaleway API] DELETE {} succeeded: status={}", instance_url, status_code);
            return Ok(true);
        }
        
        // If Instance API fails, try Block Storage API (for SBS volumes)
        let block_url = format!(
            "https://api.scaleway.com/block/v1/zones/{}/volumes/{}",
            zone, volume_id
        );
        
        eprintln!(
            "🔄 [Scaleway API] Instance API failed, trying Block Storage API: DELETE {}",
            block_url
        );
        
        let resp2 = self
            .client
            .delete(&block_url)
            .headers(self.headers())
            .send()
            .await?;
        
        let status2 = resp2.status();
        let status_code2 = status2.as_u16();
        
        if status2.is_success() {
            eprintln!("✅ [Scaleway API] DELETE {} succeeded: status={}", block_url, status_code2);
            return Ok(true);
        }
        
        // Read error message for logging
        let error_text = resp2.text().await.unwrap_or_default();
        eprintln!(
            "❌ [Scaleway API] DELETE {} failed: status={}, response={}",
            block_url, status_code2, error_text
        );
        
        // Don't fail if volume is already deleted or doesn't exist
        if error_text.contains("not found") || error_text.contains("does not exist") {
            eprintln!("ℹ️ [Scaleway API] Volume {} already deleted or doesn't exist", volume_id);
            return Ok(true);
        }
        
        // If volume is still attached to an instance, Scaleway will delete it when instance is deleted
        // Accept this as success to avoid blocking termination
        // Check for various error messages indicating volume is in use
        if error_text.contains("attached") 
            || error_text.contains("in use") 
            || error_text.contains("in_use") 
            || error_text.contains("in_use status")
            || error_text.contains("resource_still_in_use") 
            || error_text.contains("protected_resource")
            || (error_text.contains("precondition") && error_text.contains("protected")) {
            eprintln!("⚠️ [Scaleway API] Volume {} is still attached to an instance (status: in_use) - will be deleted when instance is deleted", volume_id);
            return Ok(true);
        }
        
        Err(anyhow::anyhow!(
            "Scaleway delete volume failed: status={} body={}",
            status_code2,
            error_text
        ))
    }

    async fn resize_block_storage(
        &self,
        zone: &str,
        volume_id: &str,
        new_size_gb: u64,
    ) -> Result<bool> {
        // Scaleway Block Storage can be resized via CLI only (API doesn't support resize)
        // This is used to enlarge volumes created automatically by Scaleway (e.g., 20GB → target size based on LLM model)
        
        eprintln!(
            "🔵 [Scaleway Block Storage] Resizing volume {} to {}GB via CLI (zone: {})",
            volume_id, new_size_gb, zone
        );
        
        // Get organization ID and access key from environment (required by scw CLI)
        let org_id = std::env::var("SCALEWAY_ORGANIZATION_ID").unwrap_or_default();
        let access_key = std::env::var("SCALEWAY_ACCESS_KEY").unwrap_or_default();
        
        if org_id.is_empty() || access_key.is_empty() {
            return Err(anyhow::anyhow!(
                "SCALEWAY_ORGANIZATION_ID and SCALEWAY_ACCESS_KEY are required for Block Storage resize via CLI"
            ));
        }
        
        let cli_output = std::process::Command::new("scw")
            .env("SCW_ACCESS_KEY", &access_key)
            .env("SCW_SECRET_KEY", &self.secret_key)
            .env("SCW_DEFAULT_PROJECT_ID", &self.project_id)
            .env("SCW_DEFAULT_ORGANIZATION_ID", &org_id)
            .arg("block")
            .arg("volume")
            .arg("update")
            .arg(volume_id)
            .arg(format!("zone={}", zone))
            .arg(format!("size={}GB", new_size_gb))
            .output();
        
        match cli_output {
            Ok(output) => {
                if output.status.success() {
                    eprintln!("✅ [Scaleway Block Storage] Successfully resized volume {} to {}GB", volume_id, new_size_gb);
                    
                    // Wait a bit for resize to propagate
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    
                    // Verify resize by checking volume size via API
                    let verify_url = format!(
                        "https://api.scaleway.com/block/v1/zones/{}/volumes/{}",
                        zone, volume_id
                    );
                    
                    // Verify resize with timeout (max 30 seconds: 6 attempts × 5 seconds)
                    let mut verified = false;
                    for attempt in 1..=6 {
                        match self.client
                            .get(&verify_url)
                            .headers(self.headers())
                            .send()
                            .await
                        {
                            Ok(verify_resp) => {
                                if verify_resp.status().is_success() {
                                    if let Ok(verify_json) = verify_resp.json::<serde_json::Value>().await {
                                        if let Some(size_bytes) = verify_json["size"].as_u64() {
                                            let size_gb = size_bytes / 1_000_000_000;
                                            if size_gb >= new_size_gb {
                                                eprintln!("✅ [Scaleway Block Storage] Verified resize: volume {} is now {}GB", volume_id, size_gb);
                                                verified = true;
                                                break;
                                            } else if attempt < 6 {
                                                eprintln!("⏳ [Scaleway Block Storage] Resize in progress: {}GB (target: {}GB), attempt {}/6", size_gb, new_size_gb, attempt);
                                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                            } else {
                                                eprintln!("⚠️ [Scaleway Block Storage] Resize may not be complete: {}GB (target: {}GB) - continuing anyway", size_gb, new_size_gb);
                                                // Continue anyway - resize was requested and CLI succeeded
                                                verified = true;
                                                break;
                                            }
                                        } else {
                                            eprintln!("⚠️ [Scaleway Block Storage] Could not parse size from API response (attempt {}/6)", attempt);
                                            if attempt < 6 {
                                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                            }
                                        }
                                    } else {
                                        eprintln!("⚠️ [Scaleway Block Storage] Could not parse JSON from API response (attempt {}/6)", attempt);
                                        if attempt < 6 {
                                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                        }
                                    }
                                } else {
                                    eprintln!("⚠️ [Scaleway Block Storage] API verification failed with status {} (attempt {}/6)", verify_resp.status(), attempt);
                                    if attempt < 6 {
                                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("⚠️ [Scaleway Block Storage] API verification request failed: {} (attempt {}/6)", e, attempt);
                                if attempt < 6 {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                }
                            }
                        }
                    }
                    
                    if verified {
                        Ok(true)
                    } else {
                        // CLI succeeded but verification failed - return success anyway since resize was requested
                        eprintln!("⚠️ [Scaleway Block Storage] Resize CLI succeeded but verification failed - continuing anyway");
                        Ok(true)
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    eprintln!("❌ [Scaleway Block Storage] CLI resize failed: stderr={}, stdout={}", stderr, stdout);
                    Err(anyhow::anyhow!("Failed to resize Block Storage via CLI: stderr={}, stdout={}", stderr, stdout))
                }
            }
            Err(e) => {
                eprintln!("❌ [Scaleway Block Storage] CLI execution failed: {}", e);
                Err(anyhow::anyhow!("Failed to execute scw CLI: {}", e))
            }
        }
    }

    async fn get_block_storage_size(
        &self,
        zone: &str,
        volume_id: &str,
    ) -> Result<Option<u64>> {
        let url = format!(
            "https://api.scaleway.com/block/v1/zones/{}/volumes/{}",
            zone, volume_id
        );
        
        eprintln!(
            "🔵 [Scaleway API] GET {} - Getting Block Storage volume size",
            url
        );
        
        let resp = self.client.get(&url).headers(self.headers()).send().await?;
        
        if !resp.status().is_success() {
            eprintln!("⚠️ [Scaleway API] Failed to get volume size: status={}", resp.status());
            return Ok(None);
        }
        
        if let Ok(volume_json) = resp.json::<serde_json::Value>().await {
            // Try different response structures
            let size_bytes = volume_json["size"].as_u64()
                .or_else(|| volume_json["volume"]["size"].as_u64())
                .or_else(|| volume_json["volumes"][0]["size"].as_u64());
            
            if let Some(size) = size_bytes {
                eprintln!("✅ [Scaleway API] Retrieved volume size: {}GB", size / 1_000_000_000);
                return Ok(Some(size));
            }
        }
        
        eprintln!("⚠️ [Scaleway API] Could not parse volume size from response");
        Ok(None)
    }

    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        eprintln!(
            "🔵 [Scaleway API] GET {} - Getting instance IP: server_id={}, zone={}",
            url, server_id, zone
        );
        
        let resp = self.client.get(&url).headers(self.headers()).send().await?;
        let status = resp.status();
        let status_code = status.as_u16();
        
        if !status.is_success() {
            eprintln!(
                "❌ [Scaleway API] GET {} failed: status={}",
                url, status_code
            );
            return Ok(None);
        }
        
        let json_resp: serde_json::Value = resp.json().await?;
        
        // Log server state for debugging
        let server_state = json_resp["server"]["state"].as_str();
        if let Some(state) = server_state {
            eprintln!(
                "🔍 [Scaleway API] Server {} state: {}",
                server_id, state
            );
        }
        
        // Extract IP address from public_ip.address
        // Scaleway assigns dynamic IPs only after the server reaches "running" state
        let ip = json_resp["server"]["public_ip"]["address"]
            .as_str()
            .filter(|s| !s.is_empty() && *s != "null")
            .map(|s| s.to_string());
        
        if let Some(ip_addr) = &ip {
            eprintln!(
                "✅ [Scaleway API] Server {} IP address: {}",
                server_id, ip_addr
            );
            return Ok(ip);
        }
        
        // If public_ip.address is null or empty, check if public_ip.id exists (flexible IP)
        let public_ip_id = json_resp["server"]["public_ip"]["id"].as_str();
        if let Some(ip_id) = public_ip_id {
            eprintln!(
                "ℹ️ [Scaleway API] Server {} has public_ip.id={} but address is null (IP may not be assigned yet, server state: {})",
                server_id, ip_id, server_state.unwrap_or("unknown")
            );
                } else {
            eprintln!(
                "ℹ️ [Scaleway API] Server {} has no public IP assigned yet (server state: {})",
                server_id, server_state.unwrap_or("unknown")
            );
        }
        
        Ok(None)
    }

    async fn check_instance_exists(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        let resp = self.client.get(&url).headers(self.headers()).send().await?;
        Ok(resp.status().is_success())
    }

    async fn fetch_catalog(&self, _zone: &str) -> Result<Vec<inventory::CatalogItem>> {
        // Existing catalog seeding happens elsewhere; keep best-effort empty here for now.
        Ok(vec![])
    }

    async fn list_instances(&self, zone: &str) -> Result<Vec<inventory::DiscoveredInstance>> {
        // Minimal listing: for now return empty (reconciliation uses provider APIs in orchestrator service code).
        // This keeps compilation and interface stable while we finish the provider extraction refactor.
        let _ = zone;
        Ok(vec![])
    }

    async fn resolve_boot_image(&self, zone: &str, instance_type: &str) -> Result<Option<String>> {
        // Detect if this is a GPU instance type
        let instance_type_upper = instance_type.to_uppercase();
        let is_gpu_instance = instance_type_upper.starts_with("RENDER-")
            || instance_type_upper.starts_with("L4-")
            || instance_type_upper.starts_with("L40S-")
            || instance_type_upper.contains("GPU");

        // For GPU instances, use ONLY the validated image (Ubuntu Noble GPU OS 13 passthrough)
        // Image ID: 5c3d28db-33ce-4997-8572-f49506339283
        // This validated image allows Scaleway to auto-create Block Storage bootable volume (20GB)
        if is_gpu_instance {
            eprintln!(
                "✅ Using validated GPU image for instance type {} (zone: {}): 5c3d28db-33ce-4997-8572-f49506339283 (Ubuntu Noble GPU OS 13 passthrough)",
                instance_type, zone
            );
            return Ok(Some("5c3d28db-33ce-4997-8572-f49506339283".to_string()));
        }

        // For non-GPU instances, return None (caller should use default image)
        eprintln!(
            "⚠️ Non-GPU instance type {} - no boot image resolution needed",
            instance_type
        );
        Ok(None)
    }

    async fn ensure_inbound_tcp_ports(
        &self,
        zone: &str,
        server_id: &str,
        ports: Vec<u16>,
    ) -> Result<bool> {
        eprintln!(
            "🔵 [Scaleway Security Group] Ensuring inbound TCP ports {:?} are open for instance {}",
            ports, server_id
        );

        // Step 1: Get server details to find security group
        let server_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        let resp = self.client.get(&server_url).headers(self.headers()).send().await?;
        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            eprintln!("❌ [Scaleway Security Group] Failed to get server details: {}", error_text);
            return Err(anyhow::anyhow!("Failed to get server details: {}", error_text));
        }
        
        let server_json: serde_json::Value = resp.json().await?;
        let security_group_id = server_json["server"]["security_group"]["id"]
            .as_str()
            .map(|s| s.to_string());
        
        let security_group_id = if let Some(sg_id) = security_group_id {
            eprintln!("🔍 [Scaleway Security Group] Instance has security group: {}", sg_id);
            sg_id
        } else {
            // No security group attached, create one
            let sg_name = format!("inventiv-worker-{}-sg", Uuid::new_v4());
            eprintln!("🔵 [Scaleway Security Group] Creating new security group: {}", sg_name);
            
            let create_sg_url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/security_groups",
                zone
            );
            
            let create_sg_body = json!({
                "name": sg_name,
                "project": self.project_id,
                "stateful": true,
                "inbound_default_policy": "drop",
                "outbound_default_policy": "accept",
                "tags": ["inventiv-agents", "worker"]
            });
            
            let create_resp = self.client
                .post(&create_sg_url)
                .headers(self.headers())
                .json(&create_sg_body)
                .send()
                .await?;
            
            if !create_resp.status().is_success() {
                let error_text = create_resp.text().await.unwrap_or_default();
                eprintln!("❌ [Scaleway Security Group] Failed to create security group: {}", error_text);
                return Err(anyhow::anyhow!("Failed to create security group: {}", error_text));
            }
            
            let create_json: serde_json::Value = create_resp.json().await?;
            let new_sg_id = create_json["security_group"]["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Security group creation response missing 'id' field"))?
                .to_string();
            
            eprintln!("✅ [Scaleway Security Group] Created security group: {}", new_sg_id);
            
            // Attach security group to server
            let attach_url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
                zone, server_id
            );
            
            let attach_body = json!({
                "security_group": {"id": new_sg_id}
            });
            
            let attach_resp = self.client
                .patch(&attach_url)
                .headers(self.headers())
                .json(&attach_body)
                .send()
                .await?;
            
            if !attach_resp.status().is_success() {
                let error_text = attach_resp.text().await.unwrap_or_default();
                eprintln!("⚠️ [Scaleway Security Group] Failed to attach security group to server: {}", error_text);
                // Continue anyway - rules can still be added
            } else {
                eprintln!("✅ [Scaleway Security Group] Attached security group to server");
            }
            
            new_sg_id
        };

        // Step 2: Get existing rules to avoid duplicates
        let rules_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/security_groups/{}/rules",
            zone, security_group_id
        );
        
        let rules_resp = self.client.get(&rules_url).headers(self.headers()).send().await?;
        let mut existing_ports: HashSet<u16> = HashSet::new();
        let mut existing_rules: Vec<serde_json::Value> = Vec::new();
        
        // Parse existing rules once and extract both ports and full rules
        // Check status before consuming response
        let rules_status = rules_resp.status();
        if rules_status.is_success() {
            if let Ok(rules_json) = rules_resp.json::<serde_json::Value>().await {
                if let Some(rules_array) = rules_json["rules"].as_array() {
                    for rule in rules_array {
                        if let Some(direction) = rule.get("direction").and_then(|d| d.as_str()) {
                            if direction == "inbound" {
                                if let Some(port) = rule.get("dest_port_from").and_then(|p| p.as_u64()) {
                                    existing_ports.insert(port as u16);
                                }
                                existing_rules.push(rule.clone());
                            }
                        }
                    }
                }
            }
        }

        // Step 3: Build rules for ports that don't already exist
        let mut rules_to_add = Vec::new();
        let mut position = 1;
        
        for port in &ports {
            if !existing_ports.contains(port) {
                rules_to_add.push(json!({
                    "action": "accept",
                    "protocol": "TCP",
                    "direction": "inbound",
                    "ip_range": "0.0.0.0/0",
                    "dest_port_from": port,
                    "dest_port_to": port,
                    "position": position,
                    "editable": true
                }));
                position += 1;
            } else {
                eprintln!("ℹ️ [Scaleway Security Group] Port {} already has a rule, skipping", port);
            }
        }

        if rules_to_add.is_empty() {
            eprintln!("✅ [Scaleway Security Group] All ports {:?} already have rules", ports);
            return Ok(true);
        }

        // Save length before moving rules_to_add
        let rules_to_add_count = rules_to_add.len();

        // Step 4: Add new rules (PUT replaces all rules, so we need to merge with existing)
        // Use existing_rules we already parsed above
        let mut all_rules = existing_rules;
        
        // Add new rules
        for rule in rules_to_add {
            all_rules.push(rule);
        }
        
        // Update positions
        for (idx, rule) in all_rules.iter_mut().enumerate() {
            if let Some(obj) = rule.as_object_mut() {
                obj.insert("position".to_string(), json!(idx + 1));
            }
        }
        
        let update_rules_body = json!({
            "rules": all_rules
        });
        
        eprintln!(
            "🔵 [Scaleway Security Group] Adding {} new rule(s) to security group {}",
            rules_to_add_count, security_group_id
        );
        
        let update_resp = self.client
            .put(&rules_url)
            .headers(self.headers())
            .json(&update_rules_body)
            .send()
            .await?;
        
        if !update_resp.status().is_success() {
            let error_text = update_resp.text().await.unwrap_or_default();
            eprintln!("❌ [Scaleway Security Group] Failed to update rules: {}", error_text);
            return Err(anyhow::anyhow!("Failed to update security group rules: {}", error_text));
        }
        
        eprintln!("✅ [Scaleway Security Group] Successfully added rules for ports {:?}", ports);
        Ok(true)
    }

    async fn set_cloud_init(&self, zone: &str, server_id: &str, cloud_init: &str) -> Result<bool> {
        // Scaleway supports setting user_data via PUT on /user_data/cloud-init endpoint.
        // IMPORTANT: Must use Content-Type: text/plain (not application/json) and send content directly.
        // This matches the working implementation in scripts/scw_instance_provision.sh
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/user_data/cloud-init",
            zone, server_id
        );
        
        eprintln!(
            "🔵 [Scaleway API] PUT {} - Setting cloud-init for instance: server_id={}, zone={}, cloud_init_length={}",
            url, server_id, zone, cloud_init.len()
        );
        
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "X-Auth-Token",
            reqwest::header::HeaderValue::from_str(&self.secret_key).unwrap(),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("text/plain"),
        );
        
        let resp = self
            .client
            .put(&url)
            .headers(headers)
            .body(cloud_init.to_string())
            .send()
            .await?;
        
        let status = resp.status();
        let status_code = status.as_u16();
        
        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            eprintln!(
                "❌ [Scaleway API] PUT {} failed: status={}, response={}",
                url, status_code, error_text
            );
            return Ok(false);
        }
        
        eprintln!(
            "✅ [Scaleway API] PUT {} succeeded: status={}",
            url, status_code
        );
        
        Ok(true)
    }

    async fn create_volume(
        &self,
        zone: &str,
        name: &str,
        size_bytes: i64,
        volume_type: &str,
        _perf_iops: Option<i32>,
    ) -> Result<Option<String>> {
        // Scaleway supports two types of volumes:
        // 1. Block Storage (sbs_volume) - via Block Storage API
        // 2. Local Storage (l_ssd) - via Instance API
        
        // Minimum size is 1GB = 1,000,000,000 bytes
        if size_bytes < 1_000_000_000 {
            return Err(anyhow::anyhow!("Volume size must be at least 1GB (1,000,000,000 bytes)"));
        }

        let size_gb_display = size_bytes / 1_000_000_000;

        if volume_type == "sbs_volume" {
            // Block Storage: use Block Storage API
            let url = format!(
                "https://api.scaleway.com/block/v1/zones/{}/volumes",
                zone
            );

            let body = json!({
                "name": name,
                "project_id": self.project_id,
                "from_empty": {
                    "size": size_bytes
                }
            });

            eprintln!(
                "🔵 [Scaleway API] POST {} - Creating Block Storage volume: name={}, size={}GB ({} bytes), zone={}",
                url, name, size_gb_display, size_bytes, zone
            );
            eprintln!("🔵 [Scaleway API] Request payload: {}", serde_json::to_string_pretty(&body).unwrap_or_default());

            let resp = self
                .client
                .post(&url)
                .headers(self.headers())
                .json(&body)
                .send()
                .await?;

            let status = resp.status();
            let status_code = status.as_u16();

            if !status.is_success() {
                let error_text = resp.text().await.unwrap_or_default();
                eprintln!(
                    "❌ [Scaleway API] POST {} failed: status={}, response={}",
                    url, status_code, error_text
                );
                return Err(anyhow::anyhow!(
                    "Scaleway create_volume (Block Storage) failed: status={} body={}",
                    status_code,
                    error_text
                ));
            }

            let json_resp: serde_json::Value = resp.json().await?;
            let volume_id = json_resp["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No volume id in create response"))?
                .to_string();

            eprintln!("✅ [Scaleway API] Block Storage volume created: id={}, name={}, size={}GB", volume_id, name, size_gb_display);
            Ok(Some(volume_id))
        } else if volume_type == "l_ssd" {
            // Local Storage: use Instance API
            let url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/volumes",
                zone
            );

            let body = json!({
                "name": name,
                "project": self.project_id,
                "volume_type": "l_ssd",
                "size": size_bytes
            });

            eprintln!(
                "🔵 [Scaleway API] POST {} - Creating Local Storage volume: name={}, size={}GB ({} bytes), zone={}",
                url, name, size_gb_display, size_bytes, zone
            );
            eprintln!("🔵 [Scaleway API] Request payload: {}", serde_json::to_string_pretty(&body).unwrap_or_default());

            let resp = self
                .client
                .post(&url)
                .headers(self.headers())
                .json(&body)
                .send()
                .await?;

            let status = resp.status();
            let status_code = status.as_u16();

            if !status.is_success() {
                let error_text = resp.text().await.unwrap_or_default();
                eprintln!(
                    "❌ [Scaleway API] POST {} failed: status={}, response={}",
                    url, status_code, error_text
                );
                return Err(anyhow::anyhow!(
                    "Scaleway create_volume (Local Storage) failed: status={} body={}",
                    status_code,
                    error_text
                ));
            }

            let json_resp: serde_json::Value = resp.json().await?;
            // Instance API returns {"volume": {"id": "..."}} or directly {"id": "..."}
            let volume_id = json_resp.get("volume")
                .and_then(|v| v.get("id"))
                .or_else(|| json_resp.get("id"))
                .and_then(|id| id.as_str())
                .ok_or_else(|| anyhow::anyhow!("No volume id in create response"))?
                .to_string();

            eprintln!("✅ [Scaleway API] Local Storage volume created: id={}, name={}, size={}GB", volume_id, name, size_gb_display);
            Ok(Some(volume_id))
        } else {
            eprintln!(
                "⚠️ [Scaleway API] Volume type '{}' not supported, only 'sbs_volume' (Block Storage) and 'l_ssd' (Local Storage) are supported",
                volume_type
            );
            Ok(None)
        }
    }

    async fn attach_volume(
        &self,
        zone: &str,
        server_id: &str,
        volume_id: &str,
        _delete_on_termination: bool,
    ) -> Result<bool> {
        // Scaleway Block Storage volumes created via Block Storage API are NOT visible in Instance API.
        // Instance API returns 404 "instance_volume not found" when trying to attach them via REST API.
        // Solution: Use Scaleway CLI (scw) to attach Block Storage volumes, as the CLI handles
        // the synchronization between Block Storage API and Instance API internally.
        //
        // Command: scw instance server update <server-id> zone=<zone> volumes.0.id=<existing-volume-id> volumes.1.id=<new-volume-id>
        
        eprintln!(
            "🔵 [Scaleway CLI] Attaching Block Storage volume via CLI: volume_id={}, server_id={}, zone={}",
            volume_id, server_id, zone
        );
        
        // First, verify the volume exists in Block Storage API
        let block_storage_url = format!(
            "https://api.scaleway.com/block/v1/zones/{}/volumes/{}",
            zone, volume_id
        );
        
        eprintln!(
            "🔵 [Scaleway CLI] Verifying volume exists in Block Storage API: volume_id={}, zone={}",
            volume_id, zone
        );
        let volume_resp = self.client.get(&block_storage_url).headers(self.headers()).send().await?;
        let volume_status = volume_resp.status();
        if !volume_status.is_success() {
            let error_text = volume_resp.text().await.unwrap_or_default();
            eprintln!("⚠️ [Scaleway CLI] Volume not found in Block Storage API: status={}, response={}", volume_status, error_text);
            return Err(anyhow::anyhow!("Volume not found in Block Storage API: status={}", volume_status));
        }
        
        let volume_json: serde_json::Value = volume_resp.json().await?;
        let volume_obj = volume_json.get("volumes")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_object())
            .or_else(|| volume_json.get("volume").and_then(|v| v.as_object()))
            .or_else(|| volume_json.as_object())
            .ok_or_else(|| {
                eprintln!("⚠️ [Scaleway CLI] Invalid volume response structure");
                anyhow::anyhow!("Invalid volume response")
            })?;
        
        let volume_name = volume_obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
        eprintln!("✅ [Scaleway CLI] Volume exists in Block Storage API: name={}", volume_name);
        
        // Get current server state to retrieve existing volumes
        let get_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        
        let get_resp = self.client.get(&get_url).headers(self.headers()).send().await?;
        if !get_resp.status().is_success() {
            return Err(anyhow::anyhow!("Failed to get server state: status={}", get_resp.status()));
        }
        
        let server_json: serde_json::Value = get_resp.json().await?;
        let server_obj = server_json.get("server")
            .and_then(|s| s.as_object())
            .ok_or_else(|| anyhow::anyhow!("Invalid server response"))?;
        
        // Get existing volumes and build the volumes parameter for CLI
        // Use volume-ids.{index} format as per Scaleway CLI documentation
        let volumes_obj = server_obj.get("volumes")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_else(|| serde_json::Map::new());
        
        // Build CLI command arguments: volume-ids.0=<id0> volume-ids.1=<id1> ...
        let mut volume_ids = Vec::new();
        
        // Add existing volumes (sorted by key to maintain order)
        let mut sorted_keys: Vec<String> = volumes_obj.keys().cloned().collect();
        sorted_keys.sort();
        for key in sorted_keys {
            if let Some(vol_obj) = volumes_obj.get(&key).and_then(|v| v.as_object()) {
                if let Some(existing_id) = vol_obj.get("id").and_then(|id| id.as_str()) {
                    volume_ids.push(existing_id.to_string());
                }
            }
        }
        
        // Add the new Block Storage volume
        volume_ids.push(volume_id.to_string());
        
        // Build volume-ids arguments
        let mut volume_args = Vec::new();
        for (index, vol_id) in volume_ids.iter().enumerate() {
            volume_args.push(format!("volume-ids.{}={}", index, vol_id));
        }
        
        // Build the CLI command
        let mut cmd = tokio::process::Command::new("scw");
        cmd.arg("instance")
            .arg("server")
            .arg("update")
            .arg(server_id)
            .arg(format!("zone={}", zone))
            .args(&volume_args)
            .arg("-o")
            .arg("json");
        
        // Set environment variables for authentication (CLI will use these if config is not set)
        if let Ok(secret_key) = std::env::var("SCALEWAY_SECRET_KEY") {
            cmd.env("SCW_SECRET_KEY", secret_key);
        }
        if let Ok(project_id) = std::env::var("SCALEWAY_PROJECT_ID") {
            cmd.env("SCW_DEFAULT_PROJECT_ID", project_id);
        }
        // Organization ID is required by Scaleway CLI
        // Priority: 1) SCW_DEFAULT_ORGANIZATION_ID, 2) SCALEWAY_ORGANIZATION_ID, 3) From server details
        if std::env::var("SCW_DEFAULT_ORGANIZATION_ID").is_err() {
            if let Ok(org_id) = std::env::var("SCALEWAY_ORGANIZATION_ID") {
                cmd.env("SCW_DEFAULT_ORGANIZATION_ID", org_id);
                eprintln!("🔵 [Scaleway CLI] Using organization ID from SCALEWAY_ORGANIZATION_ID");
            } else {
                // Try to get organization ID from server details (already fetched above)
                if let Some(org_id) = server_obj.get("organization").and_then(|o| o.as_str()) {
                    cmd.env("SCW_DEFAULT_ORGANIZATION_ID", org_id);
                    eprintln!("🔵 [Scaleway CLI] Using organization ID from server: {}", org_id);
                } else {
                    eprintln!("⚠️ [Scaleway CLI] No organization ID found - CLI may fail");
                }
            }
        }
        
        eprintln!(
            "🔵 [Scaleway CLI] Executing: scw instance server update {} zone={} {}",
            server_id, zone, volume_args.join(" ")
        );
        
        let output = cmd.output().await?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            eprintln!("❌ [Scaleway CLI] Command failed: {}", stderr);
            eprintln!("❌ [Scaleway CLI] Output: {}", stdout);
            return Err(anyhow::anyhow!(
                "Scaleway CLI attach_volume failed: {}",
                stderr
            ));
        }
        
        // Verify attachment by checking server volumes
        let verify_resp = self.client.get(&get_url).headers(self.headers()).send().await?;
        if verify_resp.status().is_success() {
            let verify_json: serde_json::Value = verify_resp.json().await?;
            if let Some(volumes) = verify_json.get("server")
                .and_then(|s| s.get("volumes"))
                .and_then(|v| v.as_object())
            {
                let volume_ids: Vec<String> = volumes.values()
                    .filter_map(|v| v.as_object())
                    .filter_map(|v| v.get("id"))
                    .filter_map(|id| id.as_str())
                    .map(|s| s.to_string())
                    .collect();
                
                if volume_ids.contains(&volume_id.to_string()) {
                    eprintln!("✅ [Scaleway CLI] Volume attached successfully: server_id={}, volume_id={}", server_id, volume_id);
                    return Ok(true);
                }
            }
        }
        
        // If verification failed, still return success if CLI command succeeded
        // (the volume might be attached but not yet visible in API)
        eprintln!("⚠️ [Scaleway CLI] Volume attachment command succeeded but verification inconclusive");
        Ok(true)
    }

    // Implementation of provider-specific instance type behavior
    fn requires_diskless_boot(&self, instance_type: &str) -> bool {
        Self::requires_diskless_boot_image(instance_type)
    }

    fn should_pre_create_data_volume(&self, _instance_type: &str) -> bool {
        // Scaleway Block Storage strategy: 
        // - For diskless instances (L4/L40S/H100): Scaleway automatically creates a Block Storage bootable volume (20GB)
        //   We should NOT pre-create volumes - Scaleway handles it automatically, then we resize it after creation
        // - RENDER-S uses auto-created Local Storage, so skip pre-creation
        // - For non-diskless instances: volumes can be created before if needed
        false // Never pre-create volumes - Scaleway creates boot volume automatically for diskless instances
    }

    fn should_skip_data_volume_creation(&self, instance_type: &str) -> bool {
        // RENDER-S instances have auto-created Local Storage (typically 400GB)
        // We should NOT create additional Block Storage volumes for RENDER-S
        Self::is_render_s_instance(instance_type)
    }

    fn get_data_volume_type(&self, instance_type: &str) -> String {
        // RENDER-S uses Local Storage (l_ssd), others use Block Storage (sbs_volume)
        if Self::is_render_s_instance(instance_type) {
            "l_ssd".to_string()
        } else {
            "sbs_volume".to_string()
        }
    }

    fn has_auto_created_storage(&self, instance_type: &str) -> bool {
        // All GPU instances using Scaleway GPU OS image have auto-created Local Storage volumes
        // The GPU image creates a boot volume automatically (typically 20GB for L4/L40S/H100)
        // RENDER-S also has auto-created storage (typically 400GB NVMe)
        // This prevents false positives in diskless boot verification
        Self::requires_diskless_boot_image(instance_type) || Self::is_render_s_instance(instance_type)
    }
}



