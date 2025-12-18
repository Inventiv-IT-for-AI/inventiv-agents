use std::env;

/// Returns a recommended data volume size (GB) for a given model id.
/// This is intentionally heuristic and can be overridden via env vars.
///
/// Overrides:
/// - WORKER_DATA_VOLUME_GB: force a fixed size for all workers (integer GB)
/// - WORKER_DATA_VOLUME_GB_DEFAULT: fallback size when model is unknown (integer GB)
///
/// Notes:
/// - Keeping this in orchestrator avoids VM disk-full failures during bootstrap/model pull.
/// - Provider-specific volume perf (IOPS) stays configured via instance_types.allocation_params.<provider>.data_volume_perf_iops.
pub fn recommended_data_volume_gb(model_id: &str, default_gb: i64) -> Option<i64> {
    if let Ok(v) = env::var("WORKER_DATA_VOLUME_GB") {
        if let Ok(gb) = v.trim().parse::<i64>() {
            if gb > 0 {
                return Some(gb);
            }
        }
    }

    let model = model_id.trim().to_ascii_lowercase();
    if model.is_empty() {
        return Some(default_gb).filter(|gb| *gb > 0);
    }

    // Very small models (sub-1B) typically fit comfortably.
    if model.contains("0.5b")
        || model.contains("0_5b")
        || model.contains("0.6b")
        || model.contains("0_6b")
    {
        return Some(80);
    }
    if model.contains("1b")
        || model.contains("1.5b")
        || model.contains("1_5b")
        || model.contains("2b")
    {
        return Some(120);
    }

    // Common mid-size LLMs.
    if model.contains("7b") || model.contains("8b") {
        return Some(200);
    }
    if model.contains("12b") || model.contains("13b") || model.contains("14b") {
        return Some(300);
    }

    // Larger sizes (safer defaults, especially with vLLM caches).
    if model.contains("24b")
        || model.contains("27b")
        || model.contains("30b")
        || model.contains("32b")
    {
        return Some(500);
    }
    if model.contains("70b") || model.contains("72b") {
        return Some(1000);
    }

    // Fallback for unknown models.
    Some(default_gb).filter(|gb| *gb > 0)
}

// Note: default_gb is provided by caller (provider settings / env / built-in).
