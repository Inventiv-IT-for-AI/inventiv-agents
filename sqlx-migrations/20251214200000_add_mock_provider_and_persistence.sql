-- Add a Mock provider for local/dev testing
--
-- Goals:
-- - Provide a provider implementation with real persistence, but no external HTTPS/SSH calls
-- - Support regions/zones/instance types
-- - Persist provider-side instance lifecycle (created/running/terminating/terminated)
--
-- Migration: 20251214200000_add_mock_provider_and_persistence.sql

-- Note: catalog seeding (providers/regions/zones/instance_types/instance_type_zones)
-- is intentionally NOT done in migrations anymore.
-- Use `seeds/catalog_seeds.sql` to initialize catalog data in dev.

-- Provider-side persistence for mock instances
CREATE TABLE IF NOT EXISTS mock_provider_instances (
  provider_instance_id TEXT PRIMARY KEY,
  provider_id UUID NOT NULL REFERENCES providers(id),
  zone_code TEXT NOT NULL,
  instance_type_code TEXT NOT NULL,

  status TEXT NOT NULL, -- created | running | terminating | terminated
  ip_address INET,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  started_at TIMESTAMPTZ,
  termination_requested_at TIMESTAMPTZ,
  delete_after TIMESTAMPTZ,
  terminated_at TIMESTAMPTZ,

  metadata JSONB
);

CREATE INDEX IF NOT EXISTS idx_mock_provider_instances_zone
  ON mock_provider_instances(zone_code);

CREATE INDEX IF NOT EXISTS idx_mock_provider_instances_status
  ON mock_provider_instances(status);

-- For deterministic IP allocation (optional). We'll use it in the mock provider.
CREATE SEQUENCE IF NOT EXISTS mock_provider_ip_seq;
