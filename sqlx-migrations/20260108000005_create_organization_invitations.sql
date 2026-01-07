-- Migration Phase 3: Create organization_invitations table
-- This enables inviting users by email to join an organization with a specific role

-- Table pour les invitations d'organisation
CREATE TABLE IF NOT EXISTS public.organization_invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES public.organizations(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    token TEXT NOT NULL UNIQUE,
    invited_by_user_id UUID NOT NULL REFERENCES public.users(id),
    expires_at TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Un utilisateur ne peut avoir qu'une invitation active par organisation
    CONSTRAINT organization_invitations_org_email_unique UNIQUE (organization_id, email, accepted_at)
);

-- Contrainte CHECK pour role
ALTER TABLE public.organization_invitations 
  ADD CONSTRAINT organization_invitations_role_check CHECK (role IN ('owner', 'admin', 'manager', 'user'));

-- Index pour performance
CREATE INDEX IF NOT EXISTS idx_organization_invitations_token ON public.organization_invitations(token) WHERE accepted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_organization_invitations_org ON public.organization_invitations(organization_id, accepted_at);
CREATE INDEX IF NOT EXISTS idx_organization_invitations_email ON public.organization_invitations(email, accepted_at);

-- Trigger pour updated_at
CREATE OR REPLACE FUNCTION update_organization_invitations_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER organization_invitations_updated_at
    BEFORE UPDATE ON public.organization_invitations
    FOR EACH ROW
    EXECUTE FUNCTION update_organization_invitations_updated_at();

