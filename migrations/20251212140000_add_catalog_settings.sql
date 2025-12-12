-- Add Settings Columns to Catalog Tables

-- 1. Regions
ALTER TABLE regions ADD COLUMN code VARCHAR(50);
ALTER TABLE regions ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
-- Backfill code from name if null (simple strategy for existing data)
UPDATE regions SET code = name WHERE code IS NULL;
ALTER TABLE regions ALTER COLUMN code SET NOT NULL;
ALTER TABLE regions ADD CONSTRAINT regions_provider_code_key UNIQUE (provider_id, code);

-- 2. Zones
ALTER TABLE zones ADD COLUMN code VARCHAR(50);
ALTER TABLE zones ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE zones SET code = name WHERE code IS NULL;
ALTER TABLE zones ALTER COLUMN code SET NOT NULL;
ALTER TABLE zones ADD CONSTRAINT zones_region_code_key UNIQUE (region_id, code);

-- 3. Instance Types
ALTER TABLE instance_types ADD COLUMN code VARCHAR(50);
ALTER TABLE instance_types ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE instance_types SET code = name WHERE code IS NULL;
ALTER TABLE instance_types ALTER COLUMN code SET NOT NULL;
ALTER TABLE instance_types ADD CONSTRAINT instance_types_provider_code_key UNIQUE (provider_id, code);
