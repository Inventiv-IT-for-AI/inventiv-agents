use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait CloudProvider: Send + Sync {
    async fn create_instance(
        &self,
        zone: &str,
        instance_type: &str,
        image_id: &str,
        cloud_init: Option<&str>,
        volumes: Option<&[String]>, // Optional list of volume IDs to attach at creation
    ) -> Result<String>;
    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool>;
    
    /// Phase 1: Remove local volumes from diskless instance (BEFORE startup).
    /// This must be called AFTER instance creation but BEFORE starting the instance.
    /// If a pre_created_volume_id is provided, it will be attached BEFORE removing local volumes
    /// to ensure the instance always has at least one volume attached (Scaleway requirement).
    /// Returns true if local volumes were found and removed, false if none found.
    /// Default implementation returns Ok(false) (no special handling needed).
    /// 
    /// `pre_created_volume_id`: Optional ID of a Block Storage volume that was created
    /// BEFORE instance creation. If provided, this volume will be attached before removing local volumes.
    async fn remove_local_volumes(
        &self,
        _zone: &str,
        _server_id: &str,
        _instance_type: &str,
        _pre_created_volume_id: Option<&str>,
    ) -> Result<bool> {
        Ok(false)
    }

    /// Phase 2: Attach Block Storage to instance (AFTER startup and SSH accessible).
    /// This must be called AFTER the instance has started and SSH is accessible.
    /// Returns the ID of the attached Block Storage volume.
    /// Default implementation returns Ok("".to_string()) (no special handling needed).
    /// 
    /// `pre_created_volume_id`: Optional ID of a Block Storage volume that was created
    /// BEFORE instance creation. If provided, this volume should be reused instead of creating a new one.
    async fn attach_block_storage_after_boot(
        &self,
        _zone: &str,
        _server_id: &str,
        _instance_type: &str,
        _data_volume_size_gb: u64,
        _pre_created_volume_id: Option<&str>,
    ) -> Result<String> {
        Ok(String::new())
    }

    /// Prepares a diskless boot instance for startup (DEPRECATED - use remove_local_volumes + attach_block_storage_after_boot).
    /// For providers that require special handling (e.g., Scaleway L4/L40S/H100),
    /// this method detaches auto-created local volumes and attaches Block Storage.
    /// Returns the ID of the attached data volume, or empty string if not applicable.
    /// Default implementation returns Ok("".to_string()) (no special preparation needed).
    /// 
    /// `pre_created_volume_id`: Optional ID of a Block Storage volume that was created
    /// BEFORE instance creation. If provided, this volume should be reused instead of creating a new one.
    async fn prepare_diskless_instance(
        &self,
        _zone: &str,
        _server_id: &str,
        _instance_type: &str,
        _data_volume_size_gb: u64,
        _pre_created_volume_id: Option<&str>,
    ) -> Result<String> {
        Ok(String::new())
    }
    
    // Optional: stop/poweroff instance before termination
    // Default implementation returns Ok(false) (not supported)
    async fn stop_instance(&self, _zone: &str, _server_id: &str) -> Result<bool> {
        Ok(false)
    }
    
    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool>;
    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>>;

    // Optional: get server state (e.g., "running", "stopped", "starting")
    // Default implementation returns None (caller cannot wait for specific state).
    async fn get_server_state(
        &self,
        _zone: &str,
        _server_id: &str,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    // New Generic Methods
    async fn check_instance_exists(&self, zone: &str, server_id: &str) -> Result<bool>;

    // Optional: set cloud-init user-data (text/plain) for a server.
    // Default is a no-op so providers without user-data support can compile.
    async fn set_cloud_init(
        &self,
        _zone: &str,
        _server_id: &str,
        _cloud_init: &str,
    ) -> Result<bool> {
        Ok(false)
    }

    // Optional: ensure inbound TCP ports are open (provider firewall / security group).
    // Default is a no-op.
    async fn ensure_inbound_tcp_ports(
        &self,
        _zone: &str,
        _server_id: &str,
        _ports: Vec<u16>,
    ) -> Result<bool> {
        Ok(false)
    }

    // For Catalog Sync, returning a list of generic InstanceType definitions
    async fn fetch_catalog(&self, zone: &str) -> Result<Vec<inventory::CatalogItem>>;

    // For Reconciliation
    async fn list_instances(&self, zone: &str) -> Result<Vec<inventory::DiscoveredInstance>>;

    // Optional: provider-specific boot image resolution.
    // Default implementation returns None (caller falls back to configured image_id).
    async fn resolve_boot_image(
        &self,
        _zone: &str,
        _instance_type: &str,
    ) -> Result<Option<String>> {
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

    async fn attach_volume(
        &self,
        _zone: &str,
        _server_id: &str,
        _volume_id: &str,
        _delete_on_termination: bool,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn delete_volume(&self, _zone: &str, _volume_id: &str) -> Result<bool> {
        Ok(false)
    }

    // Optional: resize a Block Storage volume to a new size (in GB).
    // Used to enlarge volumes created automatically by the provider (e.g., Scaleway creates 20GB Block Storage from image snapshot).
    // Default implementation returns Ok(false) (not supported).
    async fn resize_block_storage(
        &self,
        _zone: &str,
        _volume_id: &str,
        _new_size_gb: u64,
    ) -> Result<bool> {
        Ok(false)
    }

    // Optional: get Block Storage volume size in bytes.
    // Used to retrieve volume size when not available from list_attached_volumes.
    // Default implementation returns Ok(None) (not supported).
    async fn get_block_storage_size(
        &self,
        _zone: &str,
        _volume_id: &str,
    ) -> Result<Option<u64>> {
        Ok(None)
    }

    // Optional: list volumes currently attached to a server.
    // Used to track provider-created boot volumes so we can delete them on termination and avoid leaks.
    async fn list_attached_volumes(
        &self,
        _zone: &str,
        _server_id: &str,
    ) -> Result<Vec<inventory::AttachedVolume>> {
        Ok(vec![])
    }

    // Optional: provider-specific instance type behavior
    // Default implementations return conservative defaults (no special handling needed)

    /// Check if an instance type requires diskless boot (no local volumes at creation)
    /// Default: false (most providers don't have this constraint)
    fn requires_diskless_boot(&self, _instance_type: &str) -> bool {
        false
    }

    /// Check if data volumes should be created BEFORE instance creation
    /// Some providers (e.g., Scaleway Block Storage) require volumes to exist before attachment
    /// Default: false (create volumes after instance creation)
    fn should_pre_create_data_volume(&self, _instance_type: &str) -> bool {
        false
    }

    /// Check if data volume creation should be skipped for this instance type
    /// Some instance types have auto-created storage (e.g., Scaleway RENDER-S with Local Storage)
    /// Default: false (create data volumes normally)
    fn should_skip_data_volume_creation(&self, _instance_type: &str) -> bool {
        false
    }

    /// Get the data volume type for this instance type (e.g., "sbs_volume", "l_ssd")
    /// Default: "sbs_volume" (Block Storage)
    fn get_data_volume_type(&self, _instance_type: &str) -> String {
        "sbs_volume".to_string()
    }

    /// Check if instance type has auto-created storage that should be tracked
    /// Default: false (no auto-created storage)
    fn has_auto_created_storage(&self, _instance_type: &str) -> bool {
        false
    }
}

pub mod inventory {
    #[derive(Clone, Debug)]
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

    #[derive(Clone, Debug)]
    pub struct DiscoveredInstance {
        pub provider_id: String,
        pub name: String,
        pub zone: String,
        pub status: String,
        pub ip_address: Option<String>,
        pub created_at: Option<String>,
    }

    #[derive(Clone, Debug)]
    pub struct AttachedVolume {
        pub provider_volume_id: String,
        pub provider_volume_name: Option<String>,
        pub volume_type: String,
        pub size_bytes: Option<i64>,
        pub boot: bool,
    }
}

#[cfg(feature = "mock")]
pub mod mock;

#[cfg(feature = "scaleway")]
pub mod scaleway;


