-- Make zone codes unique per provider to avoid ambiguous lookups.
-- This is a structural fix: the UI/API uses zone.code, so the DB must guarantee it's unique for a provider.

-- 1) Add provider_id on zones (denormalized from regions.provider_id)
ALTER TABLE public.zones
  ADD COLUMN IF NOT EXISTS provider_id uuid;

-- 2) Backfill from regions
UPDATE public.zones z
SET provider_id = r.provider_id
FROM public.regions r
WHERE r.id = z.region_id
  AND (z.provider_id IS NULL OR z.provider_id <> r.provider_id);

-- 3) Enforce NOT NULL
ALTER TABLE public.zones
  ALTER COLUMN provider_id SET NOT NULL;

-- 4) FK to providers
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'zones_provider_id_fkey'
  ) THEN
    ALTER TABLE public.zones
      ADD CONSTRAINT zones_provider_id_fkey
      FOREIGN KEY (provider_id) REFERENCES public.providers(id) ON DELETE CASCADE;
  END IF;
END $$;

-- 5) Guard: provider_id must match region.provider_id
CREATE OR REPLACE FUNCTION public.zones_sync_provider_id()
RETURNS TRIGGER AS $$
DECLARE
  pid uuid;
BEGIN
  SELECT provider_id INTO pid FROM public.regions WHERE id = NEW.region_id;
  IF pid IS NULL THEN
    RAISE EXCEPTION 'zones.region_id references missing region_id=%', NEW.region_id;
  END IF;
  NEW.provider_id := pid;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_zones_sync_provider_id ON public.zones;
CREATE TRIGGER trg_zones_sync_provider_id
BEFORE INSERT OR UPDATE OF region_id ON public.zones
FOR EACH ROW
EXECUTE FUNCTION public.zones_sync_provider_id();

-- 6) Hard constraint: zone code unique per provider
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'zones_provider_code_key'
  ) THEN
    ALTER TABLE public.zones
      ADD CONSTRAINT zones_provider_code_key UNIQUE (provider_id, code);
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_zones_provider_code ON public.zones(provider_id, code);


