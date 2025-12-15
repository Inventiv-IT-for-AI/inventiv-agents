-- Add provider "code" for stable references (seed-friendly)
-- Canonical examples: 'scaleway', 'aws', 'mock'

ALTER TABLE providers
  ADD COLUMN IF NOT EXISTS code VARCHAR(50);

-- Backfill from name if missing (lowercase, snake-ish)
UPDATE providers
SET code = lower(regexp_replace(name, '[^a-zA-Z0-9]+', '_', 'g'))
WHERE code IS NULL;

-- Enforce not null (after backfill)
ALTER TABLE providers
  ALTER COLUMN code SET NOT NULL;

-- Ensure uniqueness (idempotent)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'providers_code_key') THEN
        ALTER TABLE providers ADD CONSTRAINT providers_code_key UNIQUE (code);
    END IF;
END $$;

