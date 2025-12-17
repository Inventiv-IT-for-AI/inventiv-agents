/// Worker-target selection helpers shared across API/Orchestrator.
///
/// We keep this intentionally small: just instance-type allowlist patterns with
/// `*` wildcard support (case-insensitive).
// Scaleway GPU families we support for auto-install by default.
// NOTE: `RENDER-S` is used by Scaleway for some GPU-enabled render/compute SKUs and must be supported
// for our “standard provisioning path” (no manual per-type wiring).
pub const DEFAULT_WORKER_AUTO_INSTALL_INSTANCE_PATTERNS: &str = "L4-*,L40S-*,RENDER-S";

/// Parse comma-separated patterns.
///
/// - Trims whitespace
/// - Drops empty entries
/// - If input is empty/None, returns the default patterns
pub fn parse_instance_type_patterns(raw: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = raw
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if out.is_empty() {
        out = DEFAULT_WORKER_AUTO_INSTALL_INSTANCE_PATTERNS
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
    }

    out
}

/// Return true if `instance_type` matches at least one pattern.
///
/// Pattern rules:
/// - Case-insensitive
/// - `*` matches any substring (including empty)
/// - No other glob features are supported
pub fn instance_type_matches_patterns(instance_type: &str, patterns: &[String]) -> bool {
    let it = instance_type.trim().to_ascii_uppercase();
    if it.is_empty() {
        return false;
    }

    for pat in patterns {
        let p = pat.trim().to_ascii_uppercase();
        if p.is_empty() {
            continue;
        }
        if p == "*" {
            return true;
        }
        if !p.contains('*') {
            if it == p {
                return true;
            }
            continue;
        }

        // Glob-ish matching with '*' as wildcard for any substring.
        let parts: Vec<&str> = p.split('*').collect();
        // All parts empty means pattern was "*" which we handled above.

        let mut idx = 0usize;

        // If pattern doesn't start with '*', first part is a required prefix.
        if !p.starts_with('*') {
            let first = parts.first().copied().unwrap_or("");
            if !first.is_empty() {
                if !it.starts_with(first) {
                    continue;
                }
                idx = first.len();
            }
        }

        // For each middle part, find it after current idx.
        // Note: last part handled separately for suffix constraint.
        let mut ok = true;
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            // Skip first part if we already matched prefix.
            if i == 0 && !p.starts_with('*') {
                continue;
            }
            // Skip last part for suffix check later.
            if i == parts.len().saturating_sub(1) && !p.ends_with('*') {
                continue;
            }

            if let Some(pos) = it[idx..].find(part) {
                idx = idx + pos + part.len();
            } else {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }

        // If pattern doesn't end with '*', last part is a required suffix.
        if !p.ends_with('*') {
            let last = parts.last().copied().unwrap_or("");
            if !last.is_empty() && !it.ends_with(last) {
                continue;
            }
        }

        return true;
    }

    false
}

