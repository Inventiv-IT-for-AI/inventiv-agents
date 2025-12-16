-- Add first/last name fields to users + make id auto-generated (quality-of-life)
-- Safe to run multiple times.

ALTER TABLE public.users
  ADD COLUMN IF NOT EXISTS first_name TEXT;

ALTER TABLE public.users
  ADD COLUMN IF NOT EXISTS last_name TEXT;

-- The baseline schema requires providing an id explicitly.
-- Make it default to gen_random_uuid() for easier user creation.
ALTER TABLE public.users
  ALTER COLUMN id SET DEFAULT gen_random_uuid();


