-- Migration Phase 2: Add account_plan and wallet_balance_eur to users
-- This enables user-level subscription plans and wallet management for Personal workspace

-- Ajouter account_plan et wallet à users
ALTER TABLE public.users 
  ADD COLUMN IF NOT EXISTS account_plan TEXT DEFAULT 'free' NOT NULL,
  ADD COLUMN IF NOT EXISTS account_plan_updated_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS wallet_balance_eur NUMERIC(10,2) DEFAULT 0 NOT NULL;

-- Contrainte CHECK pour account_plan
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'users_account_plan_check'
    ) THEN
        ALTER TABLE public.users 
        ADD CONSTRAINT users_account_plan_check CHECK (account_plan IN ('free', 'subscriber'));
    END IF;
END $$;

-- Index pour performance (filtrage des subscribers)
CREATE INDEX IF NOT EXISTS idx_users_account_plan ON public.users(account_plan) WHERE account_plan = 'subscriber';

