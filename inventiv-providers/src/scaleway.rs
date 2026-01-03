use crate::{inventory, CloudProvider};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

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
    ) -> Result<String> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers",
            zone
        );
        let name = format!("inventiv-worker-{}", uuid::Uuid::new_v4());
        
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
        
        // For diskless boot instances (L4, L40S, RENDER-S), we need to:
        // 1. Completely OMIT the volumes field (not set to empty object) to avoid local volumes
        // 2. Set boot_type to "local" (not "diskless" - that's not a valid boot_type value)
        // 3. The image itself must be diskless-compatible (Ubuntu 22.04+)
        // According to Scaleway docs and CLI examples:
        // - boot_type can be "local", "bootscript", or "rescue"
        // - For L4/L40S instances, boot_type="local" with NO volumes field = diskless boot
        // - The image must be compatible with diskless boot (typically Ubuntu 22.04+)
        // IMPORTANT: Do NOT set volumes={} - completely omit the field to avoid Scaleway creating a default local volume
        if requires_diskless_boot {
            // Do NOT set volumes field at all - completely omit it
            // Setting volumes={} may cause Scaleway to create a default local volume
            // boot_type="local" with no volumes field = diskless boot for L4/L40S instances
            // Note: "diskless" is NOT a valid boot_type value in the API
            body["boot_type"] = json!("local");
            eprintln!(
                "🔵 [Scaleway API] Instance type {} requires diskless boot - omitting volumes field and setting boot_type=\"local\" (diskless mode)",
                instance_type
            );
        }

        // Preferred: pass cloud-init at create-time (standard provisioning flow).
        // Note: some API schemas reject `user_data` (we observed 400: "extra keys not allowed").
        // In that case, we retry without `user_data` so provisioning can continue, and a later
        // SSH fallback install can still bootstrap the worker.
        let has_cloud_init = if let Some(ci) = cloud_init {
            if !ci.trim().is_empty() {
                body["user_data"] = json!({
                    "cloud-init": ci
                });
                true
            } else {
                false
            }
        } else {
            false
        };

        // Log request details
        eprintln!(
            "🔵 [Scaleway API] POST {} - Creating instance: type={}, image={}, zone={}, has_cloud_init={}",
            url, instance_type, image_id, zone, has_cloud_init
        );
        eprintln!("🔵 [Scaleway API] Request payload: {}", serde_json::to_string_pretty(&body).unwrap_or_default());

        let mut resp = self
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
            
            // Retry without user_data on schema errors (best-effort)
            if status_code == 400 && body.get("user_data").is_some() {
                eprintln!("🔄 [Scaleway API] Retrying without user_data (400 error)");
                let mut body2 = body.clone();
                body2.as_object_mut().map(|o| o.remove("user_data"));
                eprintln!("🔵 [Scaleway API] Retry payload: {}", serde_json::to_string_pretty(&body2).unwrap_or_default());
                
                resp = self
                    .client
                    .post(&url)
                    .headers(self.headers())
                    .json(&body2)
                    .send()
                    .await?;
                    
                    let status2 = resp.status();
                let status_code2 = status2.as_u16();
                if !status2.is_success() {
                    let text2 = resp.text().await.unwrap_or_default();
                    eprintln!(
                        "❌ [Scaleway API] Retry failed: status={}, response={}",
                        status_code2, text2
                    );
                    return Err(anyhow::anyhow!(
                        "Scaleway create_instance failed (retry): status={} body={}",
                        status_code2,
                        text2
                    ));
                } else {
                    eprintln!("✅ [Scaleway API] Retry succeeded: status={}", status_code2);
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Scaleway create_instance failed: status={} body={}",
                    status_code,
                    text
                ));
            }
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
                let error_msg = format!(
                    "Scaleway created instance {} with local volumes (l_ssd) but diskless boot is required for instance type {}. \
                    The instance will fail to start with error: 'The total size of local-volume(s) must be equal to 0GB'. \
                    Possible causes: \
                    1) The image {} is not compatible with diskless boot, \
                    2) Scaleway API created a default local boot volume despite volumes={{}}, \
                    3) The instance type requires a different image or configuration. \
                    Please check the image compatibility or use a diskless-compatible image.",
                    server_id, instance_type, image_id
                );
                eprintln!("❌ [Scaleway API] {}", error_msg);
                
                // Return error so the orchestrator can handle it properly (cleanup, etc.)
                return Err(anyhow::anyhow!(error_msg));
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
        // Stop the instance first if it's running
        let current_state = self.get_server_state(zone, server_id).await?;
        if let Some(state) = current_state {
            let state_lower = state.to_ascii_lowercase();
            if state_lower != "stopped" && state_lower != "stopped_in_place" {
                eprintln!(
                    "🔵 [Scaleway API] Instance {} is {} - stopping before termination",
                    server_id, state
                );
                if let Err(e) = self.stop_instance(zone, server_id).await {
                    eprintln!(
                        "⚠️ [Scaleway API] Failed to stop instance {} before termination: {}",
                        server_id, e
                    );
                    // Continue anyway - maybe it will work
                }
            }
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
            
            // If error is about instance needing to be powered off, try stopping again
            if error_text.contains("powered off") || error_text.contains("resource_still_in_use") {
                eprintln!(
                    "🔄 [Scaleway API] Retrying stop before termination for server {}",
                    server_id
                );
                if let Ok(_) = self.stop_instance(zone, server_id).await {
                    // Retry deletion after stopping
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
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
        
        let mut volumes = Vec::new();
        
        if let Some(server_obj) = server {
            // Scaleway returns volumes in server.volumes array
            if let Some(volumes_array) = server_obj.get("volumes").and_then(|v| v.as_array()) {
                for vol in volumes_array {
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
        
        Err(anyhow::anyhow!(
            "Scaleway delete volume failed: status={} body={}",
            status_code2,
            error_text
        ))
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
        
        // Try multiple paths for IP address:
        // 1. public_ip.address (dynamic IP)
        // 2. public_ip.id -> resolve flexible IP (if needed)
        // 3. routed_ipv4 (for flexible IPs with manual routing)
        let ip = json_resp["server"]["public_ip"]["address"]
            .as_str()
            .map(|s| s.to_string());
        
        if ip.is_some() {
            return Ok(ip);
        }
        
        // If public_ip.address is null, check if public_ip.id exists (flexible IP)
        // For now, we return None and let the health-check job retry
        // In the future, we could resolve the flexible IP address via the IPs API
        let public_ip_id = json_resp["server"]["public_ip"]["id"].as_str();
        if public_ip_id.is_some() {
            eprintln!(
                "ℹ️ Scaleway server {} has public_ip.id={} but address is null (flexible IP not yet attached?)",
                server_id, public_ip_id.unwrap_or("")
            );
        }
        
        // Log full public_ip structure for debugging (first 200 chars)
        let public_ip_debug = serde_json::to_string(&json_resp["server"]["public_ip"])
            .unwrap_or_default();
        if public_ip_debug.len() > 0 {
            eprintln!(
                "🔍 Scaleway server {} public_ip structure: {}",
                server_id,
                if public_ip_debug.len() > 200 {
                    &public_ip_debug[..200]
                } else {
                    &public_ip_debug
                }
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
        // Scaleway API: List images in the zone
        // For L4/L40S instances, we need a diskless-compatible image (typically Ubuntu 22.04+)
        // For GPU instances (RENDER-S, L4, L40S), prefer images with NVIDIA drivers pre-installed
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/images",
            zone
        );

        // List public images (Scaleway API may not support all query params, so we filter client-side)
        let resp = self
            .client
            .get(&url)
            .headers(self.headers())
            .query(&[("public", "true")])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            eprintln!(
                "⚠️ Scaleway list_images failed for zone {}: status={} body={}",
                zone, status, text
            );
            return Ok(None);
        }

        let json_resp: serde_json::Value = resp.json().await?;
        let images = json_resp["images"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No 'images' array in response"))?;

        // Detect if this is a GPU instance type
        let instance_type_upper = instance_type.to_uppercase();
        let is_gpu_instance = instance_type_upper.starts_with("RENDER-")
            || instance_type_upper.starts_with("L4-")
            || instance_type_upper.starts_with("L40S-")
            || instance_type_upper.contains("GPU");

        // Prefer Ubuntu 22.04 or newer (diskless-compatible)
        // For GPU instances, prefer images with NVIDIA drivers (contain "nvidia" or "gpu" in name)
        let mut gpu_candidates: Vec<(String, String, i32)> = vec![];
        let mut standard_candidates: Vec<(String, String, i32)> = vec![];

        for img in images {
            if let Some(id) = img["id"].as_str() {
                let name = img["name"].as_str().unwrap_or("").to_lowercase();
                let arch = img["arch"].as_str().unwrap_or("").to_lowercase();

                // Filter: x86_64 architecture and Ubuntu in name
                if arch == "x86_64" && name.contains("ubuntu") {
                    let has_nvidia = name.contains("nvidia") || name.contains("gpu");
                    
                    // Priority calculation
                    let priority = if name.contains("22.04") || name.contains("jammy") {
                        if has_nvidia { 1 } else { 3 }
                    } else if name.contains("24.04") || name.contains("noble") {
                        if has_nvidia { 2 } else { 4 }
                    } else if name.contains("20.04") || name.contains("focal") {
                        if has_nvidia { 5 } else { 7 }
                    } else {
                        if has_nvidia { 6 } else { 8 }
                    };
                    
                    if has_nvidia {
                        gpu_candidates.push((id.to_string(), name.clone(), priority));
                    } else {
                        standard_candidates.push((id.to_string(), name.clone(), priority));
                    }
                }
            }
        }

        // For GPU instances, prefer GPU-optimized images, fallback to standard Ubuntu
        // For non-GPU instances, use standard Ubuntu images
        let candidates = if is_gpu_instance && !gpu_candidates.is_empty() {
            eprintln!(
                "🔍 GPU instance detected ({}), preferring NVIDIA-enabled images",
                instance_type
            );
            gpu_candidates
        } else if is_gpu_instance {
            eprintln!(
                "⚠️ GPU instance ({}) but no NVIDIA-enabled images found, using standard Ubuntu",
                instance_type
            );
            standard_candidates
        } else {
            standard_candidates
        };

        if candidates.is_empty() {
            eprintln!(
                "⚠️ No Ubuntu diskless-compatible images found for zone {} (type {})",
                zone, instance_type
            );
            return Ok(None);
        }

        // Sort by priority (lower is better)
        let mut sorted_candidates = candidates;
        sorted_candidates.sort_by_key(|(_, _, priority)| *priority);

        let (image_id, image_name, _) = &sorted_candidates[0];
        println!(
            "✅ Scaleway boot image resolved: {} ({}) for zone {} type {} (GPU: {})",
            image_id, image_name, zone, instance_type, is_gpu_instance
        );

        Ok(Some(image_id.clone()))
    }

    async fn ensure_inbound_tcp_ports(
        &self,
        _zone: &str,
        _server_id: &str,
        _ports: Vec<u16>,
    ) -> Result<bool> {
        // Not implemented for Scaleway in this crate extraction step.
        Ok(false)
    }

    async fn set_cloud_init(&self, zone: &str, server_id: &str, cloud_init: &str) -> Result<bool> {
        // Scaleway supports setting user_data via PATCH; best-effort implementation.
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/user_data/cloud-init",
            zone, server_id
        );
        let resp = self
            .client
            .put(&url)
            .headers(self.headers())
            .body(cloud_init.to_string())
            .send()
            .await?;
        Ok(resp.status().is_success())
    }
}


