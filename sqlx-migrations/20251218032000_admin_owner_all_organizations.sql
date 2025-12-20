-- Ensure the default/admin user is owner of all organizations (bootstrap compatibility)
-- Idempotent: safe to run multiple times.
--
-- Strategy:
-- - Locate admin user by username 'admin' (preferred) or email 'admin@inventiv.local' (fallback)
-- - Upsert membership for every organization to role 'owner'

WITH admin_user AS (
  SELECT id
  FROM public.users
  WHERE username = 'admin'
     OR email = 'admin@inventiv.local'
  ORDER BY (username = 'admin') DESC, created_at ASC
  LIMIT 1
),
orgs AS (
  SELECT id AS organization_id
  FROM public.organizations
)
INSERT INTO public.organization_memberships (organization_id, user_id, role, created_at)
SELECT
  orgs.organization_id,
  admin_user.id,
  'owner',
  NOW()
FROM orgs
CROSS JOIN admin_user
ON CONFLICT (organization_id, user_id)
DO UPDATE SET role = 'owner';


