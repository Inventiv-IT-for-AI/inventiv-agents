use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, QueryBuilder};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct ActionLogsSearchQuery {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub instance_id: Option<Uuid>,
    pub component: Option<String>,
    pub status: Option<String>,
    pub action_type: Option<String>,
    /// Sort field (allowlist). Example: created_at, duration_ms, status, component, action_type, provider_name, instance_type, instance_id
    pub sort_by: Option<String>,
    /// Sort direction. "asc" or "desc" (default: desc)
    pub sort_dir: Option<String>,
    /// If true, computes `status_counts` for the filtered set (extra query).
    pub include_stats: Option<bool>,
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema, Clone)]
pub struct ActionLogRow {
    pub id: Uuid,
    pub action_type: String,
    pub component: String,
    pub status: String,
    pub provider_name: Option<String>,
    pub instance_type: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub instance_id: Option<Uuid>,
    pub duration_ms: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub instance_status_before: Option<String>,
    pub instance_status_after: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema, Default)]
pub struct StatusCounts {
    pub success: i64,
    pub failed: i64,
    pub in_progress: i64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ActionLogsSearchResponse {
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub filtered_count: i64,
    pub status_counts: Option<StatusCounts>,
    pub rows: Vec<ActionLogRow>,
}

fn push_filters(qb: &mut QueryBuilder<'_, Postgres>, params: &ActionLogsSearchQuery) {
    if let Some(instance_id) = params.instance_id {
        qb.push(" AND al.instance_id = ");
        qb.push_bind(instance_id);
    }
    if let Some(component) = params.component.as_deref().filter(|s| !s.trim().is_empty()) {
        // Backward compatible: some rows use 'backend', but canonical is now 'api'.
        if component == "api" || component == "backend" {
            qb.push(" AND al.component IN (");
            qb.push_bind("api".to_string());
            qb.push(", ");
            qb.push_bind("backend".to_string());
            qb.push(")");
        } else {
            qb.push(" AND al.component = ");
            qb.push_bind(component.to_string());
        }
    }
    if let Some(status) = params.status.as_deref().filter(|s| !s.trim().is_empty()) {
        qb.push(" AND al.status = ");
        qb.push_bind(status.to_string());
    }
    if let Some(action_type) = params
        .action_type
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        qb.push(" AND al.action_type = ");
        qb.push_bind(action_type.to_string());
    }
}

/// Backend-driven filtering + offset pagination for virtualized UIs.
#[utoipa::path(
    get,
    path = "/action_logs/search",
    tag = "ActionLogs",
    params(ActionLogsSearchQuery),
    responses(
        (status = 200, description = "Paged list of action logs", body = ActionLogsSearchResponse)
    )
)]
pub async fn search_action_logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ActionLogsSearchQuery>,
) -> Json<ActionLogsSearchResponse> {
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);
    let include_stats = params.include_stats.unwrap_or(false);

    let sort_by = params
        .sort_by
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let sort_dir = params
        .sort_dir
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s == "asc" || s == "desc")
        .unwrap_or_else(|| "desc".to_string());

    // Total count (no filters)
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM action_logs")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    // Filtered count
    let mut filtered_count_qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM action_logs al WHERE 1=1");
    push_filters(&mut filtered_count_qb, &params);
    let filtered_count: i64 = filtered_count_qb
        .build_query_scalar()
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    // Rows
    let mut rows_qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"SELECT
            al.id,
            al.action_type,
            CASE WHEN al.component = 'backend' THEN 'api' ELSE al.component END as component,
            al.status,
            p.name as provider_name,
            it.name as instance_type,
            al.error_code,
            al.error_message,
            al.instance_id,
            al.duration_ms,
            al.created_at,
            al.completed_at,
            al.metadata,
            al.instance_status_before,
            al.instance_status_after
          FROM action_logs al
          LEFT JOIN instances i ON i.id = al.instance_id
          LEFT JOIN providers p ON p.id = i.provider_id
          LEFT JOIN instance_types it ON it.id = i.instance_type_id
          WHERE 1=1"#,
    );
    push_filters(&mut rows_qb, &params);
    // Safe ORDER BY (allowlist only; no user-controlled SQL injection)
    rows_qb.push(" ORDER BY ");
    match sort_by.as_deref() {
        Some("created_at") => {
            rows_qb.push("al.created_at ");
            rows_qb.push(&sort_dir);
        }
        Some("duration_ms") => {
            rows_qb.push("al.duration_ms ");
            rows_qb.push(&sort_dir);
            rows_qb.push(" NULLS LAST");
        }
        Some("status") => {
            rows_qb.push("al.status ");
            rows_qb.push(&sort_dir);
        }
        Some("component") => {
            // Keep consistent with "backend" -> "api" normalization
            rows_qb.push("CASE WHEN al.component = 'backend' THEN 'api' ELSE al.component END ");
            rows_qb.push(&sort_dir);
        }
        Some("action_type") => {
            rows_qb.push("al.action_type ");
            rows_qb.push(&sort_dir);
        }
        Some("provider_name") => {
            rows_qb.push("p.name ");
            rows_qb.push(&sort_dir);
            rows_qb.push(" NULLS LAST");
        }
        Some("instance_type") => {
            rows_qb.push("it.name ");
            rows_qb.push(&sort_dir);
            rows_qb.push(" NULLS LAST");
        }
        Some("instance_id") => {
            rows_qb.push("al.instance_id ");
            rows_qb.push(&sort_dir);
            rows_qb.push(" NULLS LAST");
        }
        _ => {
            // Default
            rows_qb.push("al.created_at DESC");
        }
    }
    // Deterministic tie-breaker for virtualization pagination
    rows_qb.push(", al.id DESC");
    rows_qb.push(" LIMIT ");
    rows_qb.push_bind(limit);
    rows_qb.push(" OFFSET ");
    rows_qb.push_bind(offset);

    let rows: Vec<ActionLogRow> = match rows_qb.build_query_as().fetch_all(&state.db).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("⚠️  action_logs/search rows query failed: {}", e);
            Vec::new()
        }
    };

    // Optional stats for the filtered set
    let status_counts = if include_stats {
        let mut qb: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT al.status, COUNT(*) FROM action_logs al WHERE 1=1");
        push_filters(&mut qb, &params);
        qb.push(" GROUP BY status");
        let pairs: Vec<(String, i64)> = qb
            .build_query_as()
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

        let mut counts = StatusCounts::default();
        for (status, count) in pairs {
            match status.as_str() {
                "success" => counts.success = count,
                "failed" => counts.failed = count,
                "in_progress" => counts.in_progress = count,
                _ => {}
            }
        }
        Some(counts)
    } else {
        None
    };

    Json(ActionLogsSearchResponse {
        offset,
        limit,
        total_count,
        filtered_count,
        status_counts,
        rows,
    })
}
