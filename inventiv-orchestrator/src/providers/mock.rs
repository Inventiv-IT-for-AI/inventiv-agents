use async_trait::async_trait;
use anyhow::Result;
use crate::provider::{CloudProvider, inventory};
use sqlx::{Pool, Postgres};

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

    async fn validate_zone_and_type(&self, zone: &str, instance_type: &str) -> Result<()> {
        // Resolve provider id from code (no hardcoded UUIDs)
        let provider_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM providers WHERE code = $1 LIMIT 1")
            .bind(self.provider_code)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("MockProvider: provider '{}' not found in DB", self.provider_code))?;

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
    async fn create_instance(&self, zone: &str, instance_type: &str, _image_id: &str) -> Result<String> {
        self.validate_zone_and_type(zone, instance_type).await?;

        let server_id = format!("mock-{}", uuid::Uuid::new_v4());

        // Allocate a deterministic IP using a DB sequence
        let seq: i64 = sqlx::query_scalar("SELECT nextval('mock_provider_ip_seq')")
            .fetch_one(&self.db)
            .await
            .unwrap_or(1);
        let last_octet = ((seq % 250) + 1) as i64;
        let third_octet = (((seq / 250) % 250) + 1) as i64;
        let ip = format!("10.{}.{}.{}", 10, third_octet, last_octet);

        // Resolve provider id again (cheap) to persist the mock instance row
        let provider_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM providers WHERE code = $1 LIMIT 1")
            .bind(self.provider_code)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("MockProvider: provider '{}' not found in DB", self.provider_code))?;

        sqlx::query(
            r#"
            INSERT INTO mock_provider_instances (
              provider_instance_id, provider_id, zone_code, instance_type_code,
              status, ip_address, created_at, metadata
            )
            VALUES ($1, $2, $3, $4, 'created', $5::inet, NOW(), $6)
            "#,
        )
        .bind(&server_id)
        .bind(provider_id)
        .bind(zone)
        .bind(instance_type)
        .bind(&ip)
        .bind(serde_json::json!({"mock": true}))
        .execute(&self.db)
        .await?;

        Ok(server_id)
    }

    async fn start_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
        self.maybe_finalize_termination(zone, server_id).await?;

        let res = sqlx::query(
            r#"
            UPDATE mock_provider_instances
            SET status = 'running',
                started_at = COALESCE(started_at, NOW())
            WHERE provider_instance_id = $1
              AND zone_code = $2
              AND status IN ('created', 'running')
            "#,
        )
        .bind(server_id)
        .bind(zone)
        .execute(&self.db)
        .await?;

        Ok(res.rows_affected() > 0)
    }

    async fn terminate_instance(&self, zone: &str, server_id: &str) -> Result<bool> {
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
        self.maybe_finalize_termination(zone, server_id).await?;

        let row: Option<(Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT ip_address::text, status
            FROM mock_provider_instances
            WHERE provider_instance_id = $1 AND zone_code = $2
            "#,
        )
        .bind(server_id)
        .bind(zone)
        .fetch_optional(&self.db)
        .await?;

        let Some((ip, status)) = row else {
            return Ok(None);
        };

        if status == "terminated" {
            return Ok(None);
        }

        Ok(ip)
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

        Ok(matches!(status.as_deref(), Some("created" | "running" | "terminating")))
    }

    async fn fetch_catalog(&self, _zone: &str) -> Result<Vec<inventory::CatalogItem>> {
        // Return a small static catalog; orchestrator will persist it into instance_types.
        Ok(vec![
            inventory::CatalogItem {
                name: "MOCK-GPU-S".to_string(),
                code: "MOCK-GPU-S".to_string(),
                cost_per_hour: 0.2500,
                cpu_count: 8,
                ram_gb: 32,
                gpu_count: 1,
                vram_per_gpu_gb: 24,
                bandwidth_bps: 1_000_000_000,
            },
            inventory::CatalogItem {
                name: "MOCK-4GPU-M".to_string(),
                code: "MOCK-4GPU-M".to_string(),
                cost_per_hour: 0.7500,
                cpu_count: 16,
                ram_gb: 64,
                gpu_count: 4,
                vram_per_gpu_gb: 48,
                bandwidth_bps: 2_000_000_000,
            },
        ])
    }

    async fn list_instances(&self, zone: &str) -> Result<Vec<inventory::DiscoveredInstance>> {
        // Expose only non-terminated instances, like a real provider list endpoint.
        let rows: Vec<(String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT provider_instance_id,
                   provider_instance_id as name,
                   status,
                   ip_address::text,
                   created_at::text
            FROM mock_provider_instances
            WHERE zone_code = $1
              AND status <> 'terminated'
            ORDER BY created_at DESC
            "#,
        )
        .bind(zone)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|(id, name, status, ip, created_at)| inventory::DiscoveredInstance {
                provider_id: id,
                name,
                zone: zone.to_string(),
                status,
                ip_address: ip,
                created_at,
            })
            .collect())
    }

    async fn create_volume(
        &self,
        _zone: &str,
        _name: &str,
        _size_bytes: i64,
        _volume_type: &str,
        _perf_iops: Option<i32>,
    ) -> Result<Option<String>> {
        Ok(Some(format!("mock-vol-{}", uuid::Uuid::new_v4())))
    }

    async fn attach_volume(&self, _zone: &str, _server_id: &str, _volume_id: &str) -> Result<bool> {
        Ok(true)
    }

    async fn delete_volume(&self, _zone: &str, _volume_id: &str) -> Result<bool> {
        Ok(true)
    }
}

