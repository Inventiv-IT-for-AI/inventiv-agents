use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use anyhow::Result;
use crate::provider::{CloudProvider, inventory};

pub struct ScalewayProvider {
    client: Client,
    project_id: String,
    secret_key: String,
}

impl ScalewayProvider {
    pub fn new(project_id: String, secret_key: String) -> Self {
        let client = Client::builder().build().unwrap();
        Self { client, project_id, secret_key }
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
        let body = json!({
            "name": format!("inventiv-worker-{}", uuid::Uuid::new_v4()),
            "commercial_type": instance_type,
            "project": self.project_id,
            "image": image_id,
            "tags": ["inventiv-agents", "worker"],
            "dynamic_ip_required": true // We need public IP for now
        });

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
        Ok(server_id)
    }

    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action", zone, server_id);
        let body = json!({"action": "poweron"});
        
        let resp = self.client.post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        Ok(resp.status().is_success())
    }

    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>> {
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers/{}", zone, server_id);
        let resp = self.client.get(&url)
            .headers(self.headers())
            .send()
            .await?;
            
        if !resp.status().is_success() { return Ok(None); }
        
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
                let n_gpu = details["gpu"].as_i64().unwrap_or(0) as i32;
                let vram_bytes = details["gpu_info"].get("gpu_memory").and_then(|v| v.as_i64()).unwrap_or(0);
                let vram_gb = (vram_bytes / 1024 / 1024 / 1024) as i32;
                
                let bandwidth_bps = details["network"].get("sum_internet_bandwidth").and_then(|v| v.as_i64()).unwrap_or(0);

                items.push(inventory::CatalogItem {
                    name: code.clone(), // Use commercial type as name
                    code: code.clone(),
                    cost_per_hour: hourly_price,
                    cpu_count: ncpus,
                    ram_gb,
                    n_gpu,
                    vram_per_gpu_gb: vram_gb,
                    bandwidth_bps,
                });
            }
        }
        
        Ok(items)
    }

    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action", zone, server_id);
        let body = json!({"action": "terminate"});
        
        let resp = self.client.post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        Ok(resp.status().is_success())
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
