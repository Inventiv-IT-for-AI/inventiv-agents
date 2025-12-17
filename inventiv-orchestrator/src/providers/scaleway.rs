use crate::provider::{inventory, CloudProvider};
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
        headers.insert("X-Auth-Token", self.secret_key.parse().unwrap());
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }

    async fn ensure_security_group_with_ports(&self, zone: &str, ports: &[u16]) -> Result<String> {
        let sg_name = "inventiv-agents-worker-sg";

        // 1) Find existing SG by name+project
        let list_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/security_groups",
            zone
        );
        let resp = self
            .client
            .get(&list_url)
            .headers(self.headers())
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to list security groups: {} - {}",
                status,
                text
            ));
        }
        let v: serde_json::Value = resp.json().await?;
        let mut sg_id: Option<String> = None;
        if let Some(arr) = v.get("security_groups").and_then(|x| x.as_array()) {
            for sg in arr {
                let name = sg.get("name").and_then(|x| x.as_str()).unwrap_or("");
                let project = sg.get("project").and_then(|x| x.as_str()).unwrap_or("");
                if name == sg_name && project == self.project_id {
                    sg_id = sg.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
                    break;
                }
            }
        }

        // 2) Create if missing
        if sg_id.is_none() {
            let create_url = format!(
                "https://api.scaleway.com/instance/v1/zones/{}/security_groups",
                zone
            );
            let body = json!({
                "name": sg_name,
                "project": self.project_id,
                "stateful": true,
                "inbound_default_policy": "drop",
                "outbound_default_policy": "accept",
                "tags": ["inventiv-agents", "worker"],
            });
            let resp = self
                .client
                .post(&create_url)
                .headers(self.headers())
                .json(&body)
                .send()
                .await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!(
                    "Failed to create security group: {} - {}",
                    status,
                    text
                ));
            }
            let v: serde_json::Value = resp.json().await?;
            sg_id = v
                .get("security_group")
                .and_then(|x| x.get("id"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
        }

        let sg_id = sg_id.ok_or_else(|| anyhow::anyhow!("Security group id resolution failed"))?;

        // 3) Replace rules with our allowlist (idempotent)
        let mut rules = Vec::new();
        let mut pos: i32 = 1;
        for p in ports {
            rules.push(json!({
                "action": "accept",
                "protocol": "TCP",
                "direction": "inbound",
                "ip_range": "0.0.0.0/0",
                "dest_port_from": *p as i32,
                "dest_port_to": *p as i32,
                "position": pos,
                "editable": true
            }));
            pos += 1;
        }
        let rules_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/security_groups/{}/rules",
            zone, sg_id
        );
        let resp = self
            .client
            .put(&rules_url)
            .headers(self.headers())
            .json(&json!({ "rules": rules }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to set security group rules: {} - {}",
                status,
                text
            ));
        }

        Ok(sg_id)
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
        let mut body = json!({
            "name": name,
            "commercial_type": instance_type,
            "project": self.project_id,
            "tags": ["inventiv-agents", "worker"],
            "dynamic_ip_required": true
        });
        body["image"] = json!(image_id);

        // Preferred: pass cloud-init at create-time (standard provisioning flow).
        // Note: some API schemas reject `user_data` (we observed 400: "extra keys not allowed").
        // In that case, we retry without `user_data` so provisioning can continue, and a later
        // SSH fallback install can still bootstrap the worker.
        if let Some(ci) = cloud_init {
            if !ci.trim().is_empty() {
                body["user_data"] = json!({
                    "cloud-init": ci
                });
            }
        }

        let mut resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();

            // Retry on schema mismatch for user_data.
            let has_user_data = body.get("user_data").is_some();
            if has_user_data
                && status.as_u16() == 400
                && (text.contains("user_data")
                    && (text.contains("extra keys") || text.contains("extra keys not allowed")))
            {
                let mut body2 = body.clone();
                body2.as_object_mut().map(|o| o.remove("user_data"));
                resp = self
                    .client
                    .post(&url)
                    .headers(self.headers())
                    .json(&body2)
                    .send()
                    .await?;
                if !resp.status().is_success() {
                    let status2 = resp.status();
                    let text2 = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!(
                        "Failed to create instance (retry without user_data): {} - {} (initial: {} - {})",
                        status2,
                        text2,
                        status,
                        text
                    ));
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Failed to create instance: {} - {}",
                    status,
                    text
                ));
            }
        }

        let json: serde_json::Value = resp.json().await?;
        let server_id = json["server"]["id"].as_str().unwrap().to_string();

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
        let resp = self.client.get(&url).headers(self.headers()).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to list images: {} - {}",
                status,
                text
            ));
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
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action",
            zone, server_id
        );
        let body = json!({"action": "poweron"});

        // Scaleway can reject poweron right after volume attach with:
        // 400 precondition_failed resource_not_usable: "All volumes attached to the server must be available."
        // This is transient; retry for a short window.
        let max_wait = Duration::from_secs(60);
        let start = std::time::Instant::now();
        let mut attempt: u32 = 0;

        loop {
            attempt += 1;
            let resp = self
                .client
                .post(&url)
                .headers(self.headers())
                .json(&body)
                .send()
                .await?;

            if resp.status().is_success() {
                return Ok(true);
            }

            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let transient_not_usable = status.as_u16() == 400
                && (text.contains("resource_not_usable")
                    || text.contains("All volumes attached")
                    || text.contains("precondition_failed"));

            if transient_not_usable && start.elapsed() < max_wait {
                // Backoff: 500ms, 1s, 2s, 3s, 5s...
                let delay = match attempt {
                    1 => Duration::from_millis(500),
                    2 => Duration::from_secs(1),
                    3 => Duration::from_secs(2),
                    4 => Duration::from_secs(3),
                    _ => Duration::from_secs(5),
                };
                sleep(delay).await;
                continue;
            }

            return Err(anyhow::anyhow!(
                "Failed to start instance {} in zone {}: {} - {}",
                server_id,
                zone,
                status,
                text
            ));
        }
    }

    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        let resp = self.client.get(&url).headers(self.headers()).send().await?;

        if !resp.status().is_success() {
            match resp.status().as_u16() {
                404 => return Ok(None),
                _ => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!(
                        "Failed to get instance IP: {} - {}",
                        status,
                        text
                    ));
                }
            }
        }

        let json: serde_json::Value = resp.json().await?;
        let ip = json["server"]["public_ip"]["address"]
            .as_str()
            .map(|s| s.to_string());
        Ok(ip)
    }

    async fn set_cloud_init(&self, zone: &str, server_id: &str, cloud_init: &str) -> Result<bool> {
        let ud_url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/user_data/cloud-init",
            zone, server_id
        );
        let ud_resp = self
            .client
            .put(&ud_url)
            .header("X-Auth-Token", &self.secret_key)
            .header("Content-Type", "text/plain")
            .body(cloud_init.to_string())
            .send()
            .await?;
        if !ud_resp.status().is_success() {
            let status = ud_resp.status();
            let text = ud_resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to set Scaleway cloud-init user_data: {} - {}",
                status,
                text
            ));
        }
        Ok(true)
    }

    async fn ensure_inbound_tcp_ports(
        &self,
        zone: &str,
        server_id: &str,
        ports: Vec<u16>,
    ) -> Result<bool> {
        // Ensure we always keep SSH open for debugging.
        let mut ports = ports;
        if !ports.contains(&22) {
            ports.push(22);
        }
        ports.sort_unstable();
        ports.dedup();

        let sg_id = self.ensure_security_group_with_ports(zone, &ports).await?;

        // Attach SG to server (idempotent).
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        let resp = self
            .client
            .patch(&url)
            .headers(self.headers())
            .json(&json!({ "security_group": { "id": sg_id } }))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to attach security group to server: {} - {}",
                status,
                text
            ));
        }
        Ok(true)
    }

    async fn check_instance_exists(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );

        let response = self.client.get(&url).headers(self.headers()).send().await?;

        match response.status().as_u16() {
            200 => Ok(true),  // Instance exists
            404 => Ok(false), // Instance not found
            _ => {
                let status = response.status();
                Err(anyhow::anyhow!(
                    "Unexpected status from provider: {}",
                    status
                ))
            }
        }
    }

    async fn fetch_catalog(&self, zone: &str) -> Result<Vec<inventory::CatalogItem>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/products/servers",
            zone
        );

        let response = self.client.get(&url).headers(self.headers()).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to fetch catalog: {} - {}",
                status,
                text
            ));
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
                let vram_bytes = details["gpu_info"]
                    .get("gpu_memory")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let vram_gb = (vram_bytes / 1024 / 1024 / 1024) as i32;

                let bandwidth_bps = details["network"]
                    .get("sum_internet_bandwidth")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

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
            return Err(anyhow::anyhow!(
                "Failed to create volume: {} - {}",
                status,
                text
            ));
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

    async fn attach_volume(
        &self,
        zone: &str,
        server_id: &str,
        volume_id: &str,
        delete_on_termination: bool,
    ) -> Result<bool> {
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
            return Err(anyhow::anyhow!(
                "Failed to fetch server for attach: {} - {}",
                status,
                text
            ));
        }

        let s: serde_json::Value = get_resp.json().await?;
        let existing = s["server"]["volumes"]
            .as_object()
            .cloned()
            .unwrap_or_default();

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
            // Preserve delete_on_termination when present; otherwise default to true (avoid volume leaks).
            let dot = v
                .get("delete_on_termination")
                .and_then(|x| x.as_bool())
                .unwrap_or(true);
            // IMPORTANT (Scaleway quirk): for existing attached volumes, sending `volume_type`
            // can be rejected (e.g. l_ssd -> "not a valid value"). Provide only id + boot.
            new_volumes.insert(
                k,
                json!({ "id": id, "boot": boot, "delete_on_termination": dot }),
            );
        }
        new_volumes.insert(
            next_key,
            json!({
                "id": volume_id,
                "boot": false,
                "volume_type": "sbs_volume",
                "delete_on_termination": delete_on_termination
            }),
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
            return Err(anyhow::anyhow!(
                "Failed to attach volume: {} - {}",
                status,
                text
            ));
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
                Err(anyhow::anyhow!(
                    "Failed to delete volume: {} - {}",
                    status,
                    text
                ))
            }
        }
    }

    async fn list_instances(&self, zone: &str) -> Result<Vec<inventory::DiscoveredInstance>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers",
            zone
        );
        let resp = self.client.get(&url).headers(self.headers()).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to list instances: {}",
                resp.status()
            ));
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
