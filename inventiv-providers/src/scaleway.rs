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
            // Retry without user_data on schema errors (best-effort)
            if status.as_u16() == 400 && body.get("user_data").is_some() {
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
                        "Scaleway create_instance failed (retry): status={} body={}",
                        status2,
                        text2
                    ));
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Scaleway create_instance failed: status={} body={}",
                    status,
                    text
                ));
            }
        }

        let json_resp: serde_json::Value = resp.json().await?;
        let server_id = json_resp["server"]["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No server id in create response"))?
            .to_string();
        Ok(server_id)
    }

    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action",
            zone, server_id
        );
        let body = json!({ "action": "poweron" });
        let resp = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        let resp = self
            .client
            .delete(&url)
            .headers(self.headers())
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>> {
        let url = format!(
            "https://api.scaleway.com/instance/v1/zones/{}/servers/{}",
            zone, server_id
        );
        let resp = self.client.get(&url).headers(self.headers()).send().await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let json_resp: serde_json::Value = resp.json().await?;
        let ip = json_resp["server"]["public_ip"]["address"]
            .as_str()
            .map(|s| s.to_string());
        Ok(ip)
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

    async fn resolve_boot_image(&self, _zone: &str, _instance_type: &str) -> Result<Option<String>> {
        Ok(None)
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


