-- Allow NULL dimension columns in FinOps time-series tables.
-- Dropping primary keys removed constraint, but Postgres keeps NOT NULL flags that PK previously set.
-- Migration: 20251214164000_drop_not_null_finops_dimensions.sql

ALTER TABLE finops.cost_forecast_minute
  ALTER COLUMN provider_id DROP NOT NULL;

ALTER TABLE finops.cost_actual_minute
  ALTER COLUMN provider_id DROP NOT NULL,
  ALTER COLUMN instance_id DROP NOT NULL;

ALTER TABLE finops.cost_actual_cumulative_minute
  ALTER COLUMN provider_id DROP NOT NULL,
  ALTER COLUMN instance_id DROP NOT NULL;
