use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use crate::models::CloudInstance; // Assuming models are available
use inventiv_common::{InstanceStatus, InstanceType};
use anyhow::Result;

#[async_trait]
pub trait CloudProvider: Send + Sync {
    async fn create_instance(&self, zone: &str, instance_type: &str, image_id: &str) -> Result<String>;
    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool>;
    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool>;
    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>>;
}

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
            let err_text = resp.text().await?;
            return Err(anyhow::anyhow!("Scaleway API Error: {}", err_text));
        }

        let json: serde_json::Value = resp.json().await?;
        let server_id = json["server"]["id"].as_str()
            .ok_or_else(|| anyhow::anyhow!("No server ID in response"))?;
            
        Ok(server_id.to_string())
    }

    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action", zone, server_id);
        let body = json!({ "action": "poweron" });
        
        let resp = self.client.post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;
            
        Ok(resp.status().is_success())
    }

    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        // Force terminate
        let url = format!("https://api.scaleway.com/instance/v1/zones/{}/servers/{}/action", zone, server_id);
        let body = json!({ "action": "terminate" });
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
}
