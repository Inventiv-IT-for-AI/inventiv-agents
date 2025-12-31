-- Organizations (multi-tenant MVP)
-- - users can create organizations and be members (owner/admin/member)
-- - users can select a "current organization" (stored on users; can also be embedded in session JWT)
--
-- Safe to run multiple times.

CREATE TABLE IF NOT EXISTS public.organizations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name TEXT NOT NULL,
  slug TEXT NOT NULL,
  created_by_user_id UUID NOT NULL REFERENCES public.users(id) ON DELETE RESTRICT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS organizations_slug_key ON public.organizations (slug);
CREATE INDEX IF NOT EXISTS organizations_created_by_idx ON public.organizations (created_by_user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS public.organization_memberships (
  organization_id UUID NOT NULL REFERENCES public.organizations(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES public.users(id) ON DELETE CASCADE,
  role TEXT NOT NULL DEFAULT 'member',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (organization_id, user_id)
);

CREATE INDEX IF NOT EXISTS organization_memberships_user_idx
  ON public.organization_memberships (user_id, organization_id);

-- Selected organization for UX (org switcher)
ALTER TABLE public.users
  ADD COLUMN IF NOT EXISTS current_organization_id UUID REFERENCES public.organizations(id) ON DELETE SET NULL;


