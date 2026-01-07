-- Migration Phase 2: Add organization_id to instances
-- This enables workspace scoping: instances belong to an organization (or are public if NULL)

-- Ajouter organization_id à instances
ALTER TABLE public.instances 
  ADD COLUMN IF NOT EXISTS organization_id UUID REFERENCES public.organizations(id) ON DELETE SET NULL;

-- Index pour performance (workspace scoping)
CREATE INDEX IF NOT EXISTS idx_instances_org ON public.instances(organization_id) WHERE organization_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_instances_org_status ON public.instances(organization_id, status) WHERE organization_id IS NOT NULL;

