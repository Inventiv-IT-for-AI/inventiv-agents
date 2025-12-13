use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// --- Enums ---

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type, utoipa::ToSchema)]
#[sqlx(type_name = "instance_status", rename_all = "lowercase")]
pub enum InstanceStatus {
    Provisioning, // Request sent to provider
    Booting,      // Instance is up, installing/loading
    Ready,        // Healthy and serving traffic
    Draining,     // Stopping, finishing current requests
    Terminated,   // Destroyed
    Failed,       // Error state
}

// --- Entities (SQLx Mapped) ---

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip)] // Never serialize password hash
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow, utoipa::ToSchema)]
pub struct Provider {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow, utoipa::ToSchema)]
pub struct Region {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow, utoipa::ToSchema)]
pub struct Zone {
    pub id: Uuid,
    pub region_id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow, utoipa::ToSchema)]
pub struct InstanceType {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub gpu_count: i32,
    pub vram_per_gpu_gb: i32,
    pub is_active: bool,
    #[sqlx(default)] 
    pub cost_per_hour: Option<f64>, // Using f64 for simplicity, mapped from numeric
    #[sqlx(default)]
    pub cpu_count: i32,
    #[sqlx(default)]
    pub ram_gb: i32,
    #[sqlx(default)]
    pub n_gpu: i32,     // New column
    #[sqlx(default)]
    pub bandwidth_bps: i64, // Bigint
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct LlmModel {
    pub id: Uuid,
    pub name: String,
    pub model_id: String,
    pub required_vram_gb: i32,
    pub context_length: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow, utoipa::ToSchema)]
pub struct Instance {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub zone_id: Uuid,
    pub instance_type_id: Uuid,
    pub model_id: Option<Uuid>,
    
    pub provider_instance_id: Option<String>,
    pub ip_address: Option<String>, // Note: INET type handling needs careful SQLx mapping or String cast
    pub api_key: Option<String>,
    
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub terminated_at: Option<DateTime<Utc>>,
    pub gpu_profile: sqlx::types::Json<InstanceType>, // Snapshot using InstanceType struct
    
    // Deletion tracking fields for orphaned instance detection
    pub deletion_reason: Option<String>,
    pub deleted_by_provider: Option<bool>,
    pub last_reconciliation: Option<DateTime<Utc>>,  // Updated on every reconciliation check
}
