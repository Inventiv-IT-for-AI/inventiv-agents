use serde::{Deserialize, Serialize};
use uuid::Uuid;
use inventiv_common::{InstanceStatus, InstanceType}; // Re-export if needed

// Just a placeholder to make the provider code compile as it referenced crate::models
// In reality, we use inventiv-common types.
pub type CloudInstance = inventiv_common::Instance;
