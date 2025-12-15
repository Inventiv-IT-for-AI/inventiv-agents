-- Fix FinOps time-series primary keys to allow NULL dimensions
-- Postgres primary key columns are implicitly NOT NULL, which prevented storing TOTAL rows (provider_id NULL, instance_id NULL).
-- This migration introduces generated key columns using COALESCE(..., 00000000-0000-0000-0000-000000000000)
-- and moves primary keys to these non-null generated columns.
-- Migration: 20251214163000_fix_finops_timeseries_nullable_keys.sql

CREATE SCHEMA IF NOT EXISTS finops;

-- Sentinel UUID for "TOTAL" dimensions
-- (kept only in *_key columns; the actual provider_id/instance_id remain nullable)

-- 1) Forecast
ALTER TABLE finops.cost_forecast_minute
  DROP CONSTRAINT IF EXISTS cost_forecast_minute_pkey;

ALTER TABLE finops.cost_forecast_minute
  ADD COLUMN IF NOT EXISTS provider_id_key UUID
    GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

ALTER TABLE finops.cost_forecast_minute
  ADD CONSTRAINT cost_forecast_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key);

CREATE INDEX IF NOT EXISTS idx_finops_cost_forecast_minute_provider_key
  ON finops.cost_forecast_minute(provider_id_key);

-- 2) Actual minute
ALTER TABLE finops.cost_actual_minute
  DROP CONSTRAINT IF EXISTS cost_actual_minute_pkey;

ALTER TABLE finops.cost_actual_minute
  ADD COLUMN IF NOT EXISTS provider_id_key UUID
    GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

ALTER TABLE finops.cost_actual_minute
  ADD COLUMN IF NOT EXISTS instance_id_key UUID
    GENERATED ALWAYS AS (COALESCE(instance_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

ALTER TABLE finops.cost_actual_minute
  ADD CONSTRAINT cost_actual_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key, instance_id_key);

CREATE INDEX IF NOT EXISTS idx_finops_cost_actual_minute_provider_key
  ON finops.cost_actual_minute(provider_id_key);

-- 3) Cumulative minute
ALTER TABLE finops.cost_actual_cumulative_minute
  DROP CONSTRAINT IF EXISTS cost_actual_cumulative_minute_pkey;

ALTER TABLE finops.cost_actual_cumulative_minute
  ADD COLUMN IF NOT EXISTS provider_id_key UUID
    GENERATED ALWAYS AS (COALESCE(provider_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

ALTER TABLE finops.cost_actual_cumulative_minute
  ADD COLUMN IF NOT EXISTS instance_id_key UUID
    GENERATED ALWAYS AS (COALESCE(instance_id, '00000000-0000-0000-0000-000000000000'::uuid)) STORED;

ALTER TABLE finops.cost_actual_cumulative_minute
  ADD CONSTRAINT cost_actual_cumulative_minute_pkey PRIMARY KEY (bucket_minute, provider_id_key, instance_id_key);

CREATE INDEX IF NOT EXISTS idx_finops_cost_actual_cumulative_provider_key
  ON finops.cost_actual_cumulative_minute(provider_id_key);
