-- Switch FinOps monetary fields from USD to EUR naming.
-- We keep storage numeric values; only column names + defaults change.
-- Idempotent via DO blocks.

DO $$
BEGIN
  -- finops.cost_actual_minute.amount_usd -> amount_eur
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_actual_minute' AND column_name='amount_usd'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_actual_minute' AND column_name='amount_eur'
  ) THEN
    EXECUTE 'ALTER TABLE finops.cost_actual_minute RENAME COLUMN amount_usd TO amount_eur';
  END IF;

  -- finops.cost_actual_cumulative_minute.cumulative_amount_usd -> cumulative_amount_eur
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_actual_cumulative_minute' AND column_name='cumulative_amount_usd'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_actual_cumulative_minute' AND column_name='cumulative_amount_eur'
  ) THEN
    EXECUTE 'ALTER TABLE finops.cost_actual_cumulative_minute RENAME COLUMN cumulative_amount_usd TO cumulative_amount_eur';
  END IF;

  -- finops.cost_forecast_minute: burn_rate + forecasts
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='burn_rate_usd_per_hour'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='burn_rate_eur_per_hour'
  ) THEN
    EXECUTE 'ALTER TABLE finops.cost_forecast_minute RENAME COLUMN burn_rate_usd_per_hour TO burn_rate_eur_per_hour';
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_usd_per_minute'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_eur_per_minute'
  ) THEN
    EXECUTE 'ALTER TABLE finops.cost_forecast_minute RENAME COLUMN forecast_usd_per_minute TO forecast_eur_per_minute';
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_usd_per_day'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_eur_per_day'
  ) THEN
    EXECUTE 'ALTER TABLE finops.cost_forecast_minute RENAME COLUMN forecast_usd_per_day TO forecast_eur_per_day';
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_usd_per_month_30d'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_eur_per_month_30d'
  ) THEN
    EXECUTE 'ALTER TABLE finops.cost_forecast_minute RENAME COLUMN forecast_usd_per_month_30d TO forecast_eur_per_month_30d';
  END IF;

  -- Optional horizons (if created)
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_usd_per_hour'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_eur_per_hour'
  ) THEN
    EXECUTE 'ALTER TABLE finops.cost_forecast_minute RENAME COLUMN forecast_usd_per_hour TO forecast_eur_per_hour';
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_usd_per_year_365d'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='cost_forecast_minute' AND column_name='forecast_eur_per_year_365d'
  ) THEN
    EXECUTE 'ALTER TABLE finops.cost_forecast_minute RENAME COLUMN forecast_usd_per_year_365d TO forecast_eur_per_year_365d';
  END IF;

  -- provider_costs
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='provider_costs' AND column_name='amount_usd'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='provider_costs' AND column_name='amount_eur'
  ) THEN
    EXECUTE 'ALTER TABLE finops.provider_costs RENAME COLUMN amount_usd TO amount_eur';
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='provider_costs' AND column_name='currency'
  ) THEN
    EXECUTE 'ALTER TABLE finops.provider_costs ALTER COLUMN currency SET DEFAULT ''EUR''';
  END IF;

  -- subscription_charges
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='subscription_charges' AND column_name='amount_usd'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='subscription_charges' AND column_name='amount_eur'
  ) THEN
    EXECUTE 'ALTER TABLE finops.subscription_charges RENAME COLUMN amount_usd TO amount_eur';
  END IF;

  -- token_purchases
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='token_purchases' AND column_name='amount_usd'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='token_purchases' AND column_name='amount_eur'
  ) THEN
    EXECUTE 'ALTER TABLE finops.token_purchases RENAME COLUMN amount_usd TO amount_eur';
  END IF;

  -- inference_usage
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='inference_usage' AND column_name='unit_price_usd_per_1k_tokens'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='inference_usage' AND column_name='unit_price_eur_per_1k_tokens'
  ) THEN
    EXECUTE 'ALTER TABLE finops.inference_usage RENAME COLUMN unit_price_usd_per_1k_tokens TO unit_price_eur_per_1k_tokens';
  END IF;

  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='inference_usage' AND column_name='charged_amount_usd'
  ) AND NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema='finops' AND table_name='inference_usage' AND column_name='charged_amount_eur'
  ) THEN
    EXECUTE 'ALTER TABLE finops.inference_usage RENAME COLUMN charged_amount_usd TO charged_amount_eur';
  END IF;
END $$;
