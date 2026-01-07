use sqlx::{Pool, Postgres};

#[allow(dead_code)]
pub async fn run_inline_migrations(pool: &Pool<Postgres>) {
    println!("📦 Running Migrations (Inline Schema)...");

    // Minimal Schema for Orchestrator to work (Instances Table)
    let schema_sql = r#"
        CREATE TYPE instance_status AS ENUM (
            'provisioning', 'booting', 'installing', 'starting', 'unavailable', 'ready', 'draining', 'terminated', 'failed', 'startup_failed', 'terminating', 'provisioning_failed', 'archived'
        );
        CREATE TABLE IF NOT EXISTS providers (
            id UUID PRIMARY KEY,
            name VARCHAR(50) UNIQUE NOT NULL,
            description TEXT,
            created_at TIMESTAMPTZ DEFAULT NOW()
        );
        CREATE TABLE IF NOT EXISTS regions (
            id UUID PRIMARY KEY,
            provider_id UUID NOT NULL,
            name VARCHAR(50) NOT NULL,
            UNIQUE(provider_id, name)
        );
        CREATE TABLE IF NOT EXISTS zones (
            id UUID PRIMARY KEY,
            region_id UUID NOT NULL,
            name VARCHAR(50) NOT NULL,
            UNIQUE(region_id, name)
        );
        CREATE TABLE IF NOT EXISTS instance_types (
            id UUID PRIMARY KEY,
            provider_id UUID NOT NULL,
            name VARCHAR(50) NOT NULL,
            gpu_count INTEGER NOT NULL,
            vram_per_gpu_gb INTEGER NOT NULL,
            UNIQUE(provider_id, name)
        );
        CREATE TABLE IF NOT EXISTS instances (
            id UUID PRIMARY KEY,
            provider_id UUID NOT NULL,
            zone_id UUID NOT NULL,
            instance_type_id UUID NOT NULL,
            model_id UUID,
            provider_instance_id VARCHAR(255),
            ip_address INET,
            api_key VARCHAR(255),
            status instance_status NOT NULL DEFAULT 'provisioning',
            created_at TIMESTAMPTZ DEFAULT NOW(),
            terminated_at TIMESTAMPTZ,
            gpu_profile JSONB NOT NULL
        );
    "#;

    // Execute schema
    for statement in schema_sql.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() {
            let _ = sqlx::query(stmt).execute(pool).await;
        }
    }

    let db_updates = vec![
        // Providers
        r#"ALTER TABLE providers ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE"#,
        r#"ALTER TABLE providers ADD COLUMN IF NOT EXISTS code VARCHAR(50)"#,
        // Backfill code from name if null
        r#"UPDATE providers SET code = name WHERE code IS NULL"#,
        // Regions
        r#"ALTER TABLE regions ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE"#,
        r#"ALTER TABLE regions ADD COLUMN IF NOT EXISTS code VARCHAR(50)"#,
        // Zones
        r#"ALTER TABLE zones ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE"#,
        r#"ALTER TABLE zones ADD COLUMN IF NOT EXISTS code VARCHAR(50)"#,
        // Instance Types
        r#"ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE"#,
        r#"ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS code VARCHAR(50)"#,
        r#"ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS cost_per_hour NUMERIC(10,4)"#,
        r#"ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS cpu_count INTEGER"#,
        r#"ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS ram_gb INTEGER"#,
        r#"ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS bandwidth_bps BIGINT"#,
    ];

    for stmt in db_updates {
        // "ADD COLUMN IF NOT EXISTS" works in Postgres 9.6+. Assuming user has modern PG.
        let _ = sqlx::query(stmt).execute(pool).await;
    }
    println!("✅ Migrations (Inline) Applied");
}
