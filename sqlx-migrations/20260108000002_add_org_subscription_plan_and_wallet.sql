-- Migration Phase 2: Add subscription_plan, wallet_balance_eur and sidebar_color to organizations
-- This enables organization-level subscription plans, wallet management, and UX customization

-- Ajouter subscription_plan, wallet et sidebar_color à organizations
ALTER TABLE public.organizations 
  ADD COLUMN IF NOT EXISTS subscription_plan TEXT DEFAULT 'free' NOT NULL,
  ADD COLUMN IF NOT EXISTS subscription_plan_updated_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS wallet_balance_eur NUMERIC(10,2) DEFAULT 0 NOT NULL,
  ADD COLUMN IF NOT EXISTS sidebar_color TEXT;

-- Contrainte CHECK pour subscription_plan
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'organizations_subscription_plan_check'
    ) THEN
        ALTER TABLE public.organizations 
        ADD CONSTRAINT organizations_subscription_plan_check CHECK (subscription_plan IN ('free', 'subscriber'));
    END IF;
END $$;

-- Index pour performance (filtrage des subscribers)
CREATE INDEX IF NOT EXISTS idx_organizations_subscription_plan ON public.organizations(subscription_plan) WHERE subscription_plan = 'subscriber';

