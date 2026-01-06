use crate::{inventory, CloudProvider};
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Pool, Postgres};
use std::process::Stdio;
use tokio::process::Command;

pub struct MockProvider {
    db: Pool<Postgres>,
    provider_code: &'static str,
}

impl MockProvider {
    pub fn new(db: Pool<Postgres>) -> Self {
        Self {
            db,
            provider_code: "mock",
        }
    }

    async fn maybe_finalize_termination(&self, zone: &str, server_id: &str) -> Result<()> {
        // If delete_after passed, flip to terminated.
        let _ = sqlx::query(
            r#"
            UPDATE mock_provider_instances
            SET status = 'terminated',
                terminated_at = COALESCE(terminated_at, NOW())
            WHERE provider_instance_id = $1
              AND zone_code = $2
              AND status = 'terminating'
              AND delete_after IS NOT NULL
              AND delete_after <= NOW()
            "#,
        )
        .bind(server_id)
        .bind(zone)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// Resolve instance_id (UUID from instances table) from provider_instance_id (server_id).
    async fn resolve_instance_id(&self, server_id: &str) -> Result<Option<uuid::Uuid>> {
        let instance_id: Option<uuid::Uuid> = sqlx::query_scalar(
            r#"
            SELECT i.id
            FROM instances i
            WHERE i.provider_instance_id = $1
            LIMIT 1
            "#,
        )
        .bind(server_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(instance_id)
    }

    /// Get the control-plane Docker network name (from env or docker compose config).
    async fn get_controlplane_network_name(&self) -> Result<String> {
        // Try env var first (set by docker-compose.yml or Makefile)
        if let Ok(net) = std::env::var("CONTROLPLANE_NETWORK_NAME") {
            if !net.is_empty() {
                return Ok(net);
            }
        }
        // Fallback: try to infer from docker compose config
        // This is best-effort and may fail if docker compose is not available
        let output = Command::new("docker")
            .args(["compose", "config", "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
            .ok();
        if let Some(output) = output {
            if output.status.success() {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(networks) = json.get("networks") {
                        if let Some(default) = networks.get("default") {
                            if let Some(name) = default.get("name").and_then(|v| v.as_str()) {
                                return Ok(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        // Final fallback: common default
        Ok("inventiv-agents-worker-fixes_default".to_string())
    }

    /// Get the project root directory (where docker-compose.mock-runtime.yml lives).
    fn get_project_root(&self) -> String {
        // Try env var first (set by docker-compose.yml)
        if let Ok(root) = std::env::var("PROJECT_ROOT") {
            if !root.is_empty() {
                return root;
            }
        }
        // When running in Docker, the orchestrator's working dir is /app
        // and the project files are mounted there
        "/app".to_string()
    }

    /// Start a mock runtime Docker compose stack for the given instance.
    async fn start_runtime(&self, instance_id: uuid::Uuid, _server_id: &str) -> Result<String> {
        let id12 = instance_id.to_string().replace('-', "").chars().take(12).collect::<String>();
        let project_name = format!("mockrt-{}", id12);
        let network_name = self.get_controlplane_network_name().await?;
        // Mock provider always uses mock-echo-model for synthetic mock vLLM
        let model_id = "mock-echo-model".to_string();
        let project_root = self.get_project_root();
        
        // Mock Provider uses synthetic mock vLLM (echo responses) for local testing
        // This validates the complete chain: provisioning, monitoring, decommissioning
        // Real vLLM will be used with real providers (Scaleway, etc.) in staging/prod
        let compose_file = format!("{}/docker-compose.mock-runtime.yml", project_root);

        // Try 'docker compose' (plugin) first, fallback to 'docker-compose' (standalone)
        // Skip --build to avoid blocking (containers are built on first run or via make)
        let mut cmd = Command::new("docker");
        cmd.args([
            "compose",
            "-f",
            &compose_file,
            "-p",
            &project_name,
            "up",
            "-d",
            "--remove-orphans",
        ]);
        // Set working directory to project root so docker-compose can find the file
        // This also ensures that relative paths in docker-compose.mock-runtime.yml resolve correctly
        cmd.current_dir(&project_root);
        cmd.env("CONTROLPLANE_NETWORK_NAME", &network_name);
        cmd.env("INSTANCE_ID", instance_id.to_string());
        cmd.env("MOCK_VLLM_MODEL_ID", &model_id);
        cmd.env("WORKER_SIMULATE_GPU_COUNT", std::env::var("WORKER_SIMULATE_GPU_COUNT").unwrap_or_else(|_| "1".to_string()));
        cmd.env("WORKER_SIMULATE_GPU_VRAM_MB", std::env::var("WORKER_SIMULATE_GPU_VRAM_MB").unwrap_or_else(|_| "24576".to_string()));
        cmd.env("WORKER_AUTH_TOKEN", std::env::var("WORKER_AUTH_TOKEN").unwrap_or_else(|_| "dev-worker-token".to_string()));
        
        // Mock vLLM doesn't need model configuration (it just echoes requests)

        // Execute with timeout (30 seconds max for docker compose up)
        // Use spawn + select to allow killing the process on timeout
        let mut child = match cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to spawn 'docker compose': {}. Make sure Docker CLI is installed and docker compose plugin is available.",
                    e
                ));
            }
        };

        // Use select! to handle timeout and process completion concurrently
        // Get child ID before select! since wait_with_output() takes ownership
        let child_id_opt = child.id();
        let output = tokio::select! {
            result = child.wait_with_output() => {
                match result {
                    Ok(output) => output,
                    Err(e) => return Err(anyhow::anyhow!("docker compose up failed: {}", e)),
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                // Timeout: kill the process by PID (best-effort)
                if let Some(pid) = child_id_opt {
                    let _ = Command::new("kill")
                        .args(["-9", &pid.to_string()])
                        .output()
                        .await;
                }
                return Err(anyhow::anyhow!("docker compose up timed out after 30s"));
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow::anyhow!(
                "docker compose up failed for {}: stderr={} stdout={}",
                project_name,
                stderr,
                stdout
            ));
        }

        // Wait a bit for containers to start, then get the IP (with timeout)
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Get the IP of the mock-vllm container (with retries and timeout)
        let container_name = format!("{}-mock-vllm-1", project_name);
        let mut ip: Option<String> = None;
        for attempt in 1..=5 {
            let mut ip_cmd = Command::new("docker");
            ip_cmd.args([
                "inspect",
                "--format",
                "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
                &container_name,
            ]);
            ip_cmd.stdout(Stdio::piped()).stderr(Stdio::null());
            
            match ip_cmd.spawn() {
                Ok(mut child) => {
                    // Get child ID before select! since wait_with_output() takes ownership
                    let child_id_opt = child.id();
                    let ip_result = tokio::select! {
                        result = child.wait_with_output() => {
                            match result {
                                Ok(ip_output) if ip_output.status.success() => {
                                    let ip_str = String::from_utf8_lossy(&ip_output.stdout).trim().to_string();
                                    if !ip_str.is_empty() {
                                        Some(ip_str)
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            }
                        }
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                            // Timeout: kill by PID (best-effort)
                            if let Some(pid) = child_id_opt {
                                let _ = Command::new("kill")
                                    .args(["-9", &pid.to_string()])
                                    .output()
                                    .await;
                            }
                            None
                        }
                    };
                    
                    if let Some(ip_str) = ip_result {
                        ip = Some(ip_str);
                        break;
                    } else if attempt < 5 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
                Err(_) => {
                    if attempt < 5 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }
        
        if let Some(ip_str) = ip {
            return Ok(ip_str);
        }

        Err(anyhow::anyhow!(
            "Failed to get IP for mock runtime {} after 5 attempts",
            project_name
        ))
    }

    /// Stop a mock runtime Docker compose stack.
    async fn stop_runtime(&self, instance_id: uuid::Uuid) -> Result<()> {
        let id12 = instance_id.to_string().replace('-', "").chars().take(12).collect::<String>();
        let project_name = format!("mockrt-{}", id12);
        let network_name = self.get_controlplane_network_name().await?;
        let project_root = self.get_project_root();
        
        // Mock Provider only uses synthetic mock vLLM
        let compose_files = vec![
            format!("{}/docker-compose.mock-runtime.yml", project_root),
        ];

        // Try stopping with each compose file (best-effort, don't fail if one doesn't exist)
        for compose_file in compose_files {
            let mut cmd = Command::new("docker");
            cmd.args([
                "compose",
                "-f",
                &compose_file,
                "-p",
                &project_name,
                "down",
                "-v",
                "--remove-orphans",
            ]);
            cmd.current_dir(&project_root);
            cmd.env("CONTROLPLANE_NETWORK_NAME", &network_name);

            // Execute with timeout (10 seconds max for docker compose down)
            let mut child = match cmd
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => {
                    // Skip if spawn fails (file might not exist)
                    continue;
                }
            };

            let child_id_opt = child.id();
            let output = tokio::select! {
                result = child.wait_with_output() => {
                    match result {
                        Ok(output) => output,
                        Err(_) => {
                            continue;
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(10)) => {
                    // Timeout: kill by PID (best-effort)
                    if let Some(pid) = child_id_opt {
                        let _ = Command::new("kill")
                            .args(["-9", &pid.to_string()])
                            .output()
                            .await;
                    }
                    continue;
                }
            };

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Don't fail if runtime doesn't exist (idempotent)
                if !stderr.contains("No such project") && !stderr.contains("not found") {
                    // Best-effort: log but don't fail termination
                    eprintln!("⚠️ docker-compose down failed for {} ({}): {}", project_name, compose_file, stderr);
                }
            }
        }

        Ok(())
    }

    async fn validate_zone_and_type(&self, zone: &str, instance_type: &str) -> Result<()> {
        // Resolve provider id from code (no hardcoded UUIDs)
        let provider_id: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM providers WHERE code = $1 LIMIT 1")
                .bind(self.provider_code)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "MockProvider: provider '{}' not found in DB",
                        self.provider_code
                    )
                })?;

        // Ensure zone exists for mock provider
        let zone_ok: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
              SELECT 1
              FROM zones z
              JOIN regions r ON r.id = z.region_id
              WHERE r.provider_id = $1
                AND z.code = $2
                AND z.is_active = true
            )
            "#,
        )
        .bind(provider_id)
        .bind(zone)
        .fetch_one(&self.db)
        .await
        .unwrap_or(false);

        if !zone_ok {
            return Err(anyhow::anyhow!("MockProvider: invalid zone '{}'", zone));
        }

        // Ensure instance type exists and is available in that zone
        let type_ok: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
              SELECT 1
              FROM instance_types it
              JOIN instance_type_zones itz ON itz.instance_type_id = it.id
              JOIN zones z ON z.id = itz.zone_id
              JOIN regions r ON r.id = z.region_id
              WHERE it.provider_id = $1
                AND it.code = $2
                AND it.is_active = true
                AND z.code = $3
                AND itz.is_available = true
            )
            "#,
        )
        .bind(provider_id)
        .bind(instance_type)
        .bind(zone)
        .fetch_one(&self.db)
        .await
        .unwrap_or(false);

        if !type_ok {
            return Err(anyhow::anyhow!(
                "MockProvider: invalid or unavailable instance_type '{}' in zone '{}'",
                instance_type,
                zone
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl CloudProvider for MockProvider {
    async fn create_instance(
        &self,
        zone: &str,
        instance_type: &str,
        _image_id: &str,
        _cloud_init: Option<&str>,
        _volumes: Option<&[String]>, // Optional list of volume IDs to attach at creation (ignored for mock)
    ) -> Result<String> {
        self.validate_zone_and_type(zone, instance_type).await?;

        let server_id = format!("mock-{}", uuid::Uuid::new_v4());

        // Resolve provider id again (cheap) to persist the mock instance row
        let provider_id: uuid::Uuid =
            sqlx::query_scalar("SELECT id FROM providers WHERE code = $1 LIMIT 1")
                .bind(self.provider_code)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "MockProvider: provider '{}' not found in DB",
                        self.provider_code
                    )
                })?;

        sqlx::query(
            r#"
            INSERT INTO mock_provider_instances (
              provider_instance_id, provider_id, zone_code, instance_type_code,
              status, ip_address, created_at, metadata
            )
            VALUES ($1, $2, $3, $4, 'created', NULL, NOW(), $5)
            "#,
        )
        .bind(&server_id)
        .bind(provider_id)
        .bind(zone)
        .bind(instance_type)
        .bind(serde_json::json!({"mock": true}))
        .execute(&self.db)
        .await?;

        Ok(server_id)
    }

    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        self.maybe_finalize_termination(zone, server_id).await?;

        // Resolve instance_id to start the Docker runtime
        let instance_id = self
            .resolve_instance_id(server_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Instance not found for server_id={}", server_id))?;

        // Start the Docker runtime (mock-vllm + worker-agent)
        let ip_address = self.start_runtime(instance_id, server_id).await?;

        // Update DB: mark as running and store IP
        let res = sqlx::query(
            r#"
            UPDATE mock_provider_instances
            SET status = 'running',
                started_at = COALESCE(started_at, NOW()),
                ip_address = $3::inet
            WHERE provider_instance_id = $1
              AND zone_code = $2
              AND status IN ('created', 'running')
            "#,
        )
        .bind(server_id)
        .bind(zone)
        .bind(&ip_address)
        .execute(&self.db)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        // Resolve instance_id to stop the Docker runtime
        if let Some(instance_id) = self.resolve_instance_id(server_id).await? {
            // Stop the Docker runtime (best-effort, don't fail if already stopped)
            let _ = self.stop_runtime(instance_id).await;
        }

        // Set terminating and schedule delete after a short delay (emulates provider async delete)
        let res = sqlx::query(
            r#"
            UPDATE mock_provider_instances
            SET status = 'terminating',
                termination_requested_at = COALESCE(termination_requested_at, NOW()),
                delete_after = COALESCE(delete_after, NOW() + INTERVAL '15 seconds')
            WHERE provider_instance_id = $1
              AND zone_code = $2
              AND status IN ('created', 'running', 'terminating')
            "#,
        )
        .bind(server_id)
        .bind(zone)
        .execute(&self.db)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    async fn get_instance_ip(&self, zone: &str, server_id: &str) -> Result<Option<String>> {
        // Try to get IP from DB first (set when runtime was started)
        let ip_from_db: Option<String> = sqlx::query_scalar(
            r#"
            SELECT ip_address::text
            FROM mock_provider_instances
            WHERE provider_instance_id = $1 AND zone_code = $2
            "#,
        )
        .bind(server_id)
        .bind(zone)
        .fetch_optional(&self.db)
        .await?;

        if let Some(ip) = ip_from_db {
            if !ip.is_empty() {
                return Ok(Some(ip));
            }
        }

        // Fallback: try to get IP from running Docker container
        if let Some(instance_id) = self.resolve_instance_id(server_id).await? {
            let id12 = instance_id.to_string().replace('-', "").chars().take(12).collect::<String>();
            let project_name = format!("mockrt-{}", id12);
            let container_name = format!("{}-mock-vllm-1", project_name);

            let ip_output = Command::new("docker")
                .args([
                    "inspect",
                    "--format",
                    "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
                    &container_name,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .await
                .ok();

            if let Some(output) = ip_output {
                if output.status.success() {
                    let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !ip.is_empty() {
                        return Ok(Some(ip));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn check_instance_exists(&self, zone: &str, server_id: &str) -> Result<bool> {
        self.maybe_finalize_termination(zone, server_id).await?;

        let status: Option<String> = sqlx::query_scalar(
            r#"
            SELECT status
            FROM mock_provider_instances
            WHERE provider_instance_id = $1 AND zone_code = $2
            "#,
        )
        .bind(server_id)
        .bind(zone)
        .fetch_optional(&self.db)
        .await?;

        Ok(status.is_some() && status.as_deref() != Some("terminated"))
    }

    async fn fetch_catalog(&self, _zone: &str) -> Result<Vec<inventory::CatalogItem>> {
        // Catalog is seeded in DB for mock, so we return empty here.
        Ok(vec![])
    }

    async fn list_instances(&self, zone: &str) -> Result<Vec<inventory::DiscoveredInstance>> {
        // Best-effort listing from the DB table.
        let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT provider_instance_id, status, ip_address::text, created_at::text
            FROM mock_provider_instances
            WHERE zone_code = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(zone)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|(pid, status, ip, created_at)| inventory::DiscoveredInstance {
                provider_id: pid.clone(),
                name: pid,
                zone: zone.to_string(),
                status,
                ip_address: ip,
                created_at,
            })
            .collect())
    }

    async fn list_attached_volumes(
        &self,
        _zone: &str,
        _server_id: &str,
    ) -> Result<Vec<inventory::AttachedVolume>> {
        // Mock provider doesn't track volumes separately - they're part of the Docker runtime
        // Return empty list as volumes are managed by Docker Compose
        Ok(vec![])
    }

    async fn delete_volume(&self, _zone: &str, _volume_id: &str) -> Result<bool> {
        // Mock provider doesn't have separate volumes - they're part of Docker runtime
        // Volumes are cleaned up when the runtime is stopped
        Ok(true)
    }

    async fn check_volume_exists(&self, _zone: &str, _volume_id: &str) -> Result<bool> {
        // Mock provider doesn't track volumes separately
        // Always return false as volumes don't exist independently
        Ok(false)
    }
}


