-- Normalize organization membership roles (RBAC foundation)
-- Safe to run multiple times.

-- 1) Backfill legacy values (if any)
UPDATE public.organization_memberships
SET role = 'user'
WHERE role IS NULL OR lower(role) IN ('member', 'membre');

UPDATE public.organization_memberships
SET role = lower(role)
WHERE role IS NOT NULL AND role <> lower(role);

-- 2) Default role
ALTER TABLE public.organization_memberships
  ALTER COLUMN role SET DEFAULT 'user';

-- 3) Constrain allowed roles
ALTER TABLE public.organization_memberships
  DROP CONSTRAINT IF EXISTS organization_memberships_role_check;

ALTER TABLE public.organization_memberships
  ADD CONSTRAINT organization_memberships_role_check
  CHECK (role IN ('owner','admin','manager','user'));


