use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use anyhow::Result;
use crate::provider::{CloudProvider, inventory};
use std::time::Duration;

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
        Self { client, project_id, secret_key, ssh_public_key }
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("X-Auth-Token", self.secret_key.parse().unwrap());
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }
}

#[async_trait]
impl CloudProvider for ScalewayProvider {
    async fn create_instance(&self, zone: &str, instance_type: &str, image_id: &str) -> Result<String> {
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers", zone);
        let name = format!("inventiv-worker-{}", uuid::Uuid::new_v4());
        let mut body = json!({
            "name": name,
            "commercial_type": instance_type,
            "project": self.project_id,
            "tags": ["inventiv-agents", "worker"],
            "dynamic_ip_required": true
        });
        body["image"] = json!(image_id);

        let resp = self.client.post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await?;
            return Err(anyhow::anyhow!("Failed to create instance: {} - {}", status, text));
        }

        let json: serde_json::Value = resp.json().await?;
        let server_id = json["server"]["id"].as_str().unwrap().to_string();

        // Inject SSH key (optional) via cloud-init user-data.
        //
        // IMPORTANT: On the Instance API, `user_data` in the create-server payload is not always accepted
        // (we observed 400: "user_data ... extra keys not allowed"). To be robust across API schema changes,
        // we set user_data via the dedicated endpoint after server creation.
        if let Some(pk) = self.ssh_public_key.as_deref() {
            let cloud_init = format!(
                "#cloud-config\nssh_authorized_keys:\n  - {}\n",
                pk.replace('\n', " ")
            );
            let ud_url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/user_data/cloud-init",
                zone, server_id
            );
            let ud_resp = self
                .client
                .put(&ud_url)
                .header("X-Auth-Token", &self.secret_key)
                .header("Content-Type", "text/plain")
                .body(cloud_init)
                .send()
                .await?;
            if !ud_resp.status().is_success() {
                let status = ud_resp.status();
                let text = ud_resp.text().await.unwrap_or_default();
                eprintln!("⚠️ Failed to set Scaleway cloud-init user_data: {} - {}", status, text);
            }
        }

        Ok(server_id)
    }

    async fn resolve_boot_image(&self, zone: &str, instance_type: &str) -> Result<Option<String>> {
        // Some Scaleway GPU instance families require *no local volumes* (diskless boot image).
        // Example: L4-*, L40S-* (error at poweron: local-volume(s) must be equal to 0GB).
        fn requires_diskless_boot_image(instance_type: &str) -> bool {
            let t = instance_type.trim().to_ascii_uppercase();
            t.starts_with("L4-") || t.starts_with("L40S-")
        }
        if !requires_diskless_boot_image(instance_type) {
            return Ok(None);
        }

        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/images", zone);
        let resp = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to list images: {} - {}", status, text));
        }

        let v: serde_json::Value = resp.json().await?;
        let images = v["images"].as_array().cloned().unwrap_or_default();

        // Heuristic: prefer GPU/NVIDIA images, and prefer images that don't declare a local root volume.
        let mut best: Option<(i32, String)> = None;

        for img in images {
            let id = match img.get("id").and_then(|x| x.as_str()) {
                Some(s) if !s.trim().is_empty() => s.to_string(),
                _ => continue,
            };
            let name = img
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string();
            let upper = name.to_ascii_uppercase();

            // root_volume can hint the backing disk. If it looks like local storage, skip.
            // (Field shape can vary; be defensive.)
            let mut looks_local = false;
            if let Some(rv) = img.get("root_volume") {
                // e.g. {"id": "...", "name": "...", "volume_type": "l_ssd", "size": 20000000000}
                if let Some(vt) = rv.get("volume_type").and_then(|x| x.as_str()) {
                    // Treat any "l_*" volume type as local (l_ssd, l_hdd, etc.).
                    if vt.to_ascii_lowercase().starts_with("l_") {
                        looks_local = true;
                    }
                }
                if let Some(size) = rv.get("size").and_then(|x| x.as_i64()) {
                    if size > 0 && looks_local {
                        // local root volume with size -> definitely not diskless
                        looks_local = true;
                    }
                }
            }
            if looks_local {
                continue;
            }

            let mut score: i32 = 0;
            if upper.contains("GPU") || upper.contains("NVIDIA") || upper.contains("CUDA") {
                score += 50;
            }
            if upper.contains("UBUNTU") {
                score += 10;
            }
            if upper.contains("JAMMY") || upper.contains("22.04") {
                score += 5;
            }
            if upper.contains("DEBIAN") {
                score += 3;
            }

            match &best {
                Some((best_score, _)) if *best_score >= score => {}
                _ => best = Some((score, id)),
            }
        }

        Ok(best.map(|(_, id)| id))
    }

    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action", zone, server_id);
        let body = json!({"action": "poweron"});
        
        let resp = self.client.post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to start instance {} in zone {}: {} - {}",
                server_id,
                zone,
                status,
                text
            ));
        }

        Ok(true)
    }

    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>> {
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers/{}", zone, server_id);
        let resp = self.client.get(&url)
            .headers(self.headers())
            .send()
            .await?;
            
        if !resp.status().is_success() {
            match resp.status().as_u16() {
                404 => return Ok(None),
                _ => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("Failed to get instance IP: {} - {}", status, text));
                }
            }
        }
        
        let json: serde_json::Value = resp.json().await?;
        let ip = json["server"]["public_ip"]["address"].as_str().map(|s| s.to_string());
        Ok(ip)
    }

    async fn check_instance_exists(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );

        let response = self.client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;

        match response.status().as_u16() {
            200 => Ok(true),   // Instance exists
            404 => Ok(false),  // Instance not found
            _ => {
                let status = response.status();
                Err(anyhow::anyhow!("Unexpected status from provider: {}", status))
            }
        }
    }

    async fn fetch_catalog(&self, zone: &str) -> Result<Vec<inventory::CatalogItem>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/products/servers",
            zone
        );

        let response = self.client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to fetch catalog: {} - {}", status, text));
        }

        let json: serde_json::Value = response.json().await?;
        let mut items = Vec::new();

        if let Some(servers) = json.get("servers").and_then(|v| v.as_object()) {
            for (code, details) in servers {
                 // Extract specs
                let hourly_price = details["hourly_price"].as_f64().unwrap_or(0.0);
                let ncpus = details["ncpus"].as_i64().unwrap_or(0) as i32;
                let ram_bytes = details["ram"].as_i64().unwrap_or(0);
                let ram_gb = (ram_bytes / 1024 / 1024 / 1024) as i32;
                
                // GPU Info
                let gpu_count = details["gpu"].as_i64().unwrap_or(0) as i32;
                let vram_bytes = details["gpu_info"].get("gpu_memory").and_then(|v| v.as_i64()).unwrap_or(0);
                let vram_gb = (vram_bytes / 1024 / 1024 / 1024) as i32;
                
                let bandwidth_bps = details["network"].get("sum_internet_bandwidth").and_then(|v| v.as_i64()).unwrap_or(0);

                items.push(inventory::CatalogItem {
                    name: code.clone(), // Use commercial type as name
                    code: code.clone(),
                    cost_per_hour: hourly_price,
                    cpu_count: ncpus,
                    ram_gb,
                    gpu_count,
                    vram_per_gpu_gb: vram_gb,
                    bandwidth_bps,
                });
            }
        }
        
        Ok(items)
    }

    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        // Preferred: terminate action
        let action_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action",
            zone, server_id
        );
        let body = json!({"action": "terminate"});

        let resp = self
            .client
            .post(&action_url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        if resp.status().is_success() {
            return Ok(true);
        }

        // Fallback: some server states can reject terminate action (e.g. stopped).
        // Use DELETE /servers/{id} which is the canonical delete endpoint.
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.as_u16() == 400 && text.contains("resource_not_usable") {
            let delete_url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
                zone, server_id
            );
            let delete_resp = self
                .client
                .delete(&delete_url)
                .headers(self.headers())
                .send()
                .await?;

            if delete_resp.status().is_success() {
                return Ok(true);
            }

            let d_status = delete_resp.status();
            let d_text = delete_resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to terminate instance {} in zone {}: {} - {} (fallback DELETE failed: {} - {})",
                server_id,
                zone,
                status,
                text,
                d_status,
                d_text
            ));
        }

        Err(anyhow::anyhow!(
            "Failed to terminate instance {} in zone {}: {} - {}",
            server_id,
            zone,
            status,
            text
        ))
    }

    async fn create_volume(
        &self,
        zone: &str,
        name: &str,
        size_bytes: i64,
        volume_type: &str,
        perf_iops: Option<i32>,
    ) -> Result<Option<String>> {
        // Only SBS volumes supported here (Block Storage API).
        if volume_type != "sbs_volume" {
            return Ok(None);
        }

        let url = format!("https://api.scaleway.com/block/v1/zones/{}/volumes", zone);
        let perf = perf_iops.unwrap_or(5000);

        let body = json!({
            "project_id": self.project_id,
            "name": name,
            "perf_iops": perf,
            "from_empty": { "size": size_bytes }
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to create volume: {} - {}", status, text));
        }

        let v: serde_json::Value = resp.json().await?;
        // Block Storage API returns the volume as a top-level object (id, name, ...).
        // Some responses may also wrap it (defensive).
        let vol_id = v["id"]
            .as_str()
            .or_else(|| v["volume"]["id"].as_str())
            .map(|s| s.to_string());
        Ok(vol_id)
    }

    async fn attach_volume(&self, zone: &str, server_id: &str, volume_id: &str) -> Result<bool> {
        // Need to PATCH server volumes with full set (include existing volumes).
        let get_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        let get_resp = self
            .client
            .get(&get_url)
            .headers(self.headers())
            .send()
            .await?;

        if !get_resp.status().is_success() {
            let status = get_resp.status();
            let text = get_resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to fetch server for attach: {} - {}", status, text));
        }

        let s: serde_json::Value = get_resp.json().await?;
        let existing = s["server"]["volumes"].as_object().cloned().unwrap_or_default();

        // Determine next volume slot key (volumes are keyed by strings "0", "1", ...)
        let mut max_idx: i32 = -1;
        for k in existing.keys() {
            if let Ok(n) = k.parse::<i32>() {
                if n > max_idx {
                    max_idx = n;
                }
            }
        }
        let next_key = (max_idx + 1).to_string();

        let mut new_volumes = serde_json::Map::new();
        for (k, v) in existing {
            // Preserve existing volume attachments by id + boot flag.
            let id = v.get("id").and_then(|x| x.as_str()).unwrap_or_default();
            if id.is_empty() {
                continue;
            }
            let boot = v.get("boot").and_then(|x| x.as_bool()).unwrap_or(false);
            // IMPORTANT (Scaleway quirk): for existing attached volumes, sending `volume_type`
            // can be rejected (e.g. l_ssd -> "not a valid value"). Provide only id + boot.
            new_volumes.insert(k, json!({ "id": id, "boot": boot }));
        }
        new_volumes.insert(
            next_key,
            json!({ "id": volume_id, "boot": false, "volume_type": "sbs_volume" }),
        );

        let patch_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        let patch_body = json!({ "volumes": new_volumes });
        let patch_resp = self
            .client
            .patch(&patch_url)
            .headers(self.headers())
            .json(&patch_body)
            .send()
            .await?;

        if !patch_resp.status().is_success() {
            let status = patch_resp.status();
            let text = patch_resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to attach volume: {} - {}", status, text));
        }
        Ok(true)
    }

    async fn delete_volume(&self, zone: &str, volume_id: &str) -> Result<bool> {
        let url = format!(
            "https://api.scaleway.com/block/v1/zones/{}/volumes/{}",
            zone, volume_id
        );
        let resp = self
            .client
            .delete(&url)
            .headers(self.headers())
            .send()
            .await?;

        match resp.status().as_u16() {
            200 | 202 | 204 | 404 => Ok(true),
            _ => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                Err(anyhow::anyhow!("Failed to delete volume: {} - {}", status, text))
            }
        }
    }

    async fn list_instances(&self, zone: &str) -> Result<Vec<inventory::DiscoveredInstance>> {
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers", zone);
        let resp = self.client.get(&url)
            .headers(self.headers())
            .send()
            .await?;

        if !resp.status().is_success() {
             return Err(anyhow::anyhow!("Failed to list instances: {}", resp.status()));
        }

        let json: serde_json::Value = resp.json().await?;
        let mut instances = Vec::new();

        if let Some(servers) = json["servers"].as_array() {
            for s in servers {
                let id = s["id"].as_str().unwrap_or_default().to_string();
                let name = s["name"].as_str().unwrap_or_default().to_string();
                let status = s["state"].as_str().unwrap_or_default().to_string();
                let ip = s["public_ip"]["address"].as_str().map(|s| s.to_string());
                let created_at = s["creation_date"].as_str().map(|s| s.to_string());

                instances.push(inventory::DiscoveredInstance {
                    provider_id: id,
                    name,
                    zone: zone.to_string(),
                    status,
                    ip_address: ip,
                    created_at,
                });
            }
        }
        Ok(instances)
    }
}
