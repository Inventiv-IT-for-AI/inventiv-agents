-- Add username to users and enforce uniqueness (used for login)
-- Backfill existing rows safely, then enforce NOT NULL + UNIQUE.

ALTER TABLE public.users
  ADD COLUMN IF NOT EXISTS username TEXT;

-- Backfill username for existing users:
-- - base username from email local-part
-- - ensure uniqueness by appending a short id suffix when duplicates exist
WITH base AS (
  SELECT
    id,
    COALESCE(username, regexp_replace(email, '@.*$', '')) AS base_username
  FROM public.users
),
ranked AS (
  SELECT
    id,
    base_username,
    row_number() OVER (PARTITION BY base_username ORDER BY id) AS rn
  FROM base
)
UPDATE public.users u
SET username = CASE
  WHEN r.rn = 1 THEN r.base_username
  ELSE r.base_username || '_' || substr(u.id::text, 1, 8)
END
FROM ranked r
WHERE u.id = r.id
  AND u.username IS NULL;

-- Enforce non-null + unique
ALTER TABLE public.users
  ALTER COLUMN username SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS users_username_key ON public.users (username);


