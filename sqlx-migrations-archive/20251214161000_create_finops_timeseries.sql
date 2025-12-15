-- FinOps time-series (minute granularity)
-- Migration: 20251214161000_create_finops_timeseries.sql

CREATE SCHEMA IF NOT EXISTS finops;

-- 1) Forecast (burn-rate): what current allocation implies as spend
-- Stored per minute for: total + per provider
CREATE TABLE IF NOT EXISTS finops.cost_forecast_minute (
  bucket_minute TIMESTAMPTZ NOT NULL,
  provider_id UUID NULL REFERENCES providers(id),

  -- Current burn-rate snapshot at this minute
  burn_rate_usd_per_hour NUMERIC(14, 6) NOT NULL,

  -- Convenience projections computed from burn_rate
  forecast_usd_per_minute NUMERIC(14, 6) NOT NULL,
  forecast_usd_per_day NUMERIC(14, 6) NOT NULL,
  forecast_usd_per_month_30d NUMERIC(14, 6) NOT NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (bucket_minute, provider_id)
);

CREATE INDEX IF NOT EXISTS idx_finops_cost_forecast_minute_bucket
  ON finops.cost_forecast_minute(bucket_minute);

-- 2) Actual cost consumed per minute (from provider costs events)
-- Stored per minute for: total + per provider + per instance
CREATE TABLE IF NOT EXISTS finops.cost_actual_minute (
  bucket_minute TIMESTAMPTZ NOT NULL,
  provider_id UUID NULL REFERENCES providers(id),
  instance_id UUID NULL REFERENCES instances(id),

  amount_usd NUMERIC(14, 6) NOT NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (bucket_minute, provider_id, instance_id)
);

CREATE INDEX IF NOT EXISTS idx_finops_cost_actual_minute_bucket
  ON finops.cost_actual_minute(bucket_minute);

CREATE INDEX IF NOT EXISTS idx_finops_cost_actual_minute_instance_bucket
  ON finops.cost_actual_minute(instance_id, bucket_minute)
  WHERE instance_id IS NOT NULL;

-- 3) Actual cost cumulative (running sum) per minute
-- Stored per minute for: total + per provider + per instance
CREATE TABLE IF NOT EXISTS finops.cost_actual_cumulative_minute (
  bucket_minute TIMESTAMPTZ NOT NULL,
  provider_id UUID NULL REFERENCES providers(id),
  instance_id UUID NULL REFERENCES instances(id),

  cumulative_amount_usd NUMERIC(18, 6) NOT NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (bucket_minute, provider_id, instance_id)
);

CREATE INDEX IF NOT EXISTS idx_finops_cost_actual_cumulative_minute_bucket
  ON finops.cost_actual_cumulative_minute(bucket_minute);
