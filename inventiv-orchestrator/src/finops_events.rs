use anyhow::Context;
use redis::AsyncCommands;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use inventiv_common::bus::{FinopsEventEnvelope, FinopsEventType, CHANNEL_FINOPS_EVENTS};

pub async fn publish_finops_event(redis_client: &redis::Client, evt: &FinopsEventEnvelope) -> anyhow::Result<()> {
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .context("Failed to connect to Redis (publisher)")?;
    let payload = serde_json::to_string(evt)?;

    // PubSub channel dedicated to FinOps domain events
    let _: () = conn.publish(CHANNEL_FINOPS_EVENTS, payload).await?;
    Ok(())
}

/// Build and publish cost START event from instance row.
pub async fn emit_instance_cost_start(
    db: &Pool<Postgres>,
    redis_client: &redis::Client,
    instance_id: Uuid,
    source: &str,
    note: Option<&str>,
) -> anyhow::Result<()> {
    let row: Option<(Uuid, Option<Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT provider_id, instance_type_id, provider_instance_id::text FROM instances WHERE id = $1",
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await?;

    let Some((provider_id, instance_type_id, provider_instance_id)) = row else {
        return Ok(());
    };

    let evt = FinopsEventEnvelope::new(
        FinopsEventType::InstanceCostStart,
        serde_json::json!({
            "instance_id": instance_id.to_string(),
            "provider_id": provider_id.to_string(),
            "instance_type_id": instance_type_id.map(|v| v.to_string()),
            "provider_instance_id": provider_instance_id,
            "note": note,
        }),
        source,
    );
    publish_finops_event(redis_client, &evt).await
}

/// Build and publish cost STOP event from instance row.
pub async fn emit_instance_cost_stop(
    db: &Pool<Postgres>,
    redis_client: &redis::Client,
    instance_id: Uuid,
    source: &str,
    reason: &str,
) -> anyhow::Result<()> {
    let row: Option<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT provider_id, provider_instance_id::text FROM instances WHERE id = $1",
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await?;

    let Some((provider_id, provider_instance_id)) = row else {
        return Ok(());
    };

    let evt = FinopsEventEnvelope::new(
        FinopsEventType::InstanceCostStop,
        serde_json::json!({
            "instance_id": instance_id.to_string(),
            "provider_id": provider_id.to_string(),
            "provider_instance_id": provider_instance_id,
            "reason": reason,
        }),
        source,
    );
    publish_finops_event(redis_client, &evt).await
}

