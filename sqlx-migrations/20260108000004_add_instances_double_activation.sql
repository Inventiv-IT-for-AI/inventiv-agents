-- Migration Phase 2: Add double activation (tech + eco) to instances
-- This enables RBAC-based activation: Admin/Owner for tech, Manager/Owner for eco
-- A resource is operational only if both activations are present

-- Ajouter colonnes double activation à instances
ALTER TABLE public.instances 
  ADD COLUMN IF NOT EXISTS tech_activated_by UUID REFERENCES public.users(id),
  ADD COLUMN IF NOT EXISTS tech_activated_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS eco_activated_by UUID REFERENCES public.users(id),
  ADD COLUMN IF NOT EXISTS eco_activated_at TIMESTAMPTZ;

-- Colonne calculée is_operational (GENERATED ALWAYS AS ... STORED)
-- Note: PostgreSQL ne permet pas de modifier une colonne générée, donc on doit la supprimer d'abord si elle existe
DO $$
BEGIN
    -- Supprimer la colonne si elle existe déjà (pour idempotence)
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_schema = 'public' 
        AND table_name = 'instances' 
        AND column_name = 'is_operational'
    ) THEN
        ALTER TABLE public.instances DROP COLUMN is_operational;
    END IF;
    
    -- Créer la colonne générée
    ALTER TABLE public.instances 
    ADD COLUMN is_operational BOOLEAN GENERATED ALWAYS AS (
        tech_activated_by IS NOT NULL AND eco_activated_by IS NOT NULL
    ) STORED;
EXCEPTION
    WHEN duplicate_column THEN
        -- Colonne déjà créée, ignorer
        NULL;
END $$;

-- Index pour performance
CREATE INDEX IF NOT EXISTS idx_instances_operational ON public.instances(organization_id, is_operational) WHERE is_operational = true;
CREATE INDEX IF NOT EXISTS idx_instances_tech_activation ON public.instances(organization_id, tech_activated_by) WHERE tech_activated_by IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_instances_eco_activation ON public.instances(organization_id, eco_activated_by) WHERE eco_activated_by IS NOT NULL;

