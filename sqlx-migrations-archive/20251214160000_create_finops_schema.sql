-- FinOps schema: costs + revenues + usage metering
-- Migration: 20251214160000_create_finops_schema.sql

CREATE SCHEMA IF NOT EXISTS finops;

-- Customers / Tenants (who pay)
CREATE TABLE IF NOT EXISTS finops.customers (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  external_ref TEXT,
  name TEXT,
  email TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (external_ref),
  UNIQUE (email)
);

-- API keys used by customers to call inference
-- Note: store only hash/prefix, never store raw keys.
CREATE TABLE IF NOT EXISTS finops.api_keys (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  customer_id UUID NOT NULL REFERENCES finops.customers(id) ON DELETE CASCADE,
  key_prefix TEXT NOT NULL,
  key_hash TEXT NOT NULL,
  label TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  revoked_at TIMESTAMPTZ,
  UNIQUE (key_hash)
);

CREATE INDEX IF NOT EXISTS idx_finops_api_keys_customer
  ON finops.api_keys(customer_id);

-- Provider costs, near real-time, from providers billing APIs
CREATE TABLE IF NOT EXISTS finops.provider_costs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  occurred_at TIMESTAMPTZ NOT NULL,

  provider_id UUID REFERENCES providers(id),
  region_id UUID REFERENCES regions(id),
  zone_id UUID REFERENCES zones(id),

  -- Link to an instance when possible
  instance_id UUID REFERENCES instances(id),

  resource_type TEXT NOT NULL, -- e.g. "instance", "storage", "network"
  resource_id TEXT,           -- provider-side identifier

  amount_usd NUMERIC(14, 6) NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',

  external_id TEXT,           -- provider invoice line id, etc.
  metadata JSONB,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  UNIQUE (provider_id, external_id)
);

CREATE INDEX IF NOT EXISTS idx_finops_provider_costs_time
  ON finops.provider_costs(occurred_at);

CREATE INDEX IF NOT EXISTS idx_finops_provider_costs_instance_time
  ON finops.provider_costs(instance_id, occurred_at)
  WHERE instance_id IS NOT NULL;

-- Token purchases (revenue)
CREATE TABLE IF NOT EXISTS finops.token_purchases (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  customer_id UUID NOT NULL REFERENCES finops.customers(id) ON DELETE CASCADE,
  api_key_id UUID REFERENCES finops.api_keys(id) ON DELETE SET NULL,

  purchased_at TIMESTAMPTZ NOT NULL,
  tokens BIGINT NOT NULL,
  amount_usd NUMERIC(14, 6) NOT NULL,

  external_id TEXT,
  metadata JSONB,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  UNIQUE (external_id)
);

CREATE INDEX IF NOT EXISTS idx_finops_token_purchases_time
  ON finops.token_purchases(purchased_at);

CREATE INDEX IF NOT EXISTS idx_finops_token_purchases_customer
  ON finops.token_purchases(customer_id, purchased_at);

-- Subscription charges (revenue) - monthly plans
CREATE TABLE IF NOT EXISTS finops.subscription_charges (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  customer_id UUID NOT NULL REFERENCES finops.customers(id) ON DELETE CASCADE,

  charged_at TIMESTAMPTZ NOT NULL,
  period_start TIMESTAMPTZ,
  period_end TIMESTAMPTZ,

  plan_code TEXT,
  amount_usd NUMERIC(14, 6) NOT NULL,

  external_id TEXT,
  metadata JSONB,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  UNIQUE (external_id)
);

CREATE INDEX IF NOT EXISTS idx_finops_subscription_charges_time
  ON finops.subscription_charges(charged_at);

CREATE INDEX IF NOT EXISTS idx_finops_subscription_charges_customer
  ON finops.subscription_charges(customer_id, charged_at);

-- Inference usage metering (optional but useful for precision)
CREATE TABLE IF NOT EXISTS finops.inference_usage (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  occurred_at TIMESTAMPTZ NOT NULL,

  customer_id UUID REFERENCES finops.customers(id) ON DELETE SET NULL,
  api_key_id UUID REFERENCES finops.api_keys(id) ON DELETE SET NULL,

  model_id UUID REFERENCES models(id),
  instance_id UUID REFERENCES instances(id),

  input_tokens INTEGER,
  output_tokens INTEGER,
  total_tokens INTEGER,

  -- If you price usage directly, you can record a charge here
  unit_price_usd_per_1k_tokens NUMERIC(14, 6),
  charged_amount_usd NUMERIC(14, 6),

  metadata JSONB,

  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_finops_inference_usage_time
  ON finops.inference_usage(occurred_at);

CREATE INDEX IF NOT EXISTS idx_finops_inference_usage_api_key_time
  ON finops.inference_usage(api_key_id, occurred_at)
  WHERE api_key_id IS NOT NULL;
