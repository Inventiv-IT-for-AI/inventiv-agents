-- Prepare multi-tenant model sharing (Org -> Org) + chargeback billing
-- Non-breaking: adds new tables and nullable columns only.
-- Safe to run multiple times.

-- 1) Allow API keys to be scoped to an organization (future: org-owned keys instead of user-only keys)
ALTER TABLE public.api_keys
  ADD COLUMN IF NOT EXISTS organization_id UUID REFERENCES public.organizations(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_api_keys_org_created
  ON public.api_keys(organization_id, created_at DESC)
  WHERE organization_id IS NOT NULL;

-- 2) "Published model" within an organization (virtual model / product surface).
-- This represents a model offering (e.g. "sales-bot") that can be shared/sold.
CREATE TABLE IF NOT EXISTS public.organization_models (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES public.organizations(id) ON DELETE CASCADE,
  model_id UUID NOT NULL REFERENCES public.models(id) ON DELETE RESTRICT,
  name TEXT NOT NULL,
  code TEXT NOT NULL, -- org-scoped identifier (e.g. "sales-bot"); unique per org
  description TEXT,
  is_active BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS organization_models_org_code_key
  ON public.organization_models(organization_id, code);
CREATE INDEX IF NOT EXISTS organization_models_org_active_idx
  ON public.organization_models(organization_id, is_active, created_at DESC);
CREATE INDEX IF NOT EXISTS organization_models_model_idx
  ON public.organization_models(model_id);

-- 3) Sharing contract between provider org (owner of the offering) and consumer org.
-- Pricing is stored as JSON for flexibility (per-1k-tokens, per-minute, tiered, etc.).
CREATE TABLE IF NOT EXISTS public.organization_model_shares (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  provider_organization_id UUID NOT NULL REFERENCES public.organizations(id) ON DELETE CASCADE,
  consumer_organization_id UUID NOT NULL REFERENCES public.organizations(id) ON DELETE CASCADE,
  organization_model_id UUID NOT NULL REFERENCES public.organization_models(id) ON DELETE CASCADE,
  status TEXT NOT NULL DEFAULT 'active', -- active|paused|revoked|expired
  pricing JSONB NOT NULL DEFAULT '{}'::jsonb,
  starts_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  ends_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT organization_model_shares_distinct_orgs CHECK (provider_organization_id <> consumer_organization_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS organization_model_shares_unique_active
  ON public.organization_model_shares(provider_organization_id, consumer_organization_id, organization_model_id)
  WHERE status = 'active';

CREATE INDEX IF NOT EXISTS organization_model_shares_consumer_idx
  ON public.organization_model_shares(consumer_organization_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS organization_model_shares_provider_idx
  ON public.organization_model_shares(provider_organization_id, status, created_at DESC);

-- 4) Extend usage events for chargeback: who provides the model vs who consumes it.
-- NOTE: finops.inference_usage already exists in baseline.
ALTER TABLE finops.inference_usage
  ADD COLUMN IF NOT EXISTS provider_organization_id UUID,
  ADD COLUMN IF NOT EXISTS consumer_organization_id UUID,
  ADD COLUMN IF NOT EXISTS organization_model_id UUID,
  ADD COLUMN IF NOT EXISTS unit_price_eur_per_1k_tokens NUMERIC(14,6),
  ADD COLUMN IF NOT EXISTS charged_amount_eur NUMERIC(14,6);

CREATE INDEX IF NOT EXISTS idx_finops_inference_usage_consumer_time
  ON finops.inference_usage(consumer_organization_id, occurred_at)
  WHERE consumer_organization_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_finops_inference_usage_provider_time
  ON finops.inference_usage(provider_organization_id, occurred_at)
  WHERE provider_organization_id IS NOT NULL;


