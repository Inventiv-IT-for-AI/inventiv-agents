-- Fix FinOps time-series primary keys to support NULL provider_id / instance_id.
--
-- Problem:
-- - Postgres PRIMARY KEY columns are implicitly NOT NULL.
-- - We want provider_id / instance_id to be nullable so we can store "TOTAL" rows.
--
-- Solution:
-- - Add generated surrogate key columns provider_id_key / instance_id_key using a NULL sentinel UUID.
-- - Recreate primary keys on (bucket_minute, provider_id_key[, instance_id_key]).
--
-- Migration: 20251214162500_fix_finops_timeseries_nullable_keys.sql

CREATE SCHEMA IF NOT EXISTS finops;

-- Sentinel UUID used for "TOTAL" rollups when provider_id/instance_id is NULL
-- (keeps PK deterministic and unique)
DO $$
BEGIN
  -- cost_forecast_minute
  ALTER TABLE finops.cost_forecast_minute
    ADD COLUMN IF NOT EXISTS provider_id_key UUID
      GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

  ALTER TABLE finops.cost_forecast_minute
    DROP CONSTRAINT IF EXISTS cost_forecast_minute_pkey;

  -- Recreate PK on surrogate key
  ALTER TABLE finops.cost_forecast_minute
    ADD CONSTRAINT cost_forecast_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key);

  CREATE INDEX IF NOT EXISTS idx_finops_cost_forecast_minute_provider_key
    ON finops.cost_forecast_minute(provider_id_key);

  -- cost_actual_minute
  ALTER TABLE finops.cost_actual_minute
    ADD COLUMN IF NOT EXISTS provider_id_key UUID
      GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

  ALTER TABLE finops.cost_actual_minute
    ADD COLUMN IF NOT EXISTS instance_id_key UUID
      GENERATED ALWAYS AS (COALESCE(instance_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

  ALTER TABLE finops.cost_actual_minute
    DROP CONSTRAINT IF EXISTS cost_actual_minute_pkey;

  ALTER TABLE finops.cost_actual_minute
    ADD CONSTRAINT cost_actual_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key, instance_id_key);

  CREATE INDEX IF NOT EXISTS idx_finops_cost_actual_minute_provider_key
    ON finops.cost_actual_minute(provider_id_key);

  -- cost_actual_cumulative_minute
  ALTER TABLE finops.cost_actual_cumulative_minute
    ADD COLUMN IF NOT EXISTS provider_id_key UUID
      GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

  ALTER TABLE finops.cost_actual_cumulative_minute
    ADD COLUMN IF NOT EXISTS instance_id_key UUID
      GENERATED ALWAYS AS (COALESCE(instance_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

  ALTER TABLE finops.cost_actual_cumulative_minute
    DROP CONSTRAINT IF EXISTS cost_actual_cumulative_minute_pkey;

  ALTER TABLE finops.cost_actual_cumulative_minute
    ADD CONSTRAINT cost_actual_cumulative_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key, instance_id_key);

  CREATE INDEX IF NOT EXISTS idx_finops_cost_actual_cumulative_provider_key
    ON finops.cost_actual_cumulative_minute(provider_id_key);
END $$;
