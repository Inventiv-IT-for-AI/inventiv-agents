use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// -----------------------------------------------------------------------------
// Channels / Streams
// -----------------------------------------------------------------------------

pub const CHANNEL_ORCHESTRATOR_COMMANDS: &str = "orchestrator_events";
pub const CHANNEL_FINOPS_EVENTS: &str = "finops_events";

// -----------------------------------------------------------------------------
// Commands (CMD:*)
// -----------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum CommandType {
    #[serde(rename = "CMD:PROVISION")]
    Provision,
    #[serde(rename = "CMD:TERMINATE")]
    Terminate,
    #[serde(rename = "CMD:RECONCILE")]
    Reconcile,
    #[serde(rename = "CMD:SYNC_CATALOG")]
    SyncCatalog,
}

impl CommandType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommandType::Provision => "CMD:PROVISION",
            CommandType::Terminate => "CMD:TERMINATE",
            CommandType::Reconcile => "CMD:RECONCILE",
            CommandType::SyncCatalog => "CMD:SYNC_CATALOG",
        }
    }
}

// -----------------------------------------------------------------------------
// FinOps domain events (EVT:*)
// -----------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum FinopsEventType {
    #[serde(rename = "EVT:INSTANCE_COST_START")]
    InstanceCostStart,
    #[serde(rename = "EVT:INSTANCE_COST_STOP")]
    InstanceCostStop,

    // Future-proof catalog (not fully wired yet):
    #[serde(rename = "EVT:TOKENS_CONSUMED")]
    TokensConsumed,
    #[serde(rename = "EVT:CREDITS_ADDED")]
    CreditsAdded,
    #[serde(rename = "EVT:CUSTOMER_ACTIVATED")]
    CustomerActivated,
    #[serde(rename = "EVT:CUSTOMER_DEACTIVATED")]
    CustomerDeactivated,
    #[serde(rename = "EVT:API_KEY_CREATED")]
    ApiKeyCreated,
    #[serde(rename = "EVT:API_KEY_REVOKED")]
    ApiKeyRevoked,
}

impl FinopsEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FinopsEventType::InstanceCostStart => "EVT:INSTANCE_COST_START",
            FinopsEventType::InstanceCostStop => "EVT:INSTANCE_COST_STOP",
            FinopsEventType::TokensConsumed => "EVT:TOKENS_CONSUMED",
            FinopsEventType::CreditsAdded => "EVT:CREDITS_ADDED",
            FinopsEventType::CustomerActivated => "EVT:CUSTOMER_ACTIVATED",
            FinopsEventType::CustomerDeactivated => "EVT:CUSTOMER_DEACTIVATED",
            FinopsEventType::ApiKeyCreated => "EVT:API_KEY_CREATED",
            FinopsEventType::ApiKeyRevoked => "EVT:API_KEY_REVOKED",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FinopsEventEnvelope {
    pub event_id: Uuid,
    #[serde(rename = "type")]
    pub event_type: FinopsEventType,
    pub occurred_at: DateTime<Utc>,
    pub payload: serde_json::Value,
    pub source: String,
}

impl FinopsEventEnvelope {
    pub fn new(event_type: FinopsEventType, payload: serde_json::Value, source: &str) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type,
            occurred_at: Utc::now(),
            payload,
            source: source.to_string(),
        }
    }
}

