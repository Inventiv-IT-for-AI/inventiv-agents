use std::net::TcpStream;
use std::time::Duration as StdDuration;
use std::process::Stdio;

use sqlx::{Pool, Postgres};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::state_machine;
use crate::logger;

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

fn sh_escape_single(s: &str) -> String {
    // Safe single-quote escape for bash: wrap with '...' and escape internal quotes.
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

async fn maybe_trigger_worker_install_over_ssh(db: &Pool<Postgres>, instance_id: uuid::Uuid, ip: &str) {
    let auto_install = std::env::var("WORKER_AUTO_INSTALL")
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    if !auto_install {
        return;
    }

    let cp_url = std::env::var("WORKER_CONTROL_PLANE_URL").unwrap_or_default();
    let cp_url = cp_url.trim().trim_end_matches('/').to_string();
    if cp_url.is_empty() {
        return;
    }

    // Global token for early bringup (API also accepts it).
    let worker_auth_token = std::env::var("WORKER_AUTH_TOKEN").unwrap_or_default();

    let model_id = std::env::var("WORKER_MODEL_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Qwen/Qwen2.5-0.5B-Instruct".to_string());
    let vllm_image = std::env::var("WORKER_VLLM_IMAGE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "vllm/vllm-openai:latest".to_string());
    let agent_url = std::env::var("WORKER_AGENT_SOURCE_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://raw.githubusercontent.com/Inventiv-IT-for-AI/inventiv-agents/main/inventiv-worker/agent.py".to_string());

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

    // Basic throttle: avoid spamming the same instance with SSH installs.
    let recent_install: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM action_logs
          WHERE instance_id = $1
            AND action_type = 'WORKER_SSH_INSTALL'
            AND created_at > NOW() - INTERVAL '2 minutes'
        )
        "#,
    )
    .bind(instance_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);
    if recent_install {
        return;
    }

    let log_id = logger::log_event_with_metadata(
        db,
        "WORKER_SSH_INSTALL",
        "in_progress",
        instance_id,
        None,
        Some(serde_json::json!({
            "ip": clean_ip,
            "ssh_user": ssh_user,
            "ssh_key_path": ssh_key_path,
            "control_plane_url": cp_url,
            "model_id": model_id,
            "vllm_image": vllm_image,
            "agent_url": agent_url
        })),
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
AGENT_URL={agent_url}
WORKER_AUTH_TOKEN={worker_auth_token}

echo "[inventiv-worker] ssh bootstrap starting"

if ! command -v docker >/dev/null 2>&1; then
  apt-get update -y
  apt-get install -y ca-certificates curl gnupg
  curl -fsSL https://get.docker.com | sh
fi
systemctl enable --now docker || true

if command -v nvidia-smi >/dev/null 2>&1; then
  echo "[inventiv-worker] installing nvidia-container-toolkit"
  set +e
  . /etc/os-release
  distribution="${{ID}}${{VERSION_ID}}"
  curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | gpg --batch --yes --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
  curl -fsSL "https://nvidia.github.io/libnvidia-container/${{distribution}}/libnvidia-container.list" \
    | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' \
    > /etc/apt/sources.list.d/nvidia-container-toolkit.list
  apt-get update -y
  apt-get install -y nvidia-container-toolkit
  nvidia-ctk runtime configure --runtime=docker
  systemctl restart docker
  set -e
fi

mkdir -p /opt/inventiv-worker
curl -fsSL "$AGENT_URL" -o /opt/inventiv-worker/agent.py

docker pull "$VLLM_IMAGE" || true
docker pull python:3.11-slim || true

docker rm -f vllm >/dev/null 2>&1 || true
docker run -d --restart unless-stopped \
  --name vllm \
  --gpus all \
  -p 8000:8000 \
  -e HF_HOME=/opt/inventiv-worker/hf \
  -e TRANSFORMERS_CACHE=/opt/inventiv-worker/hf \
  -v /opt/inventiv-worker:/opt/inventiv-worker \
  "$VLLM_IMAGE" \
  --host 0.0.0.0 --port 8000 \
  --model "$MODEL_ID" \
  --dtype float16 || true

docker rm -f inventiv-agent >/dev/null 2>&1 || true
docker run -d --restart unless-stopped \
  --name inventiv-agent \
  --network host \
  -e CONTROL_PLANE_URL="$CONTROL_PLANE_URL" \
  -e INSTANCE_ID="$INSTANCE_ID" \
  -e MODEL_ID="$MODEL_ID" \
  -e VLLM_BASE_URL="http://127.0.0.1:8000" \
  -e WORKER_HEALTH_PORT=8080 \
  -e WORKER_VLLM_PORT=8000 \
  -e WORKER_HEARTBEAT_INTERVAL_S=10 \
  -e WORKER_AUTH_TOKEN="$WORKER_AUTH_TOKEN" \
  -v /opt/inventiv-worker/agent.py:/app/agent.py:ro \
  python:3.11-slim \
  bash -lc "pip install --no-cache-dir requests >/dev/null && python /app/agent.py" || true

echo "[inventiv-worker] ssh bootstrap done"
"#,
        instance_id = sh_escape_single(&instance_id.to_string()),
        cp_url = sh_escape_single(&cp_url),
        model_id = sh_escape_single(&model_id),
        vllm_image = sh_escape_single(&vllm_image),
        agent_url = sh_escape_single(&agent_url),
        worker_auth_token = sh_escape_single(&worker_auth_token),
    );

    let started = std::time::Instant::now();
    let mut child = match Command::new("ssh")
        .arg("-i")
        .arg(&ssh_key_path)
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("UserKnownHostsFile=/dev/null")
        .arg("-o")
        .arg("ConnectTimeout=10")
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
                let _ = logger::log_event_complete(db, lid, "failed", dur, Some(&format!("ssh spawn failed: {}", e))).await;
            }
            return;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(script.as_bytes()).await;
    }

    let out = tokio::time::timeout(std::time::Duration::from_secs(90), child.wait_with_output()).await;
    match out {
        Ok(Ok(output)) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                if output.status.success() {
                    let _ = logger::log_event_complete(db, lid, "success", dur, None).await;
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let msg = format!("ssh bootstrap failed (exit={}): {}", output.status, stderr);
                    let _ = logger::log_event_complete(db, lid, "failed", dur, Some(&msg)).await;
                }
            }
        }
        Ok(Err(e)) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                let _ = logger::log_event_complete(db, lid, "failed", dur, Some(&format!("ssh wait failed: {}", e))).await;
            }
        }
        Err(_) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                let _ = logger::log_event_complete(db, lid, "failed", dur, Some("ssh bootstrap timed out")).await;
            }
        }
    }
}

/// Health-check flow for BOOTING instances (probe + call state-machine transitions).
pub async fn check_and_transition_instance(
    instance_id: uuid::Uuid,
    ip: Option<String>,
    created_at: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
    failures: i32,
    db: Pool<Postgres>,
) {
    // Mock provider: no real SSH. We emulate a successful startup so the rest of the platform can be tested.
    let provider_code: Option<String> = sqlx::query_scalar(
        "SELECT p.code FROM instances i JOIN providers p ON p.id = i.provider_id WHERE i.id = $1",
    )
    .bind(instance_id)
    .fetch_optional(&db)
    .await
    .ok()
    .flatten();

    let ip = match ip {
        Some(ip) => ip,
        None => {
            println!("⚠️  Instance {} has no IP, skipping health check", instance_id);
            return;
        }
    };

    if provider_code.as_deref() == Some("mock") {
        println!("✅ Instance {} is on mock provider: auto-ready", instance_id);
        let hc_start = std::time::Instant::now();
        let log_id = logger::log_event_with_metadata(
            &db,
            "HEALTH_CHECK",
            "in_progress",
            instance_id,
            None,
            Some(serde_json::json!({"ip": ip, "result": "success", "failures": failures, "mode": "mock"})),
        )
        .await
        .ok();
        let _ = state_machine::booting_to_ready(&db, instance_id, "Mock provider auto-ready").await;
        if let Some(lid) = log_id {
            let dur = hc_start.elapsed().as_millis() as i32;
            let _ = logger::log_event_complete(&db, lid, "success", dur, None).await;
        }
        return;
    }

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
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    let patterns = inventiv_common::worker_target::parse_instance_type_patterns(
        std::env::var("WORKER_AUTO_INSTALL_INSTANCE_PATTERNS").ok().as_deref(),
    );
    let expect_worker = auto_install
        && provider_code.as_deref() == Some("scaleway")
        && instance_type_code
            .as_deref()
            .map(|it| inventiv_common::worker_target::instance_type_matches_patterns(it, &patterns))
            .unwrap_or(false);

    // Timeout: workers can take longer (image pulls + model downloads).
    let timeout_secs: i64 = if expect_worker { 1200 } else { 300 };

    // Timeout after N seconds
    let age = sqlx::types::chrono::Utc::now() - created_at;
    if age.num_seconds() > timeout_secs {
        println!(
            "⏱️  Instance {} timeout exceeded ({}s), marking as startup_failed",
            instance_id,
            age.num_seconds()
        );
        let timeout_msg = format!("Instance failed to become healthy within {} seconds", timeout_secs);
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
    let worker_port: u16 = std::env::var("WORKER_HEALTH_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8080);

    let is_ready_http = check_instance_readyz_http(&ip, worker_port).await;
    let is_ssh = check_instance_ssh(&ip).await;

    // Worker targets are only considered healthy when /readyz succeeds.
    let is_healthy = if expect_worker { is_ready_http } else { is_ready_http || is_ssh };

    if is_healthy {
        println!("✅ Instance {} health check PASSED! Transitioning to ready", instance_id);
        let hc_start = std::time::Instant::now();
        let log_id = logger::log_event_with_metadata(
            &db,
            "HEALTH_CHECK",
            "in_progress",
            instance_id,
            None,
            Some(serde_json::json!({
                "ip": ip,
                "result": "success",
                "failures": failures,
                "mode": if is_ready_http { "worker_readyz" } else { "ssh_22" },
                "worker_health_port": worker_port
            })),
        ).await.ok();
        let _ = state_machine::booting_to_ready(&db, instance_id, "Health check passed").await;
        if let Some(lid) = log_id {
            let dur = hc_start.elapsed().as_millis() as i32;
            let _ = logger::log_event_complete(&db, lid, "success", dur, None).await;
        }
    } else {
        // For worker targets: if SSH is up but readyz isn't, trigger a bootstrap over SSH and
        // do NOT increment failures (installation can take several minutes).
        if expect_worker && is_ssh {
            maybe_trigger_worker_install_over_ssh(&db, instance_id, &ip).await;
            return;
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
        ).await.ok();

        let _ = state_machine::update_booting_health_failures(&db, instance_id, new_failures).await;

        if let Some(lid) = log_id {
            let dur = hc_start.elapsed().as_millis() as i32;
            let _ = logger::log_event_complete(&db, lid, "failed", dur, Some("Worker readyz not reachable and SSH port 22 not reachable")).await;
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

