use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InstanceStatus {
    Provisioning,
    Running,
    Stopping,
    Stopped,
    Terminated,
    Error,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUProfile {
    pub name: String,
    pub vram_gb: i32,
    pub provider_instance_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloudInstance {
    pub id: String,
    pub provider_id: String,
    pub ip_address: Option<String>,
    pub status: InstanceStatus,
    pub gpu_profile: GPUProfile,
}
