-- Attach *_i18n_id columns to catalog tables and backfill en-US texts from existing columns.
-- Also enforce the invariant: uniqueness relies on codes (not names/labels).

-- 1) Drop name-based unique constraints (keep uniqueness on codes)
ALTER TABLE public.providers DROP CONSTRAINT IF EXISTS providers_name_key;
ALTER TABLE public.regions DROP CONSTRAINT IF EXISTS regions_provider_id_name_key;
ALTER TABLE public.zones DROP CONSTRAINT IF EXISTS zones_region_id_name_key;
ALTER TABLE public.instance_types DROP CONSTRAINT IF EXISTS instance_types_provider_id_name_key;

-- 2) Add *_i18n_id columns
ALTER TABLE public.providers
  ADD COLUMN IF NOT EXISTS name_i18n_id UUID,
  ADD COLUMN IF NOT EXISTS description_i18n_id UUID;

ALTER TABLE public.regions
  ADD COLUMN IF NOT EXISTS name_i18n_id UUID;

ALTER TABLE public.zones
  ADD COLUMN IF NOT EXISTS name_i18n_id UUID;

ALTER TABLE public.instance_types
  ADD COLUMN IF NOT EXISTS name_i18n_id UUID;

ALTER TABLE public.action_types
  ADD COLUMN IF NOT EXISTS label_i18n_id UUID;

-- 3) Backfill ids (deterministic enough; one key per row/field)
UPDATE public.providers
SET name_i18n_id = COALESCE(name_i18n_id, gen_random_uuid()),
    description_i18n_id = COALESCE(description_i18n_id, gen_random_uuid());

UPDATE public.regions
SET name_i18n_id = COALESCE(name_i18n_id, gen_random_uuid());

UPDATE public.zones
SET name_i18n_id = COALESCE(name_i18n_id, gen_random_uuid());

UPDATE public.instance_types
SET name_i18n_id = COALESCE(name_i18n_id, gen_random_uuid());

UPDATE public.action_types
SET label_i18n_id = COALESCE(label_i18n_id, gen_random_uuid());

-- 4) Ensure keys exist in i18n_keys
INSERT INTO public.i18n_keys (id)
SELECT DISTINCT x.id
FROM (
  SELECT name_i18n_id AS id FROM public.providers
  UNION ALL SELECT description_i18n_id AS id FROM public.providers
  UNION ALL SELECT name_i18n_id AS id FROM public.regions
  UNION ALL SELECT name_i18n_id AS id FROM public.zones
  UNION ALL SELECT name_i18n_id AS id FROM public.instance_types
  UNION ALL SELECT label_i18n_id AS id FROM public.action_types
) x
WHERE x.id IS NOT NULL
ON CONFLICT (id) DO NOTHING;

-- 5) Seed en-US texts from current columns (idempotent)
INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
SELECT p.name_i18n_id, 'en-US', p.name
FROM public.providers p
WHERE p.name_i18n_id IS NOT NULL
  AND COALESCE(p.name, '') <> ''
ON CONFLICT (key_id, locale_code) DO NOTHING;

INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
SELECT p.description_i18n_id, 'en-US', p.description
FROM public.providers p
WHERE p.description_i18n_id IS NOT NULL
  AND COALESCE(p.description, '') <> ''
ON CONFLICT (key_id, locale_code) DO NOTHING;

INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
SELECT r.name_i18n_id, 'en-US', r.name
FROM public.regions r
WHERE r.name_i18n_id IS NOT NULL
  AND COALESCE(r.name, '') <> ''
ON CONFLICT (key_id, locale_code) DO NOTHING;

INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
SELECT z.name_i18n_id, 'en-US', z.name
FROM public.zones z
WHERE z.name_i18n_id IS NOT NULL
  AND COALESCE(z.name, '') <> ''
ON CONFLICT (key_id, locale_code) DO NOTHING;

INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
SELECT it.name_i18n_id, 'en-US', it.name
FROM public.instance_types it
WHERE it.name_i18n_id IS NOT NULL
  AND COALESCE(it.name, '') <> ''
ON CONFLICT (key_id, locale_code) DO NOTHING;

INSERT INTO public.i18n_texts (key_id, locale_code, text_value)
SELECT at.label_i18n_id, 'en-US', at.label
FROM public.action_types at
WHERE at.label_i18n_id IS NOT NULL
  AND COALESCE(at.label, '') <> ''
ON CONFLICT (key_id, locale_code) DO NOTHING;


