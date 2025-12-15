-- Add extra forecast horizons for FinOps dashboard
-- 1h + 365d

ALTER TABLE finops.cost_forecast_minute
  ADD COLUMN IF NOT EXISTS forecast_eur_per_hour NUMERIC(14,6) NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS forecast_eur_per_year_365d NUMERIC(14,6) NOT NULL DEFAULT 0;

-- Backfill from burn_rate (will be renamed to EUR by later migration)
UPDATE finops.cost_forecast_minute
SET forecast_eur_per_hour = burn_rate_eur_per_hour,
    forecast_eur_per_year_365d = burn_rate_eur_per_hour * 24 * 365
WHERE forecast_eur_per_hour = 0
   OR forecast_eur_per_year_365d = 0;
