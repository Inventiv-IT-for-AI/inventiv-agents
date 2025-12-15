-- Required extensions (for gen_random_uuid() used by later migrations)
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- 1. Enum Type (idempotent)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'instance_status') THEN
        CREATE TYPE instance_status AS ENUM (
            'provisioning',
            'booting',
            'ready',
            'draining',
            'terminated',
            'failed'
        );
    END IF;
END $$;

-- 2. Users (Admins)
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL DEFAULT 'admin',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 3. Providers Catalog
CREATE TABLE IF NOT EXISTS providers (
    id UUID PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL, -- "scaleway", "aws"
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS regions (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES providers(id),
    name VARCHAR(50) NOT NULL, -- "fr-par"
    UNIQUE(provider_id, name)
);

CREATE TABLE IF NOT EXISTS zones (
    id UUID PRIMARY KEY,
    region_id UUID NOT NULL REFERENCES regions(id),
    name VARCHAR(50) NOT NULL, -- "fr-par-1"
    UNIQUE(region_id, name)
);

CREATE TABLE IF NOT EXISTS instance_types (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES providers(id),
    name VARCHAR(50) NOT NULL, -- "H100-1-80G"
    gpu_count INTEGER NOT NULL,
    vram_per_gpu_gb INTEGER NOT NULL,
    UNIQUE(provider_id, name)
);

CREATE TABLE IF NOT EXISTS instance_availability (
    instance_type_id UUID NOT NULL REFERENCES instance_types(id),
    zone_id UUID NOT NULL REFERENCES zones(id),
    PRIMARY KEY(instance_type_id, zone_id)
);

-- 4. SSH Keys
CREATE TABLE IF NOT EXISTS ssh_keys (
    id UUID PRIMARY KEY,
    name VARCHAR(50) NOT NULL,
    public_key TEXT NOT NULL,
    provider_id UUID NOT NULL REFERENCES providers(id),
    provider_key_id VARCHAR(255), -- ID remote chez le provider
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- 5. Models Catalog
CREATE TABLE IF NOT EXISTS models (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    model_id VARCHAR(255) UNIQUE NOT NULL, -- HuggingFace ID
    required_vram_gb INTEGER NOT NULL,
    context_length INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 6. Instances Inventory
CREATE TABLE IF NOT EXISTS instances (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES providers(id),
    zone_id UUID NOT NULL REFERENCES zones(id),
    instance_type_id UUID NOT NULL REFERENCES instance_types(id),
    model_id UUID REFERENCES models(id),

    provider_instance_id VARCHAR(255),  -- ID distant
    ip_address INET,
    api_key VARCHAR(255), -- Key to call the worker securely

    status instance_status NOT NULL DEFAULT 'provisioning',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    terminated_at TIMESTAMPTZ,
    gpu_profile JSONB NOT NULL -- Snapshot des specs
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_instances_status ON instances(status);
CREATE INDEX IF NOT EXISTS idx_instances_model_id ON instances(model_id);


