-- Add locales table + link users -> locales (BCP47)
-- Requirements:
-- - Supported locales: fr-FR, en-US, ar
-- - Fallback locale: en-US (DB default)
-- - Default admin locale: fr-FR

-- 1) Locales catalog
CREATE TABLE IF NOT EXISTS public.locales (
  code TEXT PRIMARY KEY, -- BCP47, e.g. 'en-US', 'fr-FR', 'ar'
  name TEXT NOT NULL,
  native_name TEXT,
  direction TEXT NOT NULL DEFAULT 'ltr',
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT locales_direction_check CHECK (direction IN ('ltr','rtl'))
);

-- Idempotent seed of the 3 required locales.
INSERT INTO public.locales (code, name, native_name, direction, is_active)
VALUES
  ('en-US', 'English (United States)', 'English (US)', 'ltr', TRUE),
  ('fr-FR', 'French (France)', 'Français (France)', 'ltr', TRUE),
  ('ar',    'Arabic', 'العربية', 'rtl', TRUE)
ON CONFLICT (code) DO UPDATE SET
  name = EXCLUDED.name,
  native_name = EXCLUDED.native_name,
  direction = EXCLUDED.direction,
  is_active = EXCLUDED.is_active,
  updated_at = NOW();

-- 2) Link users to locales (default fallback = en-US)
ALTER TABLE public.users
  ADD COLUMN IF NOT EXISTS locale_code TEXT;

-- Backfill existing rows (keep simple)
UPDATE public.users
SET locale_code = COALESCE(locale_code, 'en-US')
WHERE locale_code IS NULL;

-- Set default admin locale to fr-FR (bootstrap user)
UPDATE public.users
SET locale_code = 'fr-FR'
WHERE locale_code IS NOT NULL
  AND (username = 'admin' OR email = 'admin@inventiv.local');

ALTER TABLE public.users
  ALTER COLUMN locale_code SET DEFAULT 'en-US';

ALTER TABLE public.users
  ALTER COLUMN locale_code SET NOT NULL;

-- FK (added after backfill to avoid failures)
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'users_locale_code_fkey'
  ) THEN
    ALTER TABLE public.users
      ADD CONSTRAINT users_locale_code_fkey
      FOREIGN KEY (locale_code) REFERENCES public.locales(code);
  END IF;
END $$;


