use std::net::TcpStream;
use std::process::Stdio;
use std::time::Duration as StdDuration;

use sqlx::{Pool, Postgres};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::logger;
use crate::state_machine;
use uuid::Uuid;

/// Resolve vLLM Docker image with hierarchy (same logic as in services.rs)
async fn resolve_vllm_image_impl(
    db: &sqlx::Pool<sqlx::Postgres>,
    instance_type_id: Option<Uuid>,
    provider_id: Option<Uuid>,
    instance_type_code: &str,
) -> String {
    // 1. Check instance_types.allocation_params.vllm_image (instance-type specific)
    if let Some(type_id) = instance_type_id {
        if let Ok(Some(vllm_image)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT NULLIF(TRIM(allocation_params->>'vllm_image'), '') FROM instance_types WHERE id = $1"
        )
        .bind(type_id)
        .fetch_optional(db)
        .await
        {
            if let Some(img) = vllm_image {
                if !img.trim().is_empty() {
                    eprintln!("✅ [resolve_vllm_image] Using instance-type specific image: {} (from allocation_params)", img);
                    return img;
                }
            }
        }
    }

    // 2. Check provider_settings.WORKER_VLLM_IMAGE_<INSTANCE_TYPE_CODE> (per instance type)
    if let Some(pid) = provider_id {
        let setting_key = format!(
            "WORKER_VLLM_IMAGE_{}",
            instance_type_code.replace("-", "_").to_uppercase()
        );
        if let Ok(Some(img)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT NULLIF(TRIM(value_text), '') FROM provider_settings WHERE provider_id = $1 AND key = $2"
        )
        .bind(pid)
        .bind(&setting_key)
        .fetch_optional(db)
        .await
        {
            if let Some(img) = img {
                if !img.trim().is_empty() {
                    eprintln!("✅ [resolve_vllm_image] Using provider setting for {}: {}", instance_type_code, img);
                    return img;
                }
            }
        }
    }

    // 3. Check provider_settings.WORKER_VLLM_IMAGE (provider default)
    if let Some(pid) = provider_id {
        if let Ok(Some(img)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT NULLIF(TRIM(value_text), '') FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_IMAGE'"
        )
        .bind(pid)
        .fetch_optional(db)
        .await
        {
            if let Some(img) = img {
                if !img.trim().is_empty() {
                    eprintln!("✅ [resolve_vllm_image] Using provider default image: {}", img);
                    return img;
                }
            }
        }
    }

    // 4. Check environment variable
    if let Ok(img) = std::env::var("WORKER_VLLM_IMAGE") {
        if !img.trim().is_empty() {
            eprintln!(
                "✅ [resolve_vllm_image] Using env var WORKER_VLLM_IMAGE: {}",
                img
            );
            return img;
        }
    }

    // 5. Hardcoded default (stable version, not "latest")
    // Default: v0.13.0 is a stable version available on Docker Hub
    // Note: For P100 (RENDER-S), this may need to be a version compiled with sm_60 support
    // For L4/L40S, this version should work fine
    let default_image = "vllm/vllm-openai:v0.13.0".to_string();
    eprintln!("ℹ️ [resolve_vllm_image] Using hardcoded default: {} (consider configuring instance_types.allocation_params.vllm_image)", default_image);
    default_image
}

fn tail_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    // Keep last max_chars characters (best effort for UTF-8).
    s.chars()
        .rev()
        .take(max_chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn extract_phases(stdout: &str) -> Vec<String> {
    // Markers are emitted as: "::phase::<name>"
    let mut phases = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("::phase::") {
            let name = rest.trim();
            if !name.is_empty() {
                phases.push(name.to_string());
            }
        }
    }
    phases
}

fn worker_control_plane_url() -> String {
    // Priority:
    // 1) WORKER_CONTROL_PLANE_URL (direct)
    // 2) WORKER_CONTROL_PLANE_URL_FILE (read file contents)
    // 3) empty
    let direct = std::env::var("WORKER_CONTROL_PLANE_URL").unwrap_or_default();
    let direct = direct.trim().trim_end_matches('/').to_string();
    if !direct.is_empty() {
        return direct;
    }
    if let Ok(path) = std::env::var("WORKER_CONTROL_PLANE_URL_FILE") {
        let p = path.trim();
        if !p.is_empty() {
            if let Ok(contents) = std::fs::read_to_string(p) {
                let v = contents.trim().trim_end_matches('/');
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    String::new()
}

fn worker_hf_token() -> String {
    // Priority:
    // 1) WORKER_HF_TOKEN (direct)
    // 2) WORKER_HF_TOKEN_FILE (read file contents)
    // 3) empty
    let direct = std::env::var("WORKER_HF_TOKEN")
        .or_else(|_| std::env::var("HUGGINGFACE_TOKEN"))
        .or_else(|_| std::env::var("HUGGING_FACE_HUB_TOKEN"))
        .or_else(|_| std::env::var("HUGGINGFACE_HUB_TOKEN"))
        .or_else(|_| std::env::var("HF_TOKEN"))
        .unwrap_or_default();
    let direct = direct.trim().to_string();
    if !direct.is_empty() {
        return direct;
    }
    if let Ok(path) = std::env::var("WORKER_HF_TOKEN_FILE") {
        let p = path.trim();
        if !p.is_empty() {
            if let Ok(contents) = std::fs::read_to_string(p) {
                let v = contents.trim();
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    String::new()
}

async fn check_instance_readyz_http(ip: &str, port: u16) -> bool {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let url = format!("http://{}:{}/readyz", clean_ip, port);
    // Use short timeout to avoid stalling the job loop
    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(2))
        .timeout(StdDuration::from_secs(3))
        .build();
    let Ok(client) = client else {
        return false;
    };
    match client.get(url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Check agent info endpoint (/info) to verify version and checksum
async fn check_agent_info(ip: &str, port: u16) -> Result<serde_json::Value, String> {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let url = format!("http://{}:{}/info", clean_ip, port);
    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(2))
        .timeout(StdDuration::from_secs(3))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    match client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                resp.json::<serde_json::Value>()
                    .await
                    .map_err(|e| format!("Failed to parse JSON: {}", e))
            } else {
                Err(format!(
                    "HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ))
            }
        }
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

pub(crate) async fn check_vllm_http_models(
    ip: &str,
    port: u16,
) -> (bool, Vec<String>, i32, Option<String>) {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let url = format!("http://{}:{}/v1/models", clean_ip, port);
    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(2))
        .timeout(StdDuration::from_secs(4))
        .build();
    let Ok(client) = client else {
        return (
            false,
            Vec::new(),
            0,
            Some("client_build_failed".to_string()),
        );
    };
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            let ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            return (false, Vec::new(), ms, Some(format!("request_error: {}", e)));
        }
    };
    let ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
    if !resp.status().is_success() {
        return (
            false,
            Vec::new(),
            ms,
            Some(format!("status={}", resp.status())),
        );
    }
    let v: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return (false, Vec::new(), ms, Some(format!("json_error: {}", e))),
    };
    let mut ids = Vec::new();
    if let Some(arr) = v.get("data").and_then(|x| x.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|x| x.as_str()) {
                ids.push(id.to_string());
            }
        }
    }
    (true, ids, ms, None)
}

async fn check_vllm_warmup_http(
    ip: &str,
    port: u16,
    model_id: &str,
) -> (bool, i32, Option<String>) {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let url = format!("http://{}:{}/v1/chat/completions", clean_ip, port);
    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(2))
        .timeout(StdDuration::from_secs(8))
        .build();
    let Ok(client) = client else {
        return (false, 0, Some("client_build_failed".to_string()));
    };
    let payload = serde_json::json!({
        "model": model_id,
        "messages": [{"role":"user","content":"ping"}],
        "max_tokens": 1,
        "temperature": 0,
        "stream": false
    });
    let resp = match client.post(&url).json(&payload).send().await {
        Ok(r) => r,
        Err(e) => {
            let ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            return (false, ms, Some(format!("request_error: {}", e)));
        }
    };
    let ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
    if !resp.status().is_success() {
        return (false, ms, Some(format!("status={}", resp.status())));
    }
    // Best-effort parse to ensure it's valid JSON.
    let _v: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return (false, ms, Some(format!("json_error: {}", e))),
    };
    (true, ms, None)
}

/// Check instance health by testing SSH port connectivity.
async fn check_instance_ssh(ip: &str) -> bool {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let addr = format!("{}:22", clean_ip);

    tokio::task::spawn_blocking(move || {
        let socket_addr = match addr.parse() {
            Ok(a) => a,
            Err(_) => return false,
        };
        TcpStream::connect_timeout(&socket_addr, StdDuration::from_secs(3)).is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Check container health via SSH: returns (vllm_running, agent_running, vllm_exit_code, agent_exit_code)
/// Returns None if SSH check fails.
async fn check_containers_via_ssh(ip: &str) -> Option<(bool, bool, Option<i32>, Option<i32>)> {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let ssh_key_path =
        std::env::var("SSH_KEY_PATH").unwrap_or_else(|_| "/app/.ssh/llm-studio-key".to_string());
    let ssh_user = std::env::var("SSH_USER").unwrap_or_else(|_| "root".to_string());
    let target = format!("{}@{}", ssh_user, clean_ip);

    // Check both vLLM and agent containers in one SSH command
    let check_script = r#"
docker inspect -f '{{.State.Running}} {{.State.ExitCode}}' vllm 2>/dev/null || echo "false -1"
docker inspect -f '{{.State.Running}} {{.State.ExitCode}}' inventiv-agent 2>/dev/null || echo "false -1"
"#;

    let mut child = match Command::new("ssh")
        .arg("-i")
        .arg(&ssh_key_path)
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg(&target)
        .arg("bash -s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(check_script.as_bytes()).await;
    }

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(_) => return None,
    };

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    let (vllm_running, vllm_exit_code) = if lines.len() > 0 {
        let parts: Vec<&str> = lines[0].trim().split_whitespace().collect();
        if parts.len() >= 2 {
            let running = parts[0] == "true";
            let exit_code = parts[1].parse::<i32>().ok();
            (running, exit_code)
        } else {
            (false, Some(-1))
        }
    } else {
        (false, Some(-1))
    };

    let (agent_running, agent_exit_code) = if lines.len() > 1 {
        let parts: Vec<&str> = lines[1].trim().split_whitespace().collect();
        if parts.len() >= 2 {
            let running = parts[0] == "true";
            let exit_code = parts[1].parse::<i32>().ok();
            (running, exit_code)
        } else {
            (false, Some(-1))
        }
    } else {
        (false, Some(-1))
    };

    Some((vllm_running, agent_running, vllm_exit_code, agent_exit_code))
}

/// Fetch worker logs from the /logs endpoint
/// Returns None if the request fails
async fn fetch_worker_logs(ip: &str, port: u16) -> Option<WorkerLogs> {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let url = format!("http://{}:{}/logs?tail=100", clean_ip, port);
    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(2))
        .timeout(StdDuration::from_secs(5))
        .build();
    let Ok(client) = client else {
        return None;
    };
    match client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>().await {
                    Ok(data) => {
                        let events = data
                            .get("events")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let total_events = data
                            .get("total_events")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        Some(WorkerLogs {
                            total_events,
                            events,
                        })
                    }
                    Err(_) => None,
                }
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

#[derive(Debug)]
struct WorkerLogs {
    total_events: usize,
    events: Vec<serde_json::Value>,
}

async fn provider_setting_i64(
    db: &Pool<Postgres>,
    provider_id: uuid::Uuid,
    key: &str,
) -> Option<i64> {
    sqlx::query_scalar(
        r#"
        SELECT value_int
        FROM provider_settings
        WHERE provider_id = $1 AND key = $2
        "#,
    )
    .bind(provider_id)
    .bind(key)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

async fn provider_setting_bool(
    db: &Pool<Postgres>,
    provider_id: uuid::Uuid,
    key: &str,
) -> Option<bool> {
    sqlx::query_scalar(
        r#"
        SELECT value_bool
        FROM provider_settings
        WHERE provider_id = $1 AND key = $2
        "#,
    )
    .bind(provider_id)
    .bind(key)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

async fn provider_setting_text(
    db: &Pool<Postgres>,
    provider_id: uuid::Uuid,
    key: &str,
) -> Option<String> {
    sqlx::query_scalar(
        r#"
        SELECT value_text
        FROM provider_settings
        WHERE provider_id = $1 AND key = $2
        "#,
    )
    .bind(provider_id)
    .bind(key)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|s: String| {
        let t = s.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    })
}

fn sh_escape_single(s: &str) -> String {
    // Safe single-quote escape for bash: wrap with '...' and escape internal quotes.
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

async fn maybe_trigger_worker_install_over_ssh(
    db: &Pool<Postgres>,
    instance_id: uuid::Uuid,
    ip: &str,
    force: bool,
    correlation_id: Option<String>,
) {
    let auto_install = std::env::var("WORKER_AUTO_INSTALL")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    if !auto_install && !force {
        return;
    }

    let cp_url = worker_control_plane_url();
    if cp_url.is_empty() {
        return;
    }

    // Global token for early bringup (API also accepts it).
    let worker_auth_token = std::env::var("WORKER_AUTH_TOKEN").unwrap_or_default();
    let worker_hf_token = worker_hf_token();

    let provider_id: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT provider_id FROM instances WHERE id = $1")
            .bind(instance_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

    let model_id_from_db: Option<String> = sqlx::query_scalar(
        r#"
        SELECT m.model_id
        FROM instances i
        LEFT JOIN models m ON m.id = i.model_id
        WHERE i.id = $1
        "#,
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|s: String| {
        let t = s.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });

    let model_id = model_id_from_db
        .or_else(|| {
            std::env::var("WORKER_MODEL_ID")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| "Qwen/Qwen2.5-0.5B-Instruct".to_string());
    // Resolve vLLM image with hierarchy (same as in services.rs)
    let instance_type_id: Option<Uuid> =
        sqlx::query_scalar("SELECT instance_type_id FROM instances WHERE id = $1")
            .bind(instance_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

    let instance_type_code: String = sqlx::query_scalar(
        "SELECT code FROM instance_types WHERE id = (SELECT instance_type_id FROM instances WHERE id = $1)"
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "unknown".to_string());

    // Use the same resolution logic as in services.rs
    let vllm_image =
        resolve_vllm_image_impl(db, instance_type_id, provider_id, &instance_type_code).await;
    let vllm_mode = if let Some(pid) = provider_id {
        provider_setting_text(db, pid, "WORKER_VLLM_MODE")
            .await
            .or_else(|| {
                std::env::var("WORKER_VLLM_MODE")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "mono".to_string())
    } else {
        std::env::var("WORKER_VLLM_MODE")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "mono".to_string())
    }; // mono | multi
    let agent_url = std::env::var("WORKER_AGENT_SOURCE_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://raw.githubusercontent.com/Inventiv-IT-for-AI/inventiv-agents/main/inventiv-worker/agent.py".to_string());

    // Optional: expected SHA256 checksum for agent.py (for integrity verification)
    let agent_expected_sha256 = std::env::var("WORKER_AGENT_SHA256")
        .ok()
        .filter(|s| !s.trim().is_empty());

    let worker_health_port: u16 = if let Some(pid) = provider_id {
        provider_setting_i64(db, pid, "WORKER_HEALTH_PORT")
            .await
            .and_then(|v| u16::try_from(v).ok())
            .or_else(|| {
                std::env::var("WORKER_HEALTH_PORT")
                    .ok()
                    .and_then(|s| s.parse::<u16>().ok())
            })
            .unwrap_or(8080)
    } else {
        std::env::var("WORKER_HEALTH_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8080)
    };
    let worker_vllm_port: u16 = if let Some(pid) = provider_id {
        provider_setting_i64(db, pid, "WORKER_VLLM_PORT")
            .await
            .and_then(|v| u16::try_from(v).ok())
            .or_else(|| {
                std::env::var("WORKER_VLLM_PORT")
                    .ok()
                    .and_then(|s| s.parse::<u16>().ok())
            })
            .unwrap_or(8000)
    } else {
        std::env::var("WORKER_VLLM_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(8000)
    };

    let ssh_user = std::env::var("WORKER_SSH_USER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "root".to_string());
    let ssh_key_path = std::env::var("WORKER_SSH_PRIVATE_KEY_FILE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "/app/.ssh/llm-studio-key".to_string());

    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let target = format!("{}@{}", ssh_user, clean_ip);

    // De-dupe / backoff: don't re-run SSH bootstrap in a tight loop.
    // If the last install succeeded recently, it's more likely we should just wait for model load/readiness.
    #[derive(sqlx::FromRow)]
    struct LastSshInstall {
        id: uuid::Uuid,
        status: String,
        created_at: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
        completed_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
        last_phase: Option<String>,
    }

    let last: Option<LastSshInstall> = sqlx::query_as(
        r#"
        SELECT
          id,
          status::text as status,
          created_at,
          completed_at,
          (metadata->>'last_phase') as last_phase
        FROM action_logs
        WHERE instance_id = $1
          AND action_type = 'WORKER_SSH_INSTALL'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(instance_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if let Some(last) = last {
        let now = sqlx::types::chrono::Utc::now();
        let age_s = (now - last.created_at).num_seconds();
        let status = last.status.trim().to_ascii_lowercase();
        let last_phase = last.last_phase.unwrap_or_default();

        // If an install is still in progress, check if it has exceeded timeout
        if status == "in_progress" {
            // If forced (manual reinstall), cancel the in-progress installation and start a new one
            if force {
                println!(
                    "⚠️ WORKER_SSH_INSTALL for instance {} is in_progress but force=true (reinstall requested) - marking as cancelled and starting new installation",
                    instance_id
                );
                let _ = logger::log_event_complete_with_metadata(
                    db,
                    last.id,
                    "failed",
                    (age_s * 1000) as i32,
                    Some("Cancelled due to forced reinstall"),
                    Some(serde_json::json!({
                        "age_s": age_s,
                        "cancelled_by_reinstall": true
                    })),
                )
                .await;
                // Don't return - allow new installation to proceed below
            } else {
                // Get SSH timeout setting (same logic as below)
                let ssh_timeout_s: u64 = if let Some(pid) = provider_id {
                    provider_setting_i64(db, pid, "WORKER_SSH_BOOTSTRAP_TIMEOUT_S")
                        .await
                        .and_then(|v| u64::try_from(v).ok())
                        .filter(|v| *v > 0)
                        .or_else(|| {
                            std::env::var("WORKER_SSH_BOOTSTRAP_TIMEOUT_S")
                                .ok()
                                .and_then(|v| v.trim().parse::<u64>().ok())
                                .filter(|v| *v > 0)
                        })
                        .unwrap_or(900)
                } else {
                    std::env::var("WORKER_SSH_BOOTSTRAP_TIMEOUT_S")
                        .ok()
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        .filter(|v| *v > 0)
                        .unwrap_or(900)
                };

                // If SSH install has exceeded timeout, mark it as failed
                // Add a small buffer (60s) to account for the timeout check happening slightly after the actual timeout
                if age_s > (ssh_timeout_s as i64 + 60) {
                    println!(
                        "⚠️ WORKER_SSH_INSTALL for instance {} has been in_progress for {}s (timeout: {}s), marking as failed",
                        instance_id, age_s, ssh_timeout_s
                    );
                    let _ = logger::log_event_complete_with_metadata(
                        db,
                        last.id,
                        "failed",
                        (age_s * 1000) as i32,
                        Some(&format!(
                            "SSH install exceeded timeout ({}s > {}s)",
                            age_s, ssh_timeout_s
                        )),
                        Some(serde_json::json!({
                            "ssh_timeout_s": ssh_timeout_s,
                            "age_s": age_s,
                            "ssh_timed_out": true
                        })),
                    )
                    .await;
                    // Don't return - allow retry logic below to handle it
                } else {
                    // Still within timeout, don't start another one
                    println!(
                        "ℹ️ WORKER_SSH_INSTALL for instance {} is still in_progress ({}s / {}s timeout) - waiting",
                        instance_id, age_s, ssh_timeout_s
                    );
                    return;
                }
            }
        }

        // If forced, allow immediate re-run even after a success/failure (manual operator action).
        if force {
            // But if the last one was very recent, still apply a small guard to avoid accidental double-click loops.
            if age_s < 15 {
                return;
            }
        } else {
            // If we just completed a successful install, wait longer (model load can take a while).
            // This avoids repeatedly restarting vLLM/agent and never reaching READY.
            // However, if health checks are still failing after 10 minutes, something is wrong
            // and we should retry the installation (containers may have crashed or not started properly).
            // BUT: if worker is sending heartbeats, it's alive and we should wait longer (model loading).
            if status == "success" && last_phase == "done" {
                // Check if worker is sending heartbeats
                let has_recent_heartbeat: Option<i64> = sqlx::query_scalar(
                    r#"
                    SELECT EXTRACT(EPOCH FROM (NOW() - worker_last_heartbeat))::bigint
                    FROM instances
                    WHERE id = $1
                      AND worker_last_heartbeat > NOW() - INTERVAL '5 minutes'
                    "#,
                )
                .bind(instance_id)
                .fetch_optional(db)
                .await
                .unwrap_or(None);

                if let Some(heartbeat_age_s) = has_recent_heartbeat {
                    // Worker is sending heartbeats - it's alive, just not ready yet
                    // Wait longer for model loading (up to 30 minutes)
                    if age_s < 30 * 60 {
                        println!(
                            "ℹ️ Instance {} SSH install succeeded {}s ago, worker sending heartbeats (last {}s ago) - waiting for model loading (up to 30min)",
                            instance_id, age_s, heartbeat_age_s
                        );
                        return;
                    } else {
                        println!(
                            "⚠️ Instance {} SSH install succeeded {}s ago, worker sending heartbeats but still not ready after 30min - may need investigation",
                            instance_id, age_s
                        );
                        // Continue to allow retry, but this is unusual
                    }
                } else {
                    // No heartbeats - normal timeout logic
                    if age_s < 10 * 60 {
                        // Within 10 minutes: normal wait for model loading
                        return;
                    } else {
                        // After 10 minutes: if health checks still fail, retry installation
                        // This handles cases where containers crashed or didn't start properly
                        println!(
                            "⚠️ Instance {} SSH install succeeded {}s ago but health checks still failing and no heartbeats, retrying installation",
                            instance_id, age_s
                        );
                        // Continue to trigger reinstall below
                    }
                }
            }

            // If it failed recently, apply a short backoff before retrying.
            if status == "failed" && age_s < 5 * 60 {
                return;
            }
        }
    }

    let mut meta = serde_json::json!({
        "ip": clean_ip,
        "ssh_user": ssh_user,
        "ssh_key_path": ssh_key_path,
        "control_plane_url": cp_url,
        "model_id": model_id,
        "vllm_image": vllm_image,
        "vllm_mode": vllm_mode,
        "agent_url": agent_url,
        "agent_expected_sha256": agent_expected_sha256.as_deref(),
        "worker_health_port": worker_health_port,
        "worker_vllm_port": worker_vllm_port,
        "has_hf_token": !worker_hf_token.trim().is_empty(),
        "force": force,
        "correlation_id": correlation_id
    });

    let log_id = logger::log_event_with_metadata(
        db,
        "WORKER_SSH_INSTALL",
        "in_progress",
        instance_id,
        None,
        Some(meta.clone()),
    )
    .await
    .ok();

    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

INSTANCE_ID={instance_id}
CONTROL_PLANE_URL={cp_url}
MODEL_ID={model_id}
VLLM_IMAGE={vllm_image}
VLLM_MODE={vllm_mode}
AGENT_URL={agent_url}
AGENT_EXPECTED_SHA256={agent_expected_sha256_str}
WORKER_AUTH_TOKEN={worker_auth_token}
WORKER_HF_TOKEN={worker_hf_token}
WORKER_HEALTH_PORT={worker_health_port}
WORKER_VLLM_PORT={worker_vllm_port}

echo "::phase::start"
echo "[inventiv-worker] ssh bootstrap starting"

echo "::phase::mount_data_volume"
# Storage Strategy (Official Scaleway Recommendations):
# - L4: Block Storage only (mounted at /mnt/inventiv-data -> /opt/inventiv-worker)
# - L40S/H100: Scratch Storage (/scratch, temporary) + Block Storage (/mnt/inventiv-data, persistent)
# - Block Storage: Used for persistent data (models, results, checkpoints)
# - Scratch Storage: Used for temporary data (cache, intermediate results) - auto-mounted by Scaleway
#
# Mount the attached Block Storage volume (Scaleway SBS) and use it for Docker + HF cache.
# The root disk on GPU images is often too small for pulling vLLM images.
DATA_MNT="/mnt/inventiv-data"
mkdir -p "$DATA_MNT"

root_src="$(findmnt -n -o SOURCE / || true)"
root_base=""
if [[ "$root_src" =~ ^/dev/ ]]; then
  # strip partition suffix: /dev/sda2 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1
  root_base="/dev/$(basename "$root_src" | sed -E 's/p?[0-9]+$//')"
fi

# Function to force SCSI rescan (helps detect newly attached Block Storage volumes)
force_scsi_rescan() {{
  echo "[inventiv-worker] Forcing SCSI rescan to detect Block Storage volumes..."
  for host in /sys/class/scsi_host/host*/scan; do
    if [[ -f "$host" ]]; then
      echo "- - -" > "$host" 2>/dev/null || true
    fi
  done
  # Wait a moment for kernel to process
  sleep 2
  # Trigger udev events
  udevadm settle --timeout=5 || true
}}

# Function to find Block Storage candidate with retries
find_block_storage_candidate() {{
  local max_attempts=10
  local attempt=1
  
  while [[ $attempt -le $max_attempts ]]; do
    echo "[inventiv-worker] Attempt $attempt/$max_attempts: Looking for Block Storage volume..."
    
    # Force rescan on first few attempts
    if [[ $attempt -le 3 ]]; then
      force_scsi_rescan
    fi
    
    # Method 1: Check lsblk for unmounted disks
    local candidate=""
    while read -r name type size; do
      [[ "$type" == "disk" ]] || continue
      dev="/dev/$name"
      [[ -n "$root_base" && "$dev" == "$root_base" ]] && continue
      # ignore disks that are already mounted
      if lsblk -n -o MOUNTPOINT "$dev" 2>/dev/null | grep -q '/'; then
        continue
      fi
      # Prefer larger disks (Block Storage is typically > 20GB)
      local size_bytes=$(lsblk -b -n -o SIZE "$dev" 2>/dev/null | head -1 || echo "0")
      if [[ $size_bytes -gt 20000000000 ]]; then  # > 20GB
        candidate="$dev"
        echo "[inventiv-worker] Found candidate Block Storage: $candidate (size: $(numfmt --to=iec-i --suffix=B $size_bytes 2>/dev/null || echo 'unknown'))"
        break
      fi
    done < <(lsblk -ndo NAME,TYPE,SIZE 2>/dev/null || true)
    
    if [[ -n "$candidate" ]]; then
      echo "$candidate"
      return 0
    fi
    
    # Method 2: Check /dev/disk/by-id/ for Scaleway volumes (if available)
    if [[ -d /dev/disk/by-id ]]; then
      for link in /dev/disk/by-id/scw-*; do
        if [[ -L "$link" ]]; then
          local real_dev=$(readlink -f "$link" 2>/dev/null || true)
          if [[ -n "$real_dev" && "$real_dev" != "$root_src" ]]; then
            # Check if it's already mounted
            if ! lsblk -n -o MOUNTPOINT "$real_dev" 2>/dev/null | grep -q '/'; then
              candidate="$real_dev"
              echo "[inventiv-worker] Found candidate Block Storage via by-id: $candidate"
              echo "$candidate"
              return 0
            fi
          fi
        fi
      done
    fi
    
    if [[ $attempt -lt $max_attempts ]]; then
      echo "[inventiv-worker] Block Storage not found yet, waiting 3 seconds before retry..."
      sleep 3
    fi
    
    attempt=$((attempt + 1))
  done
  
  return 1
}}

candidate=""
if candidate=$(find_block_storage_candidate); then
  echo "[inventiv-worker] ✅ Block Storage candidate found: $candidate"
  
  if ! blkid "$candidate" >/dev/null 2>&1; then
    echo "[inventiv-worker] Formatting Block Storage volume $candidate with ext4..."
    mkfs.ext4 -F -L inventiv-data "$candidate"
  fi
  
  uuid="$(blkid -s UUID -o value "$candidate" || true)"
  if [[ -n "$uuid" ]]; then
    echo "[inventiv-worker] Block Storage UUID: $uuid"
    if ! grep -q "$uuid" /etc/fstab 2>/dev/null; then
      echo "UUID=$uuid $DATA_MNT ext4 defaults,nofail 0 2" >> /etc/fstab
      echo "[inventiv-worker] Added Block Storage to /etc/fstab"
    fi
  fi
  
  mount -a || true
  if ! mountpoint -q "$DATA_MNT"; then
    echo "[inventiv-worker] Mounting Block Storage $candidate to $DATA_MNT..."
    mount "$candidate" "$DATA_MNT" || {{
      echo "[inventiv-worker] ❌ Failed to mount $candidate to $DATA_MNT" >&2
      candidate=""
    }}
  fi
else
  echo "[inventiv-worker] ⚠️ WARNING: Block Storage volume not found after 10 attempts" >&2
fi

if mountpoint -q "$DATA_MNT"; then
  echo "[inventiv-worker] Block Storage mounted at $DATA_MNT"
  df -h "$DATA_MNT" || true
  
  mkdir -p "$DATA_MNT/docker" "$DATA_MNT/worker"
  # Put worker files on the data disk
  if [[ -d /opt/inventiv-worker && ! -L /opt/inventiv-worker ]]; then
    rm -rf /opt/inventiv-worker
  fi
  ln -sfn "$DATA_MNT/worker" /opt/inventiv-worker

  # Put Docker storage on the data disk (avoid huge image pulls on root)
  echo "[inventiv-worker] Configuring Docker to use Block Storage at $DATA_MNT/docker"
  systemctl stop docker >/dev/null 2>&1 || true
  
  # Wait for Docker to fully stop
  sleep 2
  
  if [[ -d /var/lib/docker && ! -L /var/lib/docker ]]; then
    # If it already has content, keep a backup rather than deleting.
    if [[ "$(ls -A /var/lib/docker 2>/dev/null | wc -l | tr -d ' ')" != "0" ]]; then
      echo "[inventiv-worker] Moving existing Docker data to backup"
      mv /var/lib/docker "/var/lib/docker.bak.$(date +%s)"
    else
      rm -rf /var/lib/docker
    fi
  fi
  
  # Verify symlink creation
  ln -sfn "$DATA_MNT/docker" /var/lib/docker
  if [[ -L /var/lib/docker ]]; then
    echo "[inventiv-worker] ✅ Docker symlink created: /var/lib/docker -> $(readlink -f /var/lib/docker)"
  else
    echo "[inventiv-worker] ❌ ERROR: Failed to create Docker symlink" >&2
    exit 1
  fi
  
  systemctl start docker >/dev/null 2>&1 || true
  
  # Verify Docker is using the Block Storage
  sleep 2
  if docker info >/dev/null 2>&1; then
    DOCKER_ROOT=$(docker info 2>/dev/null | grep -i "docker root dir" | awk '{{print $4}}' || echo "unknown")
    echo "[inventiv-worker] Docker root directory: $DOCKER_ROOT"
    df -h "$DATA_MNT" || true
  fi
else
  echo "[inventiv-worker] ⚠️ WARNING: Block Storage NOT mounted at $DATA_MNT - Docker will use root disk (may run out of space)" >&2
  echo "[inventiv-worker] Available disks:" >&2
  lsblk -o NAME,TYPE,SIZE,MOUNTPOINT || true
  echo "[inventiv-worker] Mounted filesystems:" >&2
  df -h || true
fi

echo "::phase::docker_install"
if ! command -v docker >/dev/null 2>&1; then
  apt-get update -y
  apt-get install -y ca-certificates curl gnupg
  curl -fsSL https://get.docker.com | sh
fi
echo "::phase::docker_start"
systemctl enable --now docker

if command -v nvidia-smi >/dev/null 2>&1; then
  echo "::phase::nvidia_toolkit"
  echo "[inventiv-worker] installing nvidia-container-toolkit"
  set +e
  . /etc/os-release
  distribution="${{ID}}${{VERSION_ID}}"
  curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | gpg --batch --yes --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg || true
  curl -fsSL "https://nvidia.github.io/libnvidia-container/${{distribution}}/libnvidia-container.list" \
    | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \
    > /etc/apt/sources.list.d/nvidia-container-toolkit.list || true
  apt-get update -y || true
  if ! apt-get install -y nvidia-container-toolkit; then
    echo "[inventiv-worker] ERROR: nvidia-container-toolkit installation failed" >&2
    set -e
    exit 1
  fi
  if ! nvidia-ctk runtime configure --runtime=docker; then
    echo "[inventiv-worker] ERROR: nvidia-ctk runtime configure failed" >&2
    set -e
    exit 1
  fi
  systemctl restart docker || true
  set -e
  
  echo "[inventiv-worker] verifying GPU access"
  if ! docker run --rm --gpus all nvidia/cuda:11.8.0-base-ubuntu22.04 nvidia-smi >/dev/null 2>&1; then
    echo "[inventiv-worker] ERROR: Docker cannot access GPUs. nvidia-container-toolkit may not be configured correctly." >&2
    echo "[inventiv-worker] Diagnostics:" >&2
    docker info | grep -i runtime >&2 || true
    nvidia-ctk config --dry-run --insecure --config /etc/docker/daemon.json >&2 || true
    exit 1
  fi
  echo "[inventiv-worker] GPU access verified"
else
  echo "[inventiv-worker] nvidia-smi not found; skipping nvidia-container-toolkit"
fi

echo "::phase::agent_download"
mkdir -p /opt/inventiv-worker
curl -fsSL "$AGENT_URL" -o /opt/inventiv-worker/agent.py

# Verify agent.py integrity if expected SHA256 is provided
if [[ -n "$AGENT_EXPECTED_SHA256" ]]; then
  echo "[inventiv-worker] Verifying agent.py checksum..."
  ACTUAL_SHA256="$(sha256sum /opt/inventiv-worker/agent.py 2>/dev/null | cut -d' ' -f1 || shasum -a 256 /opt/inventiv-worker/agent.py 2>/dev/null | cut -d' ' -f1)"
  if [[ "$ACTUAL_SHA256" != "$AGENT_EXPECTED_SHA256" ]]; then
    echo "[inventiv-worker] ERROR: agent.py checksum mismatch!" >&2
    echo "[inventiv-worker] Expected: $AGENT_EXPECTED_SHA256" >&2
    echo "[inventiv-worker] Actual:   $ACTUAL_SHA256" >&2
    echo "[inventiv-worker] The downloaded agent.py may be outdated or corrupted." >&2
    echo "[inventiv-worker] Please ensure WORKER_AGENT_SOURCE_URL points to the correct version or update WORKER_AGENT_SHA256." >&2
    exit 1
  fi
  echo "[inventiv-worker] agent.py checksum verified: $ACTUAL_SHA256"
else
  echo "[inventiv-worker] WARNING: No WORKER_AGENT_SHA256 provided - skipping integrity check"
  ACTUAL_SHA256="$(sha256sum /opt/inventiv-worker/agent.py 2>/dev/null | cut -d' ' -f1 || shasum -a 256 /opt/inventiv-worker/agent.py 2>/dev/null | cut -d' ' -f1)"
  echo "[inventiv-worker] Downloaded agent.py SHA256: $ACTUAL_SHA256"
fi

# Extract and log agent version if available
AGENT_VERSION="$(grep -m1 '^AGENT_VERSION' /opt/inventiv-worker/agent.py 2>/dev/null | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo 'unknown')"
AGENT_BUILD_DATE="$(grep -m1 '^AGENT_BUILD_DATE' /opt/inventiv-worker/agent.py 2>/dev/null | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo 'unknown')"
echo "[inventiv-worker] agent.py version: $AGENT_VERSION (build: $AGENT_BUILD_DATE)"

echo "::phase::docker_pull"
docker pull "$VLLM_IMAGE"
docker pull python:3.11-slim
docker pull haproxy:2.9-alpine || true

echo "::phase::vllm_start"
docker rm -f vllm >/dev/null 2>&1 || true
docker rm -f vllm-lb >/dev/null 2>&1 || true

if [[ "$VLLM_MODE" == "multi" ]]; then
  # Multi vLLM: one replica per GPU, behind a local HAProxy on :8000.
  # Each vLLM listens on container :8000 but is published on host ports 8001.. (one per GPU).
  GPU_COUNT="$(nvidia-smi -L 2>/dev/null | wc -l | tr -d ' ' || echo 0)"
  if [[ "$GPU_COUNT" -lt 1 ]]; then
    GPU_COUNT=1
  fi
  if [[ "$GPU_COUNT" -gt 8 ]]; then
    GPU_COUNT=8
  fi

  for i in $(seq 0 $((GPU_COUNT-1))); do
    name="vllm-$i"
    host_port=$((8001+i))
    docker rm -f "$name" >/dev/null 2>&1 || true
    docker run -d --restart unless-stopped \
      --name "$name" \
      --gpus "device=$i" \
      -p "$host_port:8000" \
      -e CUDA_VISIBLE_DEVICES="$i" \
      -e HUGGING_FACE_HUB_TOKEN="$WORKER_HF_TOKEN" \
      -e HUGGINGFACE_HUB_TOKEN="$WORKER_HF_TOKEN" \
      -e HF_TOKEN="$WORKER_HF_TOKEN" \
      -e HF_HOME=/opt/inventiv-worker/hf \
      -e TRANSFORMERS_CACHE=/opt/inventiv-worker/hf \
      -v /opt/inventiv-worker:/opt/inventiv-worker \
      "$VLLM_IMAGE" \
      --host 0.0.0.0 --port 8000 \
      --model "$MODEL_ID" \
      --dtype float16
  done

  cat >/opt/inventiv-worker/haproxy.cfg <<EOF
global
  maxconn 2048
defaults
  mode http
  timeout connect 5s
  timeout client  300s
  timeout server  300s

frontend fe_vllm
  bind 0.0.0.0:8000
  default_backend be_vllm

backend be_vllm
  balance roundrobin
  # Sticky sessions (best-effort): if the client provides X-Inventiv-Session, we keep affinity.
  # If not provided, we fall back to standard round-robin.
  stick-table type string len 128 size 200k expire 30m
  acl has_sid req.hdr(X-Inventiv-Session) -m found
  stick on req.hdr(X-Inventiv-Session) if has_sid
  option httpchk GET /v1/models
  http-check expect status 200
EOF
  for i in $(seq 0 $((GPU_COUNT-1))); do
    host_port=$((8001+i))
    echo "  server s$i 127.0.0.1:$host_port check inter 2s fall 3 rise 2" >>/opt/inventiv-worker/haproxy.cfg
  done

  docker run -d --restart unless-stopped \
    --name vllm-lb \
    --network host \
    -v /opt/inventiv-worker/haproxy.cfg:/usr/local/etc/haproxy/haproxy.cfg:ro \
    haproxy:2.9-alpine
else
  # Mono vLLM: one server uses all GPUs (if available).
  if command -v nvidia-smi >/dev/null 2>&1; then
    docker run -d --restart unless-stopped \
      --name vllm \
      --gpus all \
      -p 8000:8000 \
      -e HUGGING_FACE_HUB_TOKEN="$WORKER_HF_TOKEN" \
      -e HUGGINGFACE_HUB_TOKEN="$WORKER_HF_TOKEN" \
      -e HF_TOKEN="$WORKER_HF_TOKEN" \
      -e HF_HOME=/opt/inventiv-worker/hf \
      -e TRANSFORMERS_CACHE=/opt/inventiv-worker/hf \
      -v /opt/inventiv-worker:/opt/inventiv-worker \
      "$VLLM_IMAGE" \
      --host 0.0.0.0 --port 8000 \
      --model "$MODEL_ID" \
      --dtype float16
  else
    echo "[inventiv-worker] WARNING: nvidia-smi not found, running vLLM in CPU mode (slow)" >&2
    docker run -d --restart unless-stopped \
      --name vllm \
      -p 8000:8000 \
      -e HUGGING_FACE_HUB_TOKEN="$WORKER_HF_TOKEN" \
      -e HUGGINGFACE_HUB_TOKEN="$WORKER_HF_TOKEN" \
      -e HF_TOKEN="$WORKER_HF_TOKEN" \
      -e HF_HOME=/opt/inventiv-worker/hf \
      -e TRANSFORMERS_CACHE=/opt/inventiv-worker/hf \
      -v /opt/inventiv-worker:/opt/inventiv-worker \
      "$VLLM_IMAGE" \
      --host 0.0.0.0 --port 8000 \
      --model "$MODEL_ID" \
      --dtype float16 \
      --device cpu
  fi
fi

# Ensure container is actually running
sleep 2
# NOTE: this script is generated via Rust `format!`, so Go-template braces must be double-escaped.
if [[ "$VLLM_MODE" == "multi" ]]; then
  vllm_state="$(docker inspect -f '{{{{.State.Status}}}}' vllm-lb 2>/dev/null | tr -d '\r' || true)"
else
  vllm_state="$(docker inspect -f '{{{{.State.Status}}}}' vllm 2>/dev/null | tr -d '\r' || true)"
fi
if [[ "$vllm_state" != "running" ]]; then
  echo "[inventiv-worker] vllm container did not stay running. Diagnostics:" >&2
  echo "---- docker ps -a ----" >&2
  docker ps -a >&2 || true
  echo "---- docker inspect vllm ----" >&2
  if [[ "$VLLM_MODE" == "multi" ]]; then
    docker inspect vllm-lb --format 'status={{{{.State.Status}}}} exit={{{{.State.ExitCode}}}} err={{{{.State.Error}}}} oom={{{{.State.OOMKilled}}}}' >&2 || true
  else
    docker inspect vllm --format 'status={{{{.State.Status}}}} exit={{{{.State.ExitCode}}}} err={{{{.State.Error}}}} oom={{{{.State.OOMKilled}}}}' >&2 || true
  fi
  echo "---- docker logs vllm (tail) ----" >&2
  if [[ "$VLLM_MODE" == "multi" ]]; then
    docker logs --tail 200 vllm-lb >&2 || true
  else
    docker logs --tail 200 vllm >&2 || true
  fi
  exit 1
fi

echo "::phase::agent_start"
docker rm -f inventiv-agent >/dev/null 2>&1 || true
docker run -d --restart unless-stopped \
  --name inventiv-agent \
  --network host \
  -e CONTROL_PLANE_URL="$CONTROL_PLANE_URL" \
  -e INSTANCE_ID="$INSTANCE_ID" \
  -e MODEL_ID="$MODEL_ID" \
  -e VLLM_BASE_URL="http://127.0.0.1:8000" \
  -e WORKER_HEALTH_PORT="$WORKER_HEALTH_PORT" \
  -e WORKER_VLLM_PORT="$WORKER_VLLM_PORT" \
  -e WORKER_HEARTBEAT_INTERVAL_S=10 \
  -e WORKER_AUTH_TOKEN="$WORKER_AUTH_TOKEN" \
  -v /opt/inventiv-worker/agent.py:/app/agent.py:ro \
  python:3.11-slim \
  bash -lc "pip install --no-cache-dir requests >/dev/null && python /app/agent.py"

sleep 1
agent_state="$(docker inspect -f '{{{{.State.Status}}}}' inventiv-agent 2>/dev/null | tr -d '\r' || true)"
if [[ "$agent_state" != "running" ]]; then
  echo "[inventiv-worker] inventiv-agent container did not stay running. Diagnostics:" >&2
  echo "---- docker ps -a ----" >&2
  docker ps -a >&2 || true
  echo "---- docker inspect inventiv-agent ----" >&2
  docker inspect inventiv-agent --format 'status={{{{.State.Status}}}} exit={{{{.State.ExitCode}}}} err={{{{.State.Error}}}} oom={{{{.State.OOMKilled}}}}' >&2 || true
  echo "---- docker logs inventiv-agent (tail) ----" >&2
  docker logs --tail 200 inventiv-agent >&2 || true
  exit 1
fi

echo "::phase::done"
echo "[inventiv-worker] ssh bootstrap done"
"#,
        instance_id = sh_escape_single(&instance_id.to_string()),
        cp_url = sh_escape_single(&cp_url),
        model_id = sh_escape_single(&model_id),
        vllm_image = sh_escape_single(&vllm_image),
        vllm_mode = sh_escape_single(&vllm_mode),
        agent_url = sh_escape_single(&agent_url),
        agent_expected_sha256_str =
            sh_escape_single(&agent_expected_sha256.as_deref().unwrap_or("")),
        worker_auth_token = sh_escape_single(&worker_auth_token),
        worker_hf_token = sh_escape_single(&worker_hf_token),
    );

    let started = std::time::Instant::now();
    // Use a longer ConnectTimeout for instances that may have SSH service still starting
    // This is especially important for Scaleway instances that may take time to fully boot
    let mut child = match Command::new("ssh")
        .arg("-i")
        .arg(&ssh_key_path)
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("ConnectTimeout=30")
        .arg("-o")
        .arg("ServerAliveInterval=10")
        .arg("-o")
        .arg("ServerAliveCountMax=3")
        .arg(&target)
        .arg("bash -s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                let _ = logger::log_event_complete(
                    db,
                    lid,
                    "failed",
                    dur,
                    Some(&format!("ssh spawn failed: {}", e)),
                )
                .await;
            }
            return;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(script.as_bytes()).await;
    }

    let ssh_timeout_s: u64 = if let Some(pid) = provider_id {
        provider_setting_i64(db, pid, "WORKER_SSH_BOOTSTRAP_TIMEOUT_S")
            .await
            .and_then(|v| u64::try_from(v).ok())
            .filter(|v| *v > 0)
            .or_else(|| {
                std::env::var("WORKER_SSH_BOOTSTRAP_TIMEOUT_S")
                    .ok()
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .filter(|v| *v > 0)
            })
            .unwrap_or(900)
    } else {
        std::env::var("WORKER_SSH_BOOTSTRAP_TIMEOUT_S")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(900)
    };

    let out = tokio::time::timeout(
        std::time::Duration::from_secs(ssh_timeout_s),
        child.wait_with_output(),
    )
    .await;
    match out {
        Ok(Ok(output)) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let phases = extract_phases(&stdout);
                let last_phase = phases.last().cloned();
                let last_phase_str = last_phase.as_deref();

                if let serde_json::Value::Object(ref mut obj) = meta {
                    obj.insert(
                        "ssh_exit_success".to_string(),
                        serde_json::json!(output.status.success()),
                    );
                    obj.insert(
                        "ssh_exit_status".to_string(),
                        serde_json::json!(output.status.to_string()),
                    );
                    obj.insert(
                        "ssh_stdout_tail".to_string(),
                        serde_json::json!(tail_str(&stdout, 8000)),
                    );
                    obj.insert(
                        "ssh_stderr_tail".to_string(),
                        serde_json::json!(tail_str(&stderr, 8000)),
                    );
                    obj.insert("phases".to_string(), serde_json::json!(phases));
                    if let Some(p) = &last_phase {
                        obj.insert("last_phase".to_string(), serde_json::json!(p));
                    }
                    obj.insert(
                        "ssh_timeout_s".to_string(),
                        serde_json::json!(ssh_timeout_s),
                    );
                }
                if output.status.success() {
                    let _ = logger::log_event_complete_with_metadata(
                        db,
                        lid,
                        "success",
                        dur,
                        None,
                        Some(meta),
                    )
                    .await;

                    // Transition to "starting" status when SSH installation completes successfully
                    // This indicates worker containers are starting and we're waiting for them to be ready
                    if last_phase_str == Some("done") {
                        eprintln!("🔄 [health_check_flow] Transitioning instance {} from installing to starting (SSH install done)", instance_id);
                        match state_machine::installing_to_starting(
                            db,
                            instance_id,
                            "SSH installation completed - worker containers starting",
                        ).await {
                            Ok(true) => eprintln!("✅ [health_check_flow] Successfully transitioned to starting"),
                            Ok(false) => eprintln!("⚠️ [health_check_flow] Transition to starting skipped (already in different status)"),
                            Err(e) => eprintln!("❌ [health_check_flow] Failed to transition to starting: {:?}", e),
                        }
                    }
                } else {
                    let msg = format!(
                        "ssh bootstrap failed (exit={}): {}",
                        output.status,
                        tail_str(&stderr, 2000)
                    );
                    let _ = logger::log_event_complete_with_metadata(
                        db,
                        lid,
                        "failed",
                        dur,
                        Some(&msg),
                        Some(meta),
                    )
                    .await;
                }
            }
        }
        Ok(Err(e)) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                if let serde_json::Value::Object(ref mut obj) = meta {
                    obj.insert(
                        "ssh_timeout_s".to_string(),
                        serde_json::json!(ssh_timeout_s),
                    );
                }
                let _ = logger::log_event_complete_with_metadata(
                    db,
                    lid,
                    "failed",
                    dur,
                    Some(&format!("ssh wait failed: {}", e)),
                    Some(meta),
                )
                .await;
            }
        }
        Err(_) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                if let serde_json::Value::Object(ref mut obj) = meta {
                    obj.insert(
                        "ssh_timeout_s".to_string(),
                        serde_json::json!(ssh_timeout_s),
                    );
                    obj.insert("ssh_timed_out".to_string(), serde_json::json!(true));
                }
                let _ = logger::log_event_complete_with_metadata(
                    db,
                    lid,
                    "failed",
                    dur,
                    Some("ssh bootstrap timed out"),
                    Some(meta),
                )
                .await;
            }
        }
    }
}

/// Manual operator action: force a (re)install of the worker over SSH.
/// This bypasses WORKER_AUTO_INSTALL gating and relaxes backoff (still avoids double-run when very recent/in progress).
pub async fn trigger_worker_reinstall_over_ssh(
    db: &Pool<Postgres>,
    instance_id: uuid::Uuid,
    ip: &str,
    correlation_id: Option<String>,
) {
    maybe_trigger_worker_install_over_ssh(db, instance_id, ip, true, correlation_id).await;
}

/// Health-check flow for BOOTING instances (probe + call state-machine transitions).
pub async fn check_and_transition_instance(
    instance_id: uuid::Uuid,
    ip: Option<String>,
    boot_started_at: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
    failures: i32,
    db: Pool<Postgres>,
) {
    #[derive(sqlx::FromRow)]
    struct ProviderInfo {
        provider_id: uuid::Uuid,
        provider_code: String,
    }
    let provider: Option<ProviderInfo> = sqlx::query_as(
        "SELECT i.provider_id as provider_id, p.code as provider_code FROM instances i JOIN providers p ON p.id = i.provider_id WHERE i.id = $1",
    )
    .bind(instance_id)
    .fetch_optional(&db)
    .await
    .ok()
    .flatten();

    let ip = match ip {
        Some(ip) => ip,
        None => {
            println!(
                "⚠️  Instance {} has no IP, skipping health check",
                instance_id
            );
            return;
        }
    };

    let Some(provider) = provider else {
        println!(
            "⚠️  Instance {} has no provider, skipping health check",
            instance_id
        );
        return;
    };

    // Determine if this instance is expected to run a worker.
    let instance_type_code: Option<String> = sqlx::query_scalar(
        "SELECT it.code FROM instances i JOIN instance_types it ON it.id = i.instance_type_id WHERE i.id = $1",
    )
    .bind(instance_id)
    .fetch_optional(&db)
    .await
    .ok()
    .flatten();

    let auto_install = std::env::var("WORKER_AUTO_INSTALL")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    let patterns = inventiv_common::worker_target::parse_instance_type_patterns(
        std::env::var("WORKER_AUTO_INSTALL_INSTANCE_PATTERNS")
            .ok()
            .as_deref(),
    );
    let expect_worker = auto_install
        && provider.provider_code.as_str() == "scaleway"
        && instance_type_code
            .as_deref()
            .map(|it| inventiv_common::worker_target::instance_type_matches_patterns(it, &patterns))
            .unwrap_or(false);

    // Timeout: provider-scoped (DB) -> env -> default.
    // Workers can take longer (image pulls + model downloads).
    let timeout_secs: i64 = if expect_worker {
        provider_setting_i64(
            &db,
            provider.provider_id,
            "WORKER_INSTANCE_STARTUP_TIMEOUT_S",
        )
        .await
        .or_else(|| {
            std::env::var("WORKER_INSTANCE_STARTUP_TIMEOUT_S")
                .ok()
                .and_then(|s| s.trim().parse::<i64>().ok())
        })
        .unwrap_or(3600)
    } else {
        provider_setting_i64(&db, provider.provider_id, "INSTANCE_STARTUP_TIMEOUT_S")
            .await
            .or_else(|| {
                std::env::var("INSTANCE_STARTUP_TIMEOUT_S")
                    .ok()
                    .and_then(|s| s.trim().parse::<i64>().ok())
            })
            .unwrap_or(300)
    };

    // Timeout after N seconds (use boot_started_at, not created_at, so reinstalls don't instantly time out)
    let age = sqlx::types::chrono::Utc::now() - boot_started_at;
    if age.num_seconds() > timeout_secs {
        println!(
            "⏱️  Instance {} timeout exceeded ({}s), marking as startup_failed",
            instance_id,
            age.num_seconds()
        );
        let timeout_msg = format!(
            "Instance failed to become healthy within {} seconds",
            timeout_secs
        );
        let _ = state_machine::booting_to_startup_failed(
            &db,
            instance_id,
            "STARTUP_TIMEOUT",
            &timeout_msg,
        )
        .await;
        return;
    }

    // Prefer worker readiness endpoint when available; fallback to SSH to keep backward-compat.
    // Ports resolution order:
    // 1) instance-specific ports (persisted from worker register/heartbeat) -> supports multi-instances behind same IP
    // 2) provider-scoped settings (DB) -> env -> defaults
    #[derive(sqlx::FromRow)]
    struct InstancePorts {
        worker_health_port: Option<i32>,
        worker_vllm_port: Option<i32>,
    }
    let inst_ports: Option<InstancePorts> =
        sqlx::query_as("SELECT worker_health_port, worker_vllm_port FROM instances WHERE id = $1")
            .bind(instance_id)
            .fetch_optional(&db)
            .await
            .ok()
            .flatten();

    let fallback_worker_port: u16 =
        provider_setting_i64(&db, provider.provider_id, "WORKER_HEALTH_PORT")
            .await
            .and_then(|v| u16::try_from(v).ok())
            .or_else(|| {
                std::env::var("WORKER_HEALTH_PORT")
                    .ok()
                    .and_then(|s| s.parse::<u16>().ok())
            })
            .unwrap_or(8080);
    let fallback_vllm_port: u16 =
        provider_setting_i64(&db, provider.provider_id, "WORKER_VLLM_PORT")
            .await
            .and_then(|v| u16::try_from(v).ok())
            .or_else(|| {
                std::env::var("WORKER_VLLM_PORT")
                    .ok()
                    .and_then(|s| s.parse::<u16>().ok())
            })
            .unwrap_or(8000);

    let worker_port: u16 = inst_ports
        .as_ref()
        .and_then(|p| p.worker_health_port)
        .and_then(|v| u16::try_from(v).ok())
        .unwrap_or(fallback_worker_port);

    let vllm_port: u16 = inst_ports
        .as_ref()
        .and_then(|p| p.worker_vllm_port)
        .and_then(|v| u16::try_from(v).ok())
        .unwrap_or(fallback_vllm_port);

    // For worker targets: prioritize heartbeats over active health checks
    // Heartbeats work bidirectionally (workers can reach control plane via Cloudflare tunnel),
    // while active health checks may fail due to network routing issues.
    let mut is_healthy_from_heartbeat = false;
    let mut heartbeat_age_secs: Option<i64> = None;

    if expect_worker {
        // Check if we have a recent heartbeat (within last 30 seconds)
        #[derive(sqlx::FromRow)]
        struct HeartbeatInfo {
            worker_last_heartbeat: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
            worker_status: Option<String>,
        }

        let heartbeat_info: Option<HeartbeatInfo> = sqlx::query_as(
            r#"
            SELECT worker_last_heartbeat, worker_status
            FROM instances
            WHERE id = $1
            "#,
        )
        .bind(instance_id)
        .fetch_optional(&db)
        .await
        .ok()
        .flatten();

        if let Some(info) = heartbeat_info {
            if let Some(last_hb) = info.worker_last_heartbeat {
                let now = sqlx::types::chrono::Utc::now();
                let age = (now - last_hb).num_seconds();
                heartbeat_age_secs = Some(age);

                // If heartbeat is recent (within 30 seconds) and status is "ready", trust it
                if age < 30 {
                    if let Some(status) = &info.worker_status {
                        let status_lower = status.to_ascii_lowercase();
                        if status_lower == "ready" {
                            is_healthy_from_heartbeat = true;
                            println!(
                                "✅ Instance {} healthy via heartbeat (status={}, age={}s)",
                                instance_id, status, age
                            );
                        } else {
                            // Worker is sending heartbeats but not ready yet - log for visibility
                            println!(
                                "ℹ️ Instance {} worker sending heartbeats (status={}, age={}s) - waiting for ready state",
                                instance_id, status, age
                            );
                        }
                    } else {
                        // Worker is sending heartbeats but status is null - log for visibility
                        println!(
                            "ℹ️ Instance {} worker sending heartbeats (status=null, age={}s) - waiting for status",
                            instance_id, age
                        );
                    }
                }
            }
        }
    }

    // Only perform active health checks if we don't have a recent healthy heartbeat
    // This avoids network routing issues when workers can reach control plane but not vice versa
    let is_ready_http = if is_healthy_from_heartbeat {
        // Trust heartbeat, skip active check
        true
    } else {
        check_instance_readyz_http(&ip, worker_port).await
    };

    let is_ssh = check_instance_ssh(&ip).await;

    // Agent info check: verify agent version and checksum for worker instances
    let mut agent_info_check: Option<serde_json::Value> = None;
    let mut agent_info_error: Option<String> = None;
    if expect_worker && is_ready_http {
        match check_agent_info(&ip, worker_port).await {
            Ok(info) => {
                agent_info_check = Some(info.clone());
                println!(
                    "📦 [Health Check] Instance {} agent info retrieved: version={:?}, checksum={:?}",
                    instance_id,
                    info.get("agent_version"),
                    info.get("agent_checksum").and_then(|c| c.as_str().map(|s| &s[..16]))
                );
            }
            Err(e) => {
                agent_info_error = Some(e.clone());
                println!(
                    "⚠️ [Health Check] Instance {} failed to retrieve agent info: {}",
                    instance_id, e
                );
            }
        }
    }

    // Model readiness check (explicit) for worker targets: verify the OpenAI endpoint is reachable
    // and the expected model is visible in /v1/models.
    // Skip if we already know from heartbeat that model is loaded
    let mut model_check_ok = true;
    if expect_worker && is_ready_http {
        let model_id_from_db: Option<String> = sqlx::query_scalar(
            r#"
            SELECT m.model_id
            FROM instances i
            LEFT JOIN models m ON m.id = i.model_id
            WHERE i.id = $1
            "#,
        )
        .bind(instance_id)
        .fetch_optional(&db)
        .await
        .ok()
        .flatten()
        .and_then(|s: String| {
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });
        let expected_model_id = model_id_from_db
            .or_else(|| {
                std::env::var("WORKER_MODEL_ID")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_default();

        let warmup_enabled = std::env::var("WORKER_VLLM_WARMUP")
            .ok()
            .map(|v| {
                matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(true);

        let (http_ok, ids, latency_ms, http_err) = check_vllm_http_models(&ip, vllm_port).await;
        let expected = expected_model_id.trim();
        let model_loaded = expected.is_empty() || ids.iter().any(|x| x == expected);
        let model_err = if http_ok && !model_loaded && !expected.is_empty() {
            Some(format!(
                "model_not_listed (expected={}, got_count={})",
                expected,
                ids.len()
            ))
        } else {
            None
        };

        // Best-effort warmup (doesn't gate readiness unless you want it to).
        let (warmup_ok, warmup_ms, warmup_err) =
            if warmup_enabled && http_ok && model_loaded && !expected.is_empty() {
                check_vllm_warmup_http(&ip, vllm_port, expected).await
            } else {
                (true, 0, None)
            };

        model_check_ok = http_ok && model_loaded;

        // Log:
        // - log each readiness step (rate-limited on both success and failure)
        async fn should_log_step(
            db: &Pool<Postgres>,
            instance_id: uuid::Uuid,
            action_type: &str,
            ok: bool,
        ) -> bool {
            if ok {
                // success: log first time, then periodically (every 5 minutes) to show continued health
                let last_success: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>> =
                    sqlx::query_scalar(
                        r#"
                    SELECT MAX(created_at)
                    FROM action_logs
                    WHERE instance_id = $1
                      AND action_type = $2
                      AND status = 'success'
                    "#,
                    )
                    .bind(instance_id)
                    .bind(action_type)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None);

                if let Some(last) = last_success {
                    // Log again if last success was more than 5 minutes ago
                    let elapsed_minutes = (sqlx::types::chrono::Utc::now() - last).num_minutes();
                    return elapsed_minutes >= 5;
                }
                // First success: always log
                return true;
            }
            // failure: log at most once per minute
            sqlx::query_scalar(
                r#"
                SELECT NOT EXISTS(
                  SELECT 1
                  FROM action_logs
                  WHERE instance_id = $1
                    AND action_type = $2
                    AND created_at > NOW() - INTERVAL '60 seconds'
                )
                "#,
            )
            .bind(instance_id)
            .bind(action_type)
            .fetch_one(db)
            .await
            .unwrap_or(false)
        }

        // 1) vLLM HTTP up (/v1/models 200)
        if should_log_step(&db, instance_id, "WORKER_VLLM_HTTP_OK", http_ok).await {
            let log_id = logger::log_event_with_metadata(
                &db,
                "WORKER_VLLM_HTTP_OK",
                "in_progress",
                instance_id,
                None,
                Some(serde_json::json!({
                    "ip": ip,
                    "vllm_port": vllm_port,
                    "latency_ms": latency_ms,
                    "result": if http_ok { "success" } else { "failed" },
                    "error": http_err
                })),
            )
            .await
            .ok();
            if let Some(lid) = log_id {
                let _ = logger::log_event_complete(
                    &db,
                    lid,
                    if http_ok { "success" } else { "failed" },
                    0,
                    http_err.as_deref(),
                )
                .await;
            }
        }

        // 2) Model listed/loaded
        if should_log_step(&db, instance_id, "WORKER_MODEL_LOADED", model_loaded).await {
            let log_id = logger::log_event_with_metadata(
                &db,
                "WORKER_MODEL_LOADED",
                "in_progress",
                instance_id,
                None,
                Some(serde_json::json!({
                    "ip": ip,
                    "vllm_port": vllm_port,
                    "expected_model_id": expected_model_id,
                    "result": if model_loaded { "success" } else { "failed" },
                    "error": model_err
                })),
            )
            .await
            .ok();
            if let Some(lid) = log_id {
                let _ = logger::log_event_complete(
                    &db,
                    lid,
                    if model_loaded { "success" } else { "failed" },
                    0,
                    model_err.as_deref(),
                )
                .await;
            }
        }

        // 3) Warmup request
        if warmup_enabled && http_ok && model_loaded && !expected.is_empty() {
            if should_log_step(&db, instance_id, "WORKER_VLLM_WARMUP", warmup_ok).await {
                let log_id = logger::log_event_with_metadata(
                    &db,
                    "WORKER_VLLM_WARMUP",
                    "in_progress",
                    instance_id,
                    None,
                    Some(serde_json::json!({
                        "ip": ip,
                        "vllm_port": vllm_port,
                        "expected_model_id": expected_model_id,
                        "latency_ms": warmup_ms,
                        "result": if warmup_ok { "success" } else { "failed" },
                        "error": warmup_err
                    })),
                )
                .await
                .ok();
                if let Some(lid) = log_id {
                    let _ = logger::log_event_complete(
                        &db,
                        lid,
                        if warmup_ok { "success" } else { "failed" },
                        0,
                        warmup_err.as_deref(),
                    )
                    .await;
                }
            }
        }

        // Persist "worker runtime" fields so /v1/models + runtime_models can reflect serving capacity
        // even if the python worker heartbeat is not implemented yet.
        //
        // If the instance doesn't have an expected model configured, infer it from /v1/models.
        let inferred_model_id: Option<String> = if !expected.is_empty() {
            Some(expected.to_string())
        } else {
            ids.get(0).cloned()
        };
        if model_check_ok {
            let Some(mid) = inferred_model_id else {
                return;
            };
            let _ = sqlx::query(
                r#"
                UPDATE instances
                SET
                  worker_model_id = $2,
                  worker_status = 'ready',
                  worker_last_heartbeat = NOW(),
                  worker_health_port = $3,
                  worker_vllm_port = $4
                WHERE id = $1
                "#,
            )
            .bind(instance_id)
            .bind(mid)
            .bind(worker_port as i32)
            .bind(vllm_port as i32)
            .execute(&db)
            .await;
        }
    }

    // Worker targets are considered healthy if:
    // 1. We have a recent healthy heartbeat (preferred - works even with network routing issues)
    // 2. OR /readyz succeeds AND model check passes (fallback for instances without heartbeats yet)
    let is_healthy = if expect_worker {
        if is_healthy_from_heartbeat {
            // Trust heartbeat - it's more reliable than active checks when network routing is asymmetric
            true
        } else {
            // Fallback to active health checks for instances that haven't sent heartbeats yet
            is_ready_http && model_check_ok
        }
    } else {
        is_ready_http || is_ssh
    };

    // Check if we should log health check (rate-limited: every 5 minutes for success, 1 minute for failure)
    let should_log_health_check = {
        let last_hc: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>> =
            sqlx::query_scalar(
                r#"
            SELECT MAX(created_at)
            FROM action_logs
            WHERE instance_id = $1
              AND action_type = 'HEALTH_CHECK'
            "#,
            )
            .bind(instance_id)
            .fetch_optional(&db)
            .await
            .unwrap_or(None);

        if let Some(last) = last_hc {
            // Log again if last check was more than 5 minutes ago (for successes) or 1 minute ago (for failures)
            let elapsed_secs = (sqlx::types::chrono::Utc::now() - last).num_seconds();
            if is_healthy {
                elapsed_secs >= 300 // 5 minutes for successes
            } else {
                elapsed_secs >= 60 // 1 minute for failures
            }
        } else {
            true // First check: always log
        }
    };

    eprintln!("🔍 [health_check_flow] Health check evaluation for instance {}: is_healthy={}, expect_worker={}, is_healthy_from_heartbeat={}, is_ready_http={}, model_check_ok={}, heartbeat_age_secs={:?}", 
        instance_id, is_healthy, expect_worker, is_healthy_from_heartbeat, is_ready_http, model_check_ok, heartbeat_age_secs);

    if is_healthy {
        println!(
            "✅ Instance {} health check PASSED! Transitioning to ready",
            instance_id
        );
        let hc_start = std::time::Instant::now();

        // Always log health check success (with rate-limiting to avoid spam)
        if should_log_health_check {
            let log_id = logger::log_event_with_metadata(
                &db,
                "HEALTH_CHECK",
                "in_progress",
                instance_id,
                None,
                {
                    let mut meta = serde_json::json!({
                        "ip": ip,
                        "result": "success",
                        "failures": failures,
                        "mode": if is_healthy_from_heartbeat {
                            "heartbeat"
                        } else if is_ready_http {
                            "worker_readyz"
                        } else {
                            "ssh_22" 
                        },
                        "heartbeat_age_secs": heartbeat_age_secs,
                        "worker_health_port": worker_port,
                        "vllm_port": vllm_port,
                        "model_check": if expect_worker { if model_check_ok { "ok" } else { "failed" } } else { "skipped" }
                    });
                    if let Some(agent_info) = &agent_info_check {
                        meta["agent_info"] = agent_info.clone();
                    }
                    if let Some(err) = &agent_info_error {
                        meta["agent_info_error"] = serde_json::json!(err);
                    }
                    Some(meta)
                },
            ).await.ok();
            if let Some(lid) = log_id {
                let dur = hc_start.elapsed().as_millis() as i32;
                let _ = logger::log_event_complete(&db, lid, "success", dur, None).await;
            }
        }

        // Only transition if not already ready
        let current_status: Option<String> =
            sqlx::query_scalar("SELECT status::text FROM instances WHERE id = $1")
                .bind(instance_id)
                .fetch_optional(&db)
                .await
                .unwrap_or(None);

        if current_status.as_deref() != Some("ready") {
            eprintln!("🔄 [health_check_flow] Transitioning instance {} from {:?} to ready (health check passed)", instance_id, current_status);
            match state_machine::booting_to_ready(&db, instance_id, "Health check passed").await {
                Ok(true) => eprintln!("✅ [health_check_flow] Successfully transitioned to ready"),
                Ok(false) => eprintln!("⚠️ [health_check_flow] Transition to ready skipped (status may have changed: {:?})", current_status),
                Err(e) => eprintln!("❌ [health_check_flow] Failed to transition to ready: {:?}", e),
            }
        } else {
            eprintln!(
                "ℹ️ [health_check_flow] Instance {} already in ready status",
                instance_id
            );
        }
    } else {
        // For worker targets: if SSH is up but readyz isn't, check if worker is sending heartbeats
        // before triggering a bootstrap over SSH. If worker is sending heartbeats, it's alive and
        // we should wait for it to become ready (model loading can take time).
        if expect_worker && is_ssh {
            // First, check if worker is sending active heartbeats (within last 2 minutes)
            let has_recent_heartbeat: Option<i64> = sqlx::query_scalar(
                r#"
                SELECT EXTRACT(EPOCH FROM (NOW() - worker_last_heartbeat))::bigint
                FROM instances
                WHERE id = $1
                  AND worker_last_heartbeat > NOW() - INTERVAL '2 minutes'
                "#,
            )
            .bind(instance_id)
            .fetch_optional(&db)
            .await
            .unwrap_or(None);

            if let Some(heartbeat_age_s) = has_recent_heartbeat {
                // Worker is sending heartbeats - it's alive, just not ready yet
                // Don't restart containers, wait for model loading or worker to become ready
                println!(
                    "ℹ️ Instance {} SSH accessible, readyz not yet, but worker is sending heartbeats (last {}s ago) - waiting for worker to become ready",
                    instance_id, heartbeat_age_s
                );
                return;
            }

            // No recent heartbeats - check if we have a very recent successful SSH install
            let recent_success: Option<i64> = sqlx::query_scalar(
                r#"
                SELECT EXTRACT(EPOCH FROM (NOW() - completed_at))::bigint
                FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'WORKER_SSH_INSTALL'
                  AND status = 'success'
                  AND metadata->>'last_phase' = 'done'
                  AND completed_at > NOW() - INTERVAL '5 minutes'
                ORDER BY completed_at DESC
                LIMIT 1
                "#,
            )
            .bind(instance_id)
            .fetch_optional(&db)
            .await
            .unwrap_or(None);

            if let Some(age_s) = recent_success {
                if age_s < 5 * 60 {
                    // Very recent successful install - containers may still be starting up
                    // Don't restart them, just wait
                    println!(
                        "ℹ️ Instance {} SSH install succeeded {}s ago - waiting for containers to become ready (SSH accessible, readyz not yet)",
                        instance_id, age_s
                    );
                    return;
                }
            }

            // Before retrying SSH install, check container health via SSH and fetch worker logs to diagnose the issue
            // This helps avoid infinite retry loops when containers are crashing
            let container_check = check_containers_via_ssh(&ip).await;
            let worker_logs = fetch_worker_logs(&ip, worker_port).await;

            if let Some((vllm_running, agent_running, vllm_exit_code, agent_exit_code)) =
                container_check
            {
                if !vllm_running || !agent_running {
                    println!(
                        "⚠️ Instance {} containers not running: vllm={} (exit={:?}), agent={} (exit={:?}) - will retry SSH install",
                        instance_id, vllm_running, vllm_exit_code, agent_running, agent_exit_code
                    );
                    // Log container diagnostics and worker logs before retrying
                    let mut metadata = serde_json::json!({
                        "ip": ip,
                        "vllm_running": vllm_running,
                        "agent_running": agent_running,
                        "vllm_exit_code": vllm_exit_code,
                        "agent_exit_code": agent_exit_code,
                        "action": "retrying_ssh_install"
                    });
                    if let Some(logs) = &worker_logs {
                        metadata["worker_logs_summary"] = serde_json::json!({
                            "total_events": logs.total_events,
                            "recent_events": logs.events.iter().take(10).map(|e| serde_json::json!({
                                "timestamp": e.get("timestamp"),
                                "event_type": e.get("event_type"),
                                "message": e.get("message"),
                            })).collect::<Vec<_>>(),
                        });
                    }
                    // This is a normal retry scenario - containers not running yet during installation
                    let _ = logger::log_event_with_metadata(
                        &db,
                        "WORKER_CONTAINER_CHECK",
                        "retry",
                        instance_id,
                        None,
                        Some(metadata),
                    )
                    .await;
                } else {
                    // Containers are running but readyz doesn't respond - check worker logs for clues
                    if let Some(logs) = &worker_logs {
                        // Check for recent errors in logs
                        let recent_errors: Vec<_> = logs
                            .events
                            .iter()
                            .filter(|e| {
                                let event_type =
                                    e.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
                                event_type.contains("failed")
                                    || event_type.contains("exception")
                                    || event_type.contains("error")
                            })
                            .take(5)
                            .collect();

                        if !recent_errors.is_empty() {
                            // Check if there's already an SSH install in progress before triggering a new one
                            let has_in_progress_install: bool = sqlx::query_scalar(
                                r#"
                                SELECT EXISTS(
                                    SELECT 1 FROM action_logs
                                    WHERE instance_id = $1
                                      AND action_type = 'WORKER_SSH_INSTALL'
                                      AND status = 'in_progress'
                                      AND created_at > NOW() - INTERVAL '20 minutes'
                                )
                                "#,
                            )
                            .bind(instance_id)
                            .fetch_one(&db)
                            .await
                            .unwrap_or(false);

                            if has_in_progress_install {
                                println!(
                                    "ℹ️ Instance {} containers running but worker logs show errors - SSH install already in progress, waiting",
                                    instance_id
                                );
                                let log_id = logger::log_event_with_metadata(
                                    &db,
                                    "WORKER_LOG_ERRORS",
                                    "in_progress",
                                    instance_id,
                                    None,
                                    Some(serde_json::json!({
                                        "ip": ip,
                                        "recent_errors": recent_errors.iter().map(|e| serde_json::json!({
                                            "timestamp": e.get("timestamp"),
                                            "event_type": e.get("event_type"),
                                            "message": e.get("message"),
                                        })).collect::<Vec<_>>(),
                                    })),
                                ).await.ok();

                                // Complete the log immediately - these errors are normal during startup
                                // and indicate the worker is trying to connect but failing (retry scenario)
                                if let Some(lid) = log_id {
                                    let _ = logger::log_event_complete(
                                        &db,
                                        lid,
                                        "retry",
                                        0,
                                        Some("Worker logs show connection errors during startup - SSH install already in progress"),
                                    ).await;
                                }
                                return;
                            }

                            println!(
                                "⚠️ Instance {} containers running but worker logs show errors - will retry SSH install",
                                instance_id
                            );
                            let log_id = logger::log_event_with_metadata(
                                &db,
                                "WORKER_LOG_ERRORS",
                                "in_progress",
                                instance_id,
                                None,
                                Some(serde_json::json!({
                                    "ip": ip,
                                    "recent_errors": recent_errors.iter().map(|e| serde_json::json!({
                                        "timestamp": e.get("timestamp"),
                                        "event_type": e.get("event_type"),
                                        "message": e.get("message"),
                                    })).collect::<Vec<_>>(),
                                })),
                            ).await.ok();

                            // Complete the log immediately - these errors are normal during startup
                            // and indicate the worker is trying to connect but failing (retry scenario)
                            if let Some(lid) = log_id {
                                let _ = logger::log_event_complete(
                                    &db,
                                    lid,
                                    "retry",
                                    0,
                                    Some("Worker logs show connection errors during startup - retrying"),
                                ).await;
                            }

                            maybe_trigger_worker_install_over_ssh(
                                &db,
                                instance_id,
                                &ip,
                                false,
                                None,
                            )
                            .await;
                            return;
                        }
                    }
                    // Containers are running but readyz doesn't respond - may be network issue or model loading
                    println!(
                        "ℹ️ Instance {} containers are running but readyz not responding - may be network issue or model loading, waiting",
                        instance_id
                    );
                    return;
                }
            } else {
                // Could not check containers via SSH - check worker logs if available
                // But first check if there's already an SSH install in progress
                let has_in_progress_install: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM action_logs
                        WHERE instance_id = $1
                          AND action_type = 'WORKER_SSH_INSTALL'
                          AND status = 'in_progress'
                          AND created_at > NOW() - INTERVAL '20 minutes'
                    )
                    "#,
                )
                .bind(instance_id)
                .fetch_one(&db)
                .await
                .unwrap_or(false);

                if has_in_progress_install {
                    println!(
                        "ℹ️ Instance {} SSH is reachable but worker readyz is not - SSH install already in progress, waiting",
                        instance_id
                    );
                    return;
                }

                if let Some(logs) = &worker_logs {
                    println!(
                        "⚠️ Instance {} SSH is reachable but worker readyz is not - fetched {} worker log events, triggering SSH bootstrap",
                        instance_id, logs.total_events
                    );
                    let _ = logger::log_event_with_metadata(
                        &db,
                        "WORKER_LOG_FETCH",
                        "success",
                        instance_id,
                        None,
                        Some(serde_json::json!({
                            "ip": ip,
                            "total_events": logs.total_events,
                            "recent_events": logs.events.iter().take(10).map(|e| serde_json::json!({
                                "timestamp": e.get("timestamp"),
                                "event_type": e.get("event_type"),
                                "message": e.get("message"),
                            })).collect::<Vec<_>>(),
                        })),
                    )
                    .await;
                } else {
                    println!(
                        "⚠️ Instance {} SSH is reachable but worker readyz is not and no recent heartbeats - could not check container status or fetch logs, triggering SSH bootstrap",
                        instance_id
                    );
                }
            }

            maybe_trigger_worker_install_over_ssh(&db, instance_id, &ip, false, None).await;
            return;
        }

        // If SSH is also not reachable, check if we should trigger installation
        if expect_worker && !is_ssh {
            // Check if we have any SSH install attempts
            let has_ssh_install: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM action_logs
                    WHERE instance_id = $1
                      AND action_type = 'WORKER_SSH_INSTALL'
                )
                "#,
            )
            .bind(instance_id)
            .fetch_one(&db)
            .await
            .unwrap_or(false);

            if !has_ssh_install {
                // No SSH install attempt yet - check if instance has been booting for a reasonable time
                // (give Scaleway time to start SSH service, typically 1-2 minutes)
                let instance_age: Option<i64> = sqlx::query_scalar(
                    r#"
                    SELECT EXTRACT(EPOCH FROM (NOW() - created_at))::bigint
                    FROM instances
                    WHERE id = $1
                    "#,
                )
                .bind(instance_id)
                .fetch_optional(&db)
                .await
                .unwrap_or(None);

                if let Some(age_s) = instance_age {
                    // Wait at least 2 minutes for Scaleway instance to fully boot and SSH to become available
                    if age_s >= 120 && age_s < 300 {
                        // After 2 minutes but before 5 minutes, try SSH installation even if port 22 is not yet accessible
                        // The SSH service may not be fully started yet, but the connection attempt will wait
                        println!(
                            "⏳ Instance {} has been booting for {}s - SSH port 22 not yet accessible, but attempting SSH installation anyway (SSH service may still be starting)",
                            instance_id, age_s
                        );
                        maybe_trigger_worker_install_over_ssh(&db, instance_id, &ip, false, None)
                            .await;
                        return;
                    } else if age_s >= 300 {
                        // After 5 minutes, mark instance as failed if SSH is still not accessible
                        // This indicates a serious boot problem
                        let error_msg = format!(
                            "Instance failed to boot: SSH port 22 not accessible after {} seconds. Instance may not be booting correctly or SSH service is not starting.",
                            age_s
                        );
                        eprintln!("❌ {}", error_msg);

                        let _ = sqlx::query(
                            "UPDATE instances 
                             SET status='failed', 
                                 error_code=COALESCE(error_code,'SSH_NOT_ACCESSIBLE'),
                                 error_message=COALESCE($2,error_message),
                                 failed_at=COALESCE(failed_at,NOW())
                             WHERE id=$1",
                        )
                        .bind(instance_id)
                        .bind(&error_msg)
                        .execute(&db)
                        .await;

                        return;
                    }
                }
                // Instance is very new (< 2 minutes), SSH may not be ready yet
                return;
            }

            // We have SSH install attempts - check if we have a recent successful SSH install
            let recent_success: Option<i64> = sqlx::query_scalar(
                r#"
                SELECT EXTRACT(EPOCH FROM (NOW() - completed_at))::bigint
                FROM action_logs
                WHERE instance_id = $1
                  AND action_type = 'WORKER_SSH_INSTALL'
                  AND status = 'success'
                  AND metadata->>'last_phase' = 'done'
                  AND completed_at > NOW() - INTERVAL '20 minutes'
                ORDER BY completed_at DESC
                LIMIT 1
                "#,
            )
            .bind(instance_id)
            .fetch_optional(&db)
            .await
            .unwrap_or(None);

            if let Some(age_s) = recent_success {
                if age_s > 10 * 60 {
                    // SSH install succeeded more than 10 minutes ago but SSH is not reachable
                    // This suggests containers may have crashed or the instance had issues
                    println!(
                        "⚠️ Instance {} SSH install succeeded {}s ago but SSH is not reachable - containers may have crashed, retrying installation",
                        instance_id, age_s
                    );
                    maybe_trigger_worker_install_over_ssh(&db, instance_id, &ip, false, None).await;
                    return;
                }
            }
        }

        let new_failures = failures + 1;
        println!(
            "❌ Instance {} health check FAILED (attempt {}/30)",
            instance_id, new_failures
        );

        let hc_start = std::time::Instant::now();
        let log_id = logger::log_event_with_metadata(
            &db,
            "HEALTH_CHECK",
            "in_progress",
            instance_id,
            None,
            Some(serde_json::json!({
                "ip": ip,
                "result": "failed",
                "failures": new_failures,
                "mode": "ssh_22_or_worker_readyz",
                "worker_health_port": worker_port
            })),
        )
        .await
        .ok();

        let _ = state_machine::update_booting_health_failures(&db, instance_id, new_failures).await;

        if let Some(lid) = log_id {
            let dur = hc_start.elapsed().as_millis() as i32;
            let _ = logger::log_event_complete(
                &db,
                lid,
                "failed",
                dur,
                Some("Worker readyz not reachable and SSH port 22 not reachable"),
            )
            .await;
        }

        if new_failures >= 30 {
            println!(
                "❌ Instance {} exceeded max health check retries, marking as startup_failed",
                instance_id
            );
            let _ = state_machine::booting_to_startup_failed(
                &db,
                instance_id,
                "HEALTH_CHECK_FAILED",
                "Instance failed health checks after 30 attempts",
            )
            .await;
        }
    }
}
