use async_trait::async_trait;
use anyhow::Result;

#[async_trait]
pub trait CloudProvider: Send + Sync {
    async fn create_instance(&self, zone: &str, instance_type: &str, image_id: &str) -> Result<String>;
    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool>;
    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool>;
    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>>;
    
    // New Generic Methods
    async fn check_instance_exists(&self, zone: &str, server_id: &str) -> Result<bool>;
    
    // For Catalog Sync, returning a list of generic InstanceType definitions
    async fn fetch_catalog(&self, zone: &str) -> Result<Vec<inventory::CatalogItem>>;
    
    // For Reconciliation
    async fn list_instances(&self, zone: &str) -> Result<Vec<inventory::DiscoveredInstance>>;

    // Optional: provider-specific boot image resolution.
    // Default implementation returns None (caller falls back to configured image_id).
    async fn resolve_boot_image(&self, _zone: &str, _instance_type: &str) -> Result<Option<String>> {
        Ok(None)
    }

    // Optional: volume lifecycle (Block Storage, etc.)
    // Default implementations allow providers that don't support volumes to compile.
    async fn create_volume(
        &self,
        _zone: &str,
        _name: &str,
        _size_bytes: i64,
        _volume_type: &str,
        _perf_iops: Option<i32>,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    async fn attach_volume(&self, _zone: &str, _server_id: &str, _volume_id: &str) -> Result<bool> {
        Ok(false)
    }

    async fn delete_volume(&self, _zone: &str, _volume_id: &str) -> Result<bool> {
        Ok(false)
    }
}

pub mod inventory {
    pub struct CatalogItem {
        pub name: String,
        pub code: String,
        pub cost_per_hour: f64,
        pub cpu_count: i32,
        pub ram_gb: i32,
        pub gpu_count: i32,
        pub vram_per_gpu_gb: i32,
        pub bandwidth_bps: i64,
    }

    pub struct DiscoveredInstance {
        pub provider_id: String,
        pub name: String,
        pub zone: String,
        pub status: String,
        pub ip_address: Option<String>,
        pub created_at: Option<String>,
    }
}


