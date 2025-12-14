-- Add Settings Columns to Catalog Tables

-- 1. Regions
ALTER TABLE regions ADD COLUMN IF NOT EXISTS code VARCHAR(50);
ALTER TABLE regions ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE;
-- Backfill code from name if null (simple strategy for existing data)
UPDATE regions SET code = name WHERE code IS NULL;
ALTER TABLE regions ALTER COLUMN code SET NOT NULL;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'regions_provider_code_key') THEN
        ALTER TABLE regions ADD CONSTRAINT regions_provider_code_key UNIQUE (provider_id, code);
    END IF;
END $$;

-- 2. Zones
ALTER TABLE zones ADD COLUMN IF NOT EXISTS code VARCHAR(50);
ALTER TABLE zones ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE zones SET code = name WHERE code IS NULL;
ALTER TABLE zones ALTER COLUMN code SET NOT NULL;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'zones_region_code_key') THEN
        ALTER TABLE zones ADD CONSTRAINT zones_region_code_key UNIQUE (region_id, code);
    END IF;
END $$;

-- 3. Instance Types
ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS code VARCHAR(50);
ALTER TABLE instance_types ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE instance_types SET code = name WHERE code IS NULL;
ALTER TABLE instance_types ALTER COLUMN code SET NOT NULL;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'instance_types_provider_code_key') THEN
        ALTER TABLE instance_types ADD CONSTRAINT instance_types_provider_code_key UNIQUE (provider_id, code);
    END IF;
END $$;


