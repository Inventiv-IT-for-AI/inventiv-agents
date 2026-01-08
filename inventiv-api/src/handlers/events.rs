// Events stream handler (SSE)
use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;

use crate::app::AppState;

#[derive(Deserialize)]
pub struct EventsStreamParams {
    // Optional: narrow action log events to a specific instance
    instance_id: Option<uuid::Uuid>,
    // Comma-separated topics. Default: instances,actions
    topics: Option<String>,
}

#[derive(serde::Serialize)]
struct InstancesChangedPayload {
    ids: Vec<uuid::Uuid>,
    emitted_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize)]
struct ActionLogsChangedPayload {
    ids: Vec<uuid::Uuid>,
    instance_ids: Vec<uuid::Uuid>,
    emitted_at: chrono::DateTime<chrono::Utc>,
}

pub async fn events_stream(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EventsStreamParams>,
) -> impl axum::response::IntoResponse {
    let topics_raw = params
        .topics
        .unwrap_or_else(|| "instances,actions".to_string());
    let topics: std::collections::HashSet<String> = topics_raw
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let db = state.db.clone();
    let instance_id_filter = params.instance_id;

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        // IMPORTANT:
        // We do NOT want to emit "instance.updated" on noisy changes like heartbeats.
        // We compute a stable signature (hash) for "meaningful" instance fields and only emit when it changes.
        // On connect, we initialize the signature map but do not emit (UI will fetch initial state anyway).
        let mut instances_initialized = false;
        let mut instance_sig: std::collections::HashMap<uuid::Uuid, String> =
            std::collections::HashMap::new();

        let mut last_actions_ts = chrono::Utc::now();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));

        // Quick handshake
        let hello = Event::default().event("hello").data(r#"{"ok":true}"#);
        if tx.send(Ok(hello)).await.is_err() {
            return;
        }

        loop {
            interval.tick().await;

            if topics.contains("instances") {
                #[derive(sqlx::FromRow)]
                struct InstanceSigRow {
                    id: uuid::Uuid,
                    sig: String,
                }

                // Signature excludes "noisy" fields (heartbeats, health checks, reconciliation, worker telemetry)
                // and ALSO excludes error/debug fields that can change frequently during retries but are not
                // displayed in the Instances list UI.
                //
                // It includes only fields that are user-visible in the Instances table (and affect sorting/filtering).
                let rows: Vec<InstanceSigRow> = sqlx::query_as(
                    r#"
                    SELECT
                      id,
                      md5(
                        concat_ws(
                          '|',
                          COALESCE(status::text, ''),
                          COALESCE(is_archived::text, ''),
                          COALESCE(provider_id::text, ''),
                          COALESCE(zone_id::text, ''),
                          COALESCE(instance_type_id::text, ''),
                          COALESCE(model_id::text, ''),
                          COALESCE(ip_address::text, '')
                        )
                      ) AS sig
                    FROM instances
                    "#,
                )
                .fetch_all(&db)
                .await
                .unwrap_or_default();

                if !instances_initialized {
                    instance_sig.clear();
                    for r in rows {
                        instance_sig.insert(r.id, r.sig);
                    }
                    instances_initialized = true;
                } else {
                    let mut seen = std::collections::HashSet::with_capacity(rows.len());
                    let mut changed: Vec<uuid::Uuid> = Vec::new();

                    for r in rows {
                        seen.insert(r.id);
                        match instance_sig.get(&r.id) {
                            Some(prev) if prev == &r.sig => {}
                            _ => {
                                instance_sig.insert(r.id, r.sig);
                                changed.push(r.id);
                            }
                        }
                    }

                    // Remove signatures for deleted instances
                    instance_sig.retain(|id, _| seen.contains(id));

                    if !changed.is_empty() {
                        // Keep payload size reasonable; send in chunks.
                        for chunk in changed.chunks(200) {
                            let payload = InstancesChangedPayload {
                                ids: chunk.to_vec(),
                                emitted_at: chrono::Utc::now(),
                            };
                            let ev = Event::default().event("instance.updated").data(
                                serde_json::to_string(&payload)
                                    .unwrap_or_else(|_| "{}".to_string()),
                            );
                            if tx.send(Ok(ev)).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }

            if topics.contains("actions") || topics.contains("action_logs") {
                // Important: action logs often "update in place" (status in_progress -> success/failed)
                // by setting completed_at + duration + metadata. Track changes using changed_at.
                let rows: Vec<(
                    uuid::Uuid,
                    Option<uuid::Uuid>,
                    chrono::DateTime<chrono::Utc>,
                )> = sqlx::query_as(
                    r#"
                    SELECT
                      id,
                      instance_id,
                      GREATEST(created_at, COALESCE(completed_at, created_at)) AS changed_at
                    FROM action_logs
                    WHERE GREATEST(created_at, COALESCE(completed_at, created_at)) > $1
                      AND ($2::uuid IS NULL OR instance_id = $2)
                    ORDER BY changed_at ASC
                    LIMIT 500
                    "#,
                )
                .bind(last_actions_ts)
                .bind(instance_id_filter)
                .fetch_all(&db)
                .await
                .unwrap_or_default();

                if !rows.is_empty() {
                    let mut max_ts = last_actions_ts;
                    let mut ids = Vec::with_capacity(rows.len());
                    let mut inst_ids = std::collections::BTreeSet::new();
                    for (id, inst, ts) in rows {
                        ids.push(id);
                        if let Some(iid) = inst {
                            inst_ids.insert(iid);
                        }
                        if ts > max_ts {
                            max_ts = ts;
                        }
                    }
                    last_actions_ts = max_ts;

                    let payload = ActionLogsChangedPayload {
                        ids,
                        instance_ids: inst_ids.into_iter().collect(),
                        emitted_at: chrono::Utc::now(),
                    };
                    // Keep event name stable for the UI: treat it as "action log changed".
                    let ev = Event::default()
                        .event("action_log.created")
                        .data(serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()));
                    if tx.send(Ok(ev)).await.is_err() {
                        return;
                    }
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    )
}
