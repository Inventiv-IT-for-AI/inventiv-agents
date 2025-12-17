use std::net::TcpStream;
use std::time::Duration as StdDuration;
use std::process::Stdio;

use sqlx::{Pool, Postgres};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::state_machine;
use crate::logger;

fn tail_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    // Keep last max_chars characters (best effort for UTF-8).
    s.chars().rev().take(max_chars).collect::<String>().chars().rev().collect()
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

pub(crate) async fn check_vllm_http_models(ip: &str, port: u16) -> (bool, Vec<String>, i32, Option<String>) {
    let clean_ip = ip.split('/').next().unwrap_or(ip);
    let url = format!("http://{}:{}/v1/models", clean_ip, port);
    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(2))
        .timeout(StdDuration::from_secs(4))
        .build();
    let Ok(client) = client else {
        return (false, Vec::new(), 0, Some("client_build_failed".to_string()));
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
        return (false, Vec::new(), ms, Some(format!("status={}", resp.status())));
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

async fn check_vllm_warmup_http(ip: &str, port: u16, model_id: &str) -> (bool, i32, Option<String>) {
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

    let cp_url = worker_control_plane_url();
    if cp_url.is_empty() {
        return;
    }

    // Global token for early bringup (API also accepts it).
    let worker_auth_token = std::env::var("WORKER_AUTH_TOKEN").unwrap_or_default();
    let worker_hf_token = worker_hf_token();

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
        if t.is_empty() { None } else { Some(t) }
    });

    let model_id = model_id_from_db
        .or_else(|| {
            std::env::var("WORKER_MODEL_ID")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| "Qwen/Qwen2.5-0.5B-Instruct".to_string());
    let vllm_image = std::env::var("WORKER_VLLM_IMAGE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "vllm/vllm-openai:latest".to_string());
    let vllm_mode = std::env::var("WORKER_VLLM_MODE")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "mono".to_string()); // mono | multi
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

    let mut meta = serde_json::json!({
        "ip": clean_ip,
        "ssh_user": ssh_user,
        "ssh_key_path": ssh_key_path,
        "control_plane_url": cp_url,
        "model_id": model_id,
        "vllm_image": vllm_image,
        "vllm_mode": vllm_mode,
        "agent_url": agent_url,
        "has_hf_token": !worker_hf_token.trim().is_empty()
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
WORKER_AUTH_TOKEN={worker_auth_token}
WORKER_HF_TOKEN={worker_hf_token}

echo "::phase::start"
echo "[inventiv-worker] ssh bootstrap starting"

echo "::phase::mount_data_volume"
# Mount the attached data volume (Scaleway SBS) and use it for Docker + HF cache.
# The root disk on GPU images is often too small for pulling vLLM images.
DATA_MNT="/mnt/inventiv-data"
mkdir -p "$DATA_MNT"

root_src="$(findmnt -n -o SOURCE / || true)"
root_base=""
if [[ "$root_src" =~ ^/dev/ ]]; then
  # strip partition suffix: /dev/sda2 -> /dev/sda, /dev/nvme0n1p2 -> /dev/nvme0n1
  root_base="/dev/$(basename "$root_src" | sed -E 's/p?[0-9]+$//')"
fi

candidate=""
while read -r name type; do
  [[ "$type" == "disk" ]] || continue
  dev="/dev/$name"
  [[ -n "$root_base" && "$dev" == "$root_base" ]] && continue
  # ignore disks that are already mounted
  if lsblk -n -o MOUNTPOINT "$dev" 2>/dev/null | grep -q '/'; then
    continue
  fi
  candidate="$dev"
  break
done < <(lsblk -ndo NAME,TYPE 2>/dev/null || true)

if [[ -n "$candidate" ]]; then
  if ! blkid "$candidate" >/dev/null 2>&1; then
    mkfs.ext4 -F -L inventiv-data "$candidate"
  fi
  uuid="$(blkid -s UUID -o value "$candidate" || true)"
  if [[ -n "$uuid" ]]; then
    grep -q "$uuid" /etc/fstab || echo "UUID=$uuid $DATA_MNT ext4 defaults,nofail 0 2" >> /etc/fstab
  fi
  mount -a || true
  mountpoint -q "$DATA_MNT" || mount "$candidate" "$DATA_MNT"
fi

if mountpoint -q "$DATA_MNT"; then
  mkdir -p "$DATA_MNT/docker" "$DATA_MNT/worker"
  # Put worker files on the data disk
  if [[ -d /opt/inventiv-worker && ! -L /opt/inventiv-worker ]]; then
    rm -rf /opt/inventiv-worker
  fi
  ln -sfn "$DATA_MNT/worker" /opt/inventiv-worker

  # Put Docker storage on the data disk (avoid huge image pulls on root)
  systemctl stop docker >/dev/null 2>&1 || true
  if [[ -d /var/lib/docker && ! -L /var/lib/docker ]]; then
    # If it already has content, keep a backup rather than deleting.
    if [[ "$(ls -A /var/lib/docker 2>/dev/null | wc -l | tr -d ' ')" != "0" ]]; then
      mv /var/lib/docker "/var/lib/docker.bak.$(date +%s)"
    else
      rm -rf /var/lib/docker
    fi
  fi
  ln -sfn "$DATA_MNT/docker" /var/lib/docker
  systemctl start docker >/dev/null 2>&1 || true
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
fi

echo "::phase::agent_download"
mkdir -p /opt/inventiv-worker
curl -fsSL "$AGENT_URL" -o /opt/inventiv-worker/agent.py

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
  # Mono vLLM: one server uses all GPUs.
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
  -e WORKER_HEALTH_PORT=8080 \
  -e WORKER_VLLM_PORT=8000 \
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
        worker_auth_token = sh_escape_single(&worker_auth_token),
        worker_hf_token = sh_escape_single(&worker_hf_token),
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

    let ssh_timeout_s: u64 = std::env::var("WORKER_SSH_BOOTSTRAP_TIMEOUT_S")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(900);

    let out = tokio::time::timeout(std::time::Duration::from_secs(ssh_timeout_s), child.wait_with_output()).await;
    match out {
        Ok(Ok(output)) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let phases = extract_phases(&stdout);
                let last_phase = phases.last().cloned();

                if let serde_json::Value::Object(ref mut obj) = meta {
                    obj.insert("ssh_exit_success".to_string(), serde_json::json!(output.status.success()));
                    obj.insert("ssh_exit_status".to_string(), serde_json::json!(output.status.to_string()));
                    obj.insert("ssh_stdout_tail".to_string(), serde_json::json!(tail_str(&stdout, 8000)));
                    obj.insert("ssh_stderr_tail".to_string(), serde_json::json!(tail_str(&stderr, 8000)));
                    obj.insert("phases".to_string(), serde_json::json!(phases));
                    if let Some(p) = last_phase {
                        obj.insert("last_phase".to_string(), serde_json::json!(p));
                    }
                    obj.insert("ssh_timeout_s".to_string(), serde_json::json!(ssh_timeout_s));
                }
                if output.status.success() {
                    let _ = logger::log_event_complete_with_metadata(db, lid, "success", dur, None, Some(meta)).await;
                } else {
                    let msg = format!("ssh bootstrap failed (exit={}): {}", output.status, tail_str(&stderr, 2000));
                    let _ = logger::log_event_complete_with_metadata(db, lid, "failed", dur, Some(&msg), Some(meta)).await;
                }
            }
        }
        Ok(Err(e)) => {
            if let Some(lid) = log_id {
                let dur = started.elapsed().as_millis() as i32;
                if let serde_json::Value::Object(ref mut obj) = meta {
                    obj.insert("ssh_timeout_s".to_string(), serde_json::json!(ssh_timeout_s));
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
                    obj.insert("ssh_timeout_s".to_string(), serde_json::json!(ssh_timeout_s));
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
    let vllm_port: u16 = std::env::var("WORKER_VLLM_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8000);

    let is_ready_http = check_instance_readyz_http(&ip, worker_port).await;
    let is_ssh = check_instance_ssh(&ip).await;

    // Model readiness check (explicit) for worker targets: verify the OpenAI endpoint is reachable
    // and the expected model is visible in /v1/models.
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
            if t.is_empty() { None } else { Some(t) }
        });
        let expected_model_id = model_id_from_db
            .or_else(|| std::env::var("WORKER_MODEL_ID").ok().filter(|s| !s.trim().is_empty()))
            .unwrap_or_default();

        let warmup_enabled = std::env::var("WORKER_VLLM_WARMUP")
            .ok()
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);

        let (http_ok, ids, latency_ms, http_err) = check_vllm_http_models(&ip, vllm_port).await;
        let expected = expected_model_id.trim();
        let model_loaded = expected.is_empty() || ids.iter().any(|x| x == expected);
        let model_err = if http_ok && !model_loaded && !expected.is_empty() {
            Some(format!("model_not_listed (expected={}, got_count={})", expected, ids.len()))
        } else {
            None
        };

        // Best-effort warmup (doesn't gate readiness unless you want it to).
        let (warmup_ok, warmup_ms, warmup_err) = if warmup_enabled && http_ok && model_loaded && !expected.is_empty() {
            check_vllm_warmup_http(&ip, vllm_port, expected).await
        } else {
            (true, 0, None)
        };

        model_check_ok = http_ok && model_loaded;

        // Log:
        // - log each readiness step (rate-limited on failure)
        async fn should_log_step(db: &Pool<Postgres>, instance_id: uuid::Uuid, action_type: &str, ok: bool) -> bool {
            if ok {
                // success: log only if we never logged a success before
                let already_ok: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM action_logs WHERE instance_id = $1 AND action_type = $2 AND status = 'success')"
                )
                .bind(instance_id)
                .bind(action_type)
                .fetch_one(db)
                .await
                .unwrap_or(false);
                return !already_ok;
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
            ).await.ok();
            if let Some(lid) = log_id {
                let _ = logger::log_event_complete(&db, lid, if http_ok { "success" } else { "failed" }, 0, http_err.as_deref()).await;
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
            ).await.ok();
            if let Some(lid) = log_id {
                let _ = logger::log_event_complete(&db, lid, if model_loaded { "success" } else { "failed" }, 0, model_err.as_deref()).await;
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
                ).await.ok();
                if let Some(lid) = log_id {
                    let _ = logger::log_event_complete(&db, lid, if warmup_ok { "success" } else { "failed" }, 0, warmup_err.as_deref()).await;
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
            let Some(mid) = inferred_model_id else { return; };
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

    // Worker targets are only considered healthy when /readyz succeeds.
    let is_healthy = if expect_worker { is_ready_http && model_check_ok } else { is_ready_http || is_ssh };

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
                "worker_health_port": worker_port,
                "vllm_port": vllm_port,
                "model_check": if expect_worker { if model_check_ok { "ok" } else { "failed" } } else { "skipped" }
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

