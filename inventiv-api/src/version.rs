// Version information module
// Reads version from VERSION file at compile time and runtime

use std::fs;

/// Get the backend version from VERSION file
/// Falls back to "unknown" if file cannot be read
pub fn get_backend_version() -> String {
    // Try to read from VERSION file at runtime
    // This allows updating version without recompiling
    if let Ok(contents) = fs::read_to_string("VERSION") {
        return contents.trim().to_string();
    }

    // Fallback: try relative path from binary location
    if let Ok(contents) = fs::read_to_string("../VERSION") {
        return contents.trim().to_string();
    }

    // Fallback: try from project root (for development)
    if let Ok(contents) = fs::read_to_string("../../VERSION") {
        return contents.trim().to_string();
    }

    // Last resort: use compile-time version if available
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get version info response structure
#[derive(Debug, serde::Serialize)]
pub struct VersionInfo {
    pub backend_version: String,
    pub build_time: String,
}

/// Get version info for API response
pub fn get_version_info() -> VersionInfo {
    VersionInfo {
        backend_version: get_backend_version(),
        build_time: chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
    }
}
